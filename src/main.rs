use egui_winit_vulkano::{
    Gui, GuiConfig,
    egui::{self, Color32},
};
use nalgebra::{Matrix3, Matrix4, Vector2, Vector3, Vector4};
use std::path::PathBuf;
use std::{
    f64::consts::{FRAC_PI_2, TAU},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use vulkano::{
    Validated, Version, VulkanError, VulkanLibrary,
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage},
    command_buffer::{
        AutoCommandBufferBuilder, BlitImageInfo, ClearColorImageInfo, CommandBufferUsage,
        CopyBufferInfo, CopyBufferToImageInfo, CopyImageToBufferInfo, PrimaryAutoCommandBuffer,
        PrimaryCommandBufferAbstract,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{
        DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator,
    },
    device::{
        Device, DeviceCreateInfo, DeviceExtensions, DeviceFeatures, Queue, QueueCreateInfo,
        QueueFlags, physical::PhysicalDeviceType,
    },
    format::{Format, NumericFormat},
    image::{
        Image, ImageCreateInfo, ImageType, ImageUsage,
        sampler::Filter,
        view::{ImageView, ImageViewCreateInfo},
    },
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    swapchain::{
        PresentMode, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
        acquire_next_image,
    },
    sync::{self, GpuFuture},
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::KeyCode,
    window::{CursorGrabMode, Icon, Window, WindowId},
};
use winit_input_helper::WinitInputHelper;

mod camera;
mod hot_reload;
mod player;
mod voxelize;
mod worldgen;

use crate::camera::Camera;
use crate::hot_reload::HotReloadComputePipeline;
use crate::player::Player;
use crate::worldgen::{Terrain, TerrainSettings, World};

const INITIAL_VOXEL_RESOLUTION: u32 = 24;
const INITIAL_TERRAIN: TerrainSettings = TerrainSettings {
    width_m: 256,
    height_m: 64,
    voxels_per_meter: 8,
    seed: 1,
};
const VOXELS_PER_METER_OPTIONS: &[u32] = &[1, 2, 4, 8, 16];
const INITIAL_WINDOW_RESOLUTION: PhysicalSize<u32> = PhysicalSize::new(960, 960);

// How many frames each pixel averages. While still, more stops improving the image noticeably.
// While moving, fewer keeps old frames from smearing across the new ones.
const STILL_HISTORY: u32 = 4096;
const MOVING_HISTORY: u32 = 12;

// Has to match the FLAG_* constants in shaders\traverse.comp
const FLAG_SHADOWS: u32 = 1;
const FLAG_AO: u32 = 2;
const FLAG_GI: u32 = 4;
const FLAG_SUNBEAMS: u32 = 8;
const FLAG_GRASS: u32 = 16;

// Has to match shaders\traverse.comp
const SUN_MAP_MODE: u32 = 100;
const SUN_MAP_SIZE: u32 = 512;

// Has to match the POST_* constants and stages in shaders\post.comp
const POST_ENCODE_SRGB: u32 = 1;
const POST_GRADE: u32 = 2;
const POST_DENOISE: u32 = 4;
#[derive(Clone, Copy)]
enum PostStage {
    Denoise,
    BloomExtract,
    BloomBlurX,
    BloomBlurY,
    Tonemap,
}
// Bloom is computed at this fraction of the render resolution, has to match shaders\post.comp
const BLOOM_SCALE: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderMode {
    Shaded,
    Coord,
    Steps,
    Normal,
    UV,
    Depth,
}

impl RenderMode {
    pub const ALL: &'static [RenderMode] = &[
        RenderMode::Shaded,
        RenderMode::Coord,
        RenderMode::Steps,
        RenderMode::Normal,
        RenderMode::UV,
        RenderMode::Depth,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scene {
    Terrain,
    Model(Model),
}

impl Scene {
    pub const ALL: &'static [Scene] = &[
        Scene::Terrain,
        Scene::Model(Model::Bunny),
        Scene::Model(Model::Dragon),
        Scene::Model(Model::Armadillo),
    ];

    pub fn name(&self) -> String {
        match self {
            Scene::Terrain => "Terrain".to_string(),
            Scene::Model(model) => format!("{:?}", model),
        }
    }

    pub fn is_terrain(&self) -> bool {
        matches!(self, Scene::Terrain)
    }

    pub fn initial_camera(&self) -> Camera {
        let mut camera;
        if self.is_terrain() {
            // A lake shore on the default seed
            camera = Camera::new(
                Vector3::new(0.42, 0.30, -0.02),
                Vector3::zeros(),
                INITIAL_WINDOW_RESOLUTION.into(),
                70.0,
            );
            camera.look_at(Vector3::new(0.1, 0.0, -0.05));
        } else {
            camera = Camera::new(
                Vector3::new(2.0, 3.0, 1.0),
                Vector3::zeros(),
                INITIAL_WINDOW_RESOLUTION.into(),
                20.0,
            );
            camera.look_at(Vector3::zeros());
        }
        camera
    }

    pub fn initial_move_speed(&self) -> f64 {
        if self.is_terrain() { 0.03 } else { 1.0 }
    }
}

// Camera space has the world centered on the origin, with its largest side one unit long
fn camera_to_voxels(position: Vector3<f64>, world_size: [u32; 3]) -> Vector3<f64> {
    let size = Vector3::from(world_size).cast::<f64>();
    position * size.max() + size * 0.5
}

fn voxels_to_camera(position: Vector3<f64>, world_size: [u32; 3]) -> Vector3<f64> {
    let size = Vector3::from(world_size).cast::<f64>();
    (position - size * 0.5) / size.max()
}

/// The world in voxels, and for generated landscapes what it was built from, to build finer pieces of later
fn build_world(scene: Scene, terrain: &TerrainSettings, voxel_resolution: u32) -> (World, Option<Arc<Terrain>>) {
    match scene {
        Scene::Terrain => {
            let (landscape, world) = worldgen::generate_terrain(terrain);
            (world, Some(Arc::new(landscape)))
        }
        Scene::Model(model) => {
            let occupancy = voxelize::ply_to_voxels(model.path(), voxel_resolution);
            let world = World::from_packed_occupancy(
                [voxel_resolution; 3],
                &occupancy,
                worldgen::material::CLAY,
            );
            (world, None)
        }
    }
}

/// Finer voxels in a box around the camera, rebuilt in the background as it moves.
struct NearDetail {
    set: Arc<DescriptorSet>,
    /// The box's corner in voxels of the main world
    origin: Vector3<f64>,
    /// How many fine voxels fit along a main one
    scale: f64,
    /// In fine voxels
    size: [u32; 3],
    /// Where the camera was when it was built, in voxels of the main world
    center: Vector3<f64>,
}

struct NearBuild {
    job: std::thread::JoinHandle<World>,
    landscape: Arc<Terrain>,
    center: Vector3<f64>,
}

// The box of fine detail around the camera, in meters. It gets rebuilt once the camera
// gets this far from where it was last built.
const NEAR_WIDTH_M: u32 = 24;
const NEAR_HEIGHT_M: u32 = 16;
const NEAR_REBUILD_M: f64 = 6.0;
const NEAR_DETAIL_OPTIONS: &[u32] = &[0, 16, 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Model {
    Bunny,
    Dragon,
    Armadillo,
}

impl Model {
    pub fn path(&self) -> impl AsRef<Path> {
        let buf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("models");
        match self {
            Model::Bunny => buf.join("bunny_remeshed.ply"),
            Model::Dragon => buf.join("dragon.ply"),
            Model::Armadillo => buf.join("armadillo.ply"),
        }
    }
}

fn get_allocators(
    device: &Arc<Device>,
) -> (
    Arc<StandardMemoryAllocator>,
    Arc<StandardDescriptorSetAllocator>,
    Arc<StandardCommandBufferAllocator>,
) {
    let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));
    let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
        device.clone(),
        Default::default(),
    ));
    let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
        device.clone(),
        Default::default(),
    ));
    (
        memory_allocator,
        descriptor_set_allocator,
        command_buffer_allocator,
    )
}

fn get_swapchain_images(
    device: &Arc<Device>,
    surface: &Arc<Surface>,
    window: &Window,
) -> (Arc<Swapchain>, Vec<Arc<Image>>) {
    let caps = device
        .physical_device()
        .surface_capabilities(surface, Default::default())
        .unwrap();

    let image_format = device
        .physical_device()
        .surface_formats(surface, Default::default())
        .unwrap()[0]
        .0;

    let composite_alpha = caps.supported_composite_alpha.into_iter().next().unwrap();

    Swapchain::new(
        device.clone(),
        surface.clone(),
        SwapchainCreateInfo {
            min_image_count: caps.min_image_count.max(3),
            image_format,
            image_extent: window.inner_size().into(),
            image_usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST,
            composite_alpha,
            present_mode: PresentMode::Immediate,
            ..Default::default()
        },
    )
    .unwrap()
}

fn get_render_image(
    memory_allocator: Arc<StandardMemoryAllocator>,
    extent: [u32; 2],
) -> (Arc<Image>, Arc<ImageView>) {
    let image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_DST,
            format: Format::R8G8B8A8_UNORM,
            extent: [extent[0], extent[1], 1],
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )
    .unwrap();

    let image_view =
        ImageView::new(image.clone(), ImageViewCreateInfo::from_image(&image)).unwrap();

    (image, image_view)
}

// For the shaded frames averaged over time, and the depth of each pixel to reproject them with
fn get_history_image_view(
    memory_allocator: Arc<StandardMemoryAllocator>,
    extent: [u32; 2],
    format: Format,
) -> Arc<ImageView> {
    let image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            usage: ImageUsage::STORAGE,
            format,
            extent: [extent[0], extent[1], 1],
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )
    .unwrap();

    ImageView::new(image.clone(), ImageViewCreateInfo::from_image(&image)).unwrap()
}

fn get_resample_image(
    memory_allocator: Arc<StandardMemoryAllocator>,
    extent: [u32; 2],
) -> (Arc<Image>, Arc<ImageView>) {
    let image = Image::new(
        memory_allocator.clone(),
        ImageCreateInfo {
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_SRC,
            format: Format::R8G8B8A8_UNORM,
            extent: [extent[0], extent[1], 1],
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )
    .unwrap();

    let image_view =
        ImageView::new(image.clone(), ImageViewCreateInfo::from_image(&image)).unwrap();

    (image, image_view)
}

/// Everything sized to the render or window resolution, recreated when either changes.
struct RenderImages {
    render_image: Arc<Image>,
    // Indexed by frame parity, see get_render_images()
    render_sets: [Arc<DescriptorSet>; 2],
    // The denoiser's three passes, by frame parity
    denoise_sets: [[Arc<DescriptorSet>; 3]; 2],
    bloom_extract_set: Arc<DescriptorSet>,
    bloom_blur_sets: [Arc<DescriptorSet>; 2],
    tonemap_set: Arc<DescriptorSet>,
    bloom_extent: [u32; 2],
    resample_image: Arc<Image>,
    resample_set: Arc<DescriptorSet>,
}

#[allow(clippy::too_many_arguments)]
fn get_render_images(
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    render_pipeline: &ComputePipeline,
    post_pipeline: &ComputePipeline,
    resample_pipeline: &ComputePipeline,
    sun_map: &Arc<ImageView>,
    render_extent: [u32; 2],
    window_extent: [u32; 2],
) -> RenderImages {
    let (render_image, render_image_view) =
        get_render_image(memory_allocator.clone(), render_extent);
    let float_image = |extent: [u32; 2], format: Format| {
        get_history_image_view(memory_allocator.clone(), extent, format)
    };

    // Two sets of history images that swap roles every frame, one read while the other is written
    let colors = [0, 1].map(|_| float_image(render_extent, Format::R32G32B32A32_SFLOAT));
    let depths = [0, 1].map(|_| float_image(render_extent, Format::R32_SFLOAT));
    let guide = float_image(render_extent, Format::R32G32B32A32_SFLOAT);

    let layout = render_pipeline.layout().set_layouts()[0].clone();
    let render_sets = [0, 1].map(|write| {
        let read = 1 - write;
        DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [
                WriteDescriptorSet::image_view(0, render_image_view.clone()),
                WriteDescriptorSet::image_view(1, colors[read].clone()),
                WriteDescriptorSet::image_view(2, depths[read].clone()),
                WriteDescriptorSet::image_view(3, colors[write].clone()),
                WriteDescriptorSet::image_view(4, depths[write].clone()),
                WriteDescriptorSet::image_view(5, guide.clone()),
                WriteDescriptorSet::image_view(6, sun_map.clone()),
            ],
            [],
        )
        .unwrap()
    });

    // post.comp's passes each read one image and write another, ping-ponging between these
    let temps = [0, 1].map(|_| float_image(render_extent, Format::R32G32B32A32_SFLOAT));
    let bloom_extent = render_extent.map(|n| n.div_ceil(BLOOM_SCALE).max(1));
    let blooms = [0, 1].map(|_| float_image(bloom_extent, Format::R32G32B32A32_SFLOAT));

    let layout = post_pipeline.layout().set_layouts()[0].clone();
    let post_set = |hdr_in: &Arc<ImageView>, hdr_out: &Arc<ImageView>, depth: &Arc<ImageView>, bloom_in: usize| {
        DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [
                WriteDescriptorSet::image_view(0, hdr_in.clone()),
                WriteDescriptorSet::image_view(1, hdr_out.clone()),
                WriteDescriptorSet::image_view(2, depth.clone()),
                WriteDescriptorSet::image_view(3, guide.clone()),
                WriteDescriptorSet::image_view(4, blooms[bloom_in].clone()),
                WriteDescriptorSet::image_view(5, blooms[1 - bloom_in].clone()),
                WriteDescriptorSet::image_view(6, render_image_view.clone()),
            ],
            [],
        )
        .unwrap()
    };
    // The denoiser reads this frame's colors and ends up in temps[0]
    let denoise_sets = [0, 1].map(|parity| {
        [
            post_set(&colors[parity], &temps[0], &depths[parity], 0),
            post_set(&temps[0], &temps[1], &depths[parity], 0),
            post_set(&temps[1], &temps[0], &depths[parity], 0),
        ]
    });
    // Bloom ends up in blooms[0], which tone mapping then reads
    let bloom_extract_set = post_set(&temps[0], &temps[1], &depths[0], 1);
    let bloom_blur_sets = [
        post_set(&temps[0], &temps[1], &depths[0], 0),
        post_set(&temps[0], &temps[1], &depths[0], 1),
    ];
    let tonemap_set = post_set(&temps[0], &temps[1], &depths[0], 0);

    let (resample_image, resample_image_view) = get_resample_image(memory_allocator.clone(), window_extent);

    let layout = resample_pipeline.layout().set_layouts()[0].clone();
    let resample_set = DescriptorSet::new(
        descriptor_set_allocator.clone(),
        layout,
        [
            WriteDescriptorSet::image_view(0, render_image_view.clone()),
            WriteDescriptorSet::image_view(1, resample_image_view.clone()),
        ],
        [],
    )
    .unwrap();

    RenderImages {
        render_image,
        render_sets,
        denoise_sets,
        bloom_extract_set,
        bloom_blur_sets,
        tonemap_set,
        bloom_extent,
        resample_image,
        resample_set,
    }
}

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct PostConstants {
    stage: u32,
    step: u32,
    flags: u32,
    bloom: f32,
}

// Runs one of post.comp's stages over an image of the given size
fn dispatch_post(
    builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
    pipeline: &Arc<ComputePipeline>,
    set: &Arc<DescriptorSet>,
    constants: PostConstants,
    extent: [u32; 2],
) {
    builder
        .bind_pipeline_compute(pipeline.clone())
        .unwrap()
        .push_constants(pipeline.layout().clone(), 0, constants)
        .unwrap()
        .bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, vec![set.clone()])
        .unwrap();
    unsafe {
        builder.dispatch([extent[0].div_ceil(8), extent[1].div_ceil(8), 1]).unwrap();
    }
}

// The inverse of a pixel-to-ray matrix: projects a world position to pixel = xy / z
fn world_to_pixel(pixel_to_ray: &Matrix4<f64>) -> Matrix4<f64> {
    let rays: Matrix3<f64> = pixel_to_ray.fixed_view::<3, 3>(0, 0).into_owned();
    let origin: Vector3<f64> = pixel_to_ray.fixed_view::<3, 1>(0, 3).into_owned();
    let inverse = rays.try_inverse().unwrap_or_else(Matrix3::identity);
    let mut result = Matrix4::identity();
    result.fixed_view_mut::<3, 3>(0, 0).copy_from(&inverse);
    result.fixed_view_mut::<3, 1>(0, 3).copy_from(&(-inverse * origin));
    result
}

// Uploads a world, for traverse.comp's descriptor set `set_index`: 1 for the main world, 2 for the fine detail
fn get_voxel_set(
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    render_pipeline: &ComputePipeline,
    queue: &Arc<Queue>,
    set_index: usize,
    world: &World,
) -> Arc<DescriptorSet> {
    let [sx, sy, sz] = world.size;

    // One texel per 8x8x8 brick, see worldgen::World
    let image = Image::new(
        memory_allocator.clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim3d,
            format: Format::R32_UINT,
            extent: [sx / 8, sy / 8, sz / 8],
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap();

    let src_buffer = Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        world.brick_grid.iter().copied(),
    )
    .unwrap();

    // An empty world still needs a buffer to bind
    let brick_words: &[u32] = if world.bricks.is_empty() {
        &[0; worldgen::BRICK_WORDS]
    } else {
        &world.bricks
    };
    let bricks_src_buffer = Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        brick_words.iter().copied(),
    )
    .unwrap();
    let bricks_buffer = Buffer::new_slice::<u32>(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
        brick_words.len() as u64,
    )
    .unwrap();

    let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
        command_buffer_allocator.clone(),
        queue.queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )
    .unwrap();

    command_buffer_builder
        .clear_color_image(ClearColorImageInfo::image(image.clone()))
        .unwrap()
        .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            src_buffer,
            image.clone(),
        ))
        .unwrap()
        .copy_buffer(CopyBufferInfo::buffers(
            bricks_src_buffer,
            bricks_buffer.clone(),
        ))
        .unwrap();

    let image_view =
        ImageView::new(image.clone(), ImageViewCreateInfo::from_image(&image)).unwrap();
    let mut writes = vec![
        WriteDescriptorSet::image_view(0, image_view),
        WriteDescriptorSet::buffer(1, bricks_buffer),
    ];

    // The main world also has the map of where grass grows, see worldgen::World::grass
    if set_index == 1 {
        let (extent, grass): ([u32; 2], &[u32]) = match &world.grass {
            Some(grass) => ([sx, sy], grass),
            None => ([1, 1], &[0]),
        };
        let grass_image = Image::new(
            memory_allocator.clone(),
            ImageCreateInfo {
                format: Format::R32_UINT,
                extent: [extent[0], extent[1], 1],
                usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .unwrap();
        let grass_src_buffer = Buffer::from_iter(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            grass.iter().copied(),
        )
        .unwrap();
        command_buffer_builder
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                grass_src_buffer,
                grass_image.clone(),
            ))
            .unwrap();
        let grass_view =
            ImageView::new(grass_image.clone(), ImageViewCreateInfo::from_image(&grass_image)).unwrap();
        writes.push(WriteDescriptorSet::image_view(2, grass_view));
    }

    let _ = command_buffer_builder
        .build()
        .unwrap()
        .execute(queue.clone())
        .unwrap();

    let layout = render_pipeline
        .layout()
        .set_layouts()
        .get(set_index)
        .unwrap()
        .clone();
    DescriptorSet::new(descriptor_set_allocator.clone(), layout.clone(), writes, []).unwrap()
}

fn load_icon(icon: &[u8]) -> Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(icon).unwrap().to_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    Icon::from_rgba(icon_rgba, icon_width, icon_height).unwrap()
}

struct App {
    instance: Arc<Instance>,
    device: Arc<Device>,
    queue: Arc<Queue>,

    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,

    render_pipeline: HotReloadComputePipeline,
    post_pipeline: HotReloadComputePipeline,
    resample_pipeline: HotReloadComputePipeline,

    // Which parts of the world sunlight reaches, for sunbeams. Computed for this sun direction.
    sun_map: Arc<ImageView>,
    sun_map_for: Option<[f32; 4]>,

    voxel_set: Arc<DescriptorSet>,
    // Kept on the CPU too, for walking around in it
    world: World,
    // What a generated world was built from
    landscape: Option<Arc<Terrain>>,
    // Voxels per meter in the box of fine detail around the camera, 0 for none
    near_detail: u32,
    near: Option<NearDetail>,
    near_build: Option<NearBuild>,
    empty_near_set: Arc<DescriptorSet>,
    // Some while walking, None while flying
    player: Option<Player>,
    world_error: Option<String>,
    loaded_scene: Scene,
    scene: Scene,
    future_voxel_resolution: u32,
    terrain: TerrainSettings,

    camera: Camera,
    move_speed: f64,
    render_mode: RenderMode,
    render_scale: f32,
    still_render_scale: f32,
    // Frames since the camera or lighting last changed
    still_frames: u32,
    last_camera_view: Option<[f64; 13]>,

    sun_azimuth: f32,
    sun_elevation: f32,
    shadows: bool,
    ambient_occlusion: bool,
    bounce_light: bool,
    sunbeams: bool,
    // Density of the haze near the ground, per meter
    haze: f32,
    grass: bool,
    denoise: bool,
    color_grade: bool,
    bloom: f32,

    frame: u32,
    accum_count: u32,
    last_view: Option<ViewKey>,
    reset_accum: bool,

    screenshot_requested: bool,
    capture: Option<Capture>,

    input: WinitInputHelper,
    focused: bool,
    last_second: Instant,
    frames_since_last_second: u32,
    fps: u32,

    rcx: Option<RenderContext>,
}

struct RenderContext {
    window: Arc<Window>,
    swapchain: Arc<Swapchain>,
    image_views: Vec<Arc<ImageView>>,

    images: RenderImages,

    gui: Gui,

    recreate_swapchain: bool,
    recreate_images: bool,
    render_scale_in_use: f32,

    // Lets the CPU get on with the next frame while the GPU works on this one
    previous_frame_end: Option<Box<dyn GpuFuture>>,
}

/// Set by `--screenshot <path> [--frames <n>]`: render n frames, save the last one, print timings and exit.
struct Capture {
    path: PathBuf,
    frames: u32,
    frame_times: Vec<Duration>,
    last_frame: Option<Instant>,
}

impl Capture {
    fn from_args() -> Option<Self> {
        let args: Vec<String> = std::env::args().collect();
        let value = |flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
        };
        Some(Capture {
            path: PathBuf::from(value("--screenshot")?),
            frames: value("--frames").and_then(|n| n.parse().ok()).unwrap_or(64),
            frame_times: Vec::new(),
            last_frame: None,
        })
    }
}

/// `--set key=value` arguments, for trying out settings from the command line.
fn settings_from_args() -> Vec<(String, String)> {
    let args: Vec<String> = std::env::args().collect();
    args.windows(2)
        .filter(|w| w[0] == "--set")
        .filter_map(|w| w[1].split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn parse_vector(s: &str) -> Option<Vector3<f64>> {
    let v: Vec<f64> = s.split(',').filter_map(|n| n.trim().parse().ok()).collect();
    (v.len() == 3).then(|| Vector3::new(v[0], v[1], v[2]))
}

fn save_screenshot(buffer: &vulkano::buffer::Subbuffer<[u8]>, extent: [u32; 3], encoded: bool, path: &Path) {
    let mut pixels = buffer.read().unwrap().to_vec();
    if !encoded {
        // The image holds linear values when the swapchain does the sRGB encoding
        for (i, p) in pixels.iter_mut().enumerate() {
            if i % 4 != 3 {
                let c = *p as f32 / 255.0;
                let s = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
                *p = (s * 255.0).round() as u8;
            }
        }
    }
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match image::save_buffer(path, &pixels, extent[0], extent[1], image::ColorType::Rgba8) {
        Ok(()) => println!("Saved screenshot to {}", path.display()),
        Err(e) => eprintln!("Failed to save screenshot: {e}"),
    }
}

// Everything that changes what a pixel converges to. When any of it
// changes, the accumulated frames are thrown away.
#[derive(PartialEq)]
struct ViewKey {
    pixel_to_ray: Matrix4<f32>,
    sun_direction: [f32; 4],
    render_mode: u32,
    flags: u32,
    haze: f32,
}

impl App {
    fn new(event_loop: &EventLoop<()>) -> Self {
        let library = VulkanLibrary::new().unwrap();

        let mut required_extensions = Surface::required_extensions(event_loop).unwrap();

        required_extensions.ext_debug_utils = true;

        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        )
        .unwrap();

        let mut device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };

        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| {
                p.api_version() >= Version::V1_3 || p.supported_extensions().khr_dynamic_rendering
            })
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && p.presentation_support(i as u32, event_loop).unwrap()
                    })
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        if physical_device.api_version() < Version::V1_3 {
            device_extensions.khr_dynamic_rendering = true;
        }

        let (device, mut queues) = Device::new(
            physical_device,
            DeviceCreateInfo {
                queue_create_infos: vec![QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                enabled_extensions: device_extensions,
                enabled_features: DeviceFeatures {
                    dynamic_rendering: true,
                    ..DeviceFeatures::empty()
                },
                ..Default::default()
            },
        )
        .unwrap();

        let queue = queues.next().unwrap();

        let (memory_allocator, descriptor_set_allocator, command_buffer_allocator) =
            get_allocators(&device);

        let shaders_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders");
        let render_pipeline =
            HotReloadComputePipeline::new(device.clone(), &shaders_dir.join("traverse.comp"));
        let post_pipeline =
            HotReloadComputePipeline::new(device.clone(), &shaders_dir.join("post.comp"));
        let resample_pipeline =
            HotReloadComputePipeline::new(device.clone(), &shaders_dir.join("resample.comp"));

        let sun_map = get_history_image_view(
            memory_allocator.clone(),
            [SUN_MAP_SIZE, SUN_MAP_SIZE],
            Format::R32_SFLOAT,
        );

        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());

        let scene = Scene::Terrain;
        let mut terrain = INITIAL_TERRAIN;
        if let Some(v) = setting("voxels_per_meter").and_then(|v| v.parse().ok()) {
            terrain.voxels_per_meter = v;
        }
        let (world, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION);
        let voxel_set = get_voxel_set(
            memory_allocator.clone(),
            command_buffer_allocator.clone(),
            descriptor_set_allocator.clone(),
            &render_pipeline,
            &queue,
            1,
            &world,
        );
        // Bound in place of the fine detail when there isn't any
        let empty_near_set = get_voxel_set(
            memory_allocator.clone(),
            command_buffer_allocator.clone(),
            descriptor_set_allocator.clone(),
            &render_pipeline,
            &queue,
            2,
            &World::from_packed_occupancy([8; 3], &[0; 4], worldgen::material::AIR),
        );

        let input = WinitInputHelper::new();

        let mut camera = scene.initial_camera();
        if let Some(position) = setting("camera").and_then(parse_vector) {
            camera.position = position;
        }
        if let Some(target) = setting("look_at").and_then(parse_vector) {
            camera.look_at(target);
        }
        let flag = |key: &str, default: bool| setting(key).map_or(default, |v| v == "true" || v == "1");

        // Start on foot, except for screenshot runs which need the camera to stay put
        let capture = Capture::from_args();
        let walk = scene.is_terrain() && flag("walk", capture.is_none());
        let player = walk.then(|| Player::on_dry_ground(camera_to_voxels(camera.position, world.size), &world));

        App {
            instance,
            device,
            queue,

            memory_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,

            render_pipeline,
            post_pipeline,
            resample_pipeline,

            sun_map,
            sun_map_for: None,

            voxel_set,
            player,
            world,
            landscape,
            near_detail: setting("near_detail").and_then(|v| v.parse().ok()).unwrap_or(16),
            near: None,
            near_build: None,
            empty_near_set,
            world_error: None,
            loaded_scene: scene,
            scene,
            future_voxel_resolution: INITIAL_VOXEL_RESOLUTION,
            terrain,

            camera,
            move_speed: scene.initial_move_speed(),
            render_mode: RenderMode::Shaded,
            render_scale: setting("render_scale").and_then(|v| v.parse().ok()).unwrap_or(0.5),
            still_render_scale: setting("still_render_scale").and_then(|v| v.parse().ok()).unwrap_or(0.75),
            still_frames: 0,
            last_camera_view: None,

            // Low and ahead to one side of the starting view, so things are backlit
            sun_azimuth: setting("sun_azimuth").and_then(|v| v.parse().ok()).unwrap_or(170.0),
            sun_elevation: setting("sun_elevation").and_then(|v| v.parse().ok()).unwrap_or(18.0),
            shadows: flag("shadows", true),
            ambient_occlusion: flag("ao", true),
            bounce_light: flag("bounce", true),
            sunbeams: flag("sunbeams", true),
            denoise: flag("denoise", true),
            color_grade: flag("grade", true),
            bloom: setting("bloom").and_then(|v| v.parse().ok()).unwrap_or(0.5),
            haze: setting("haze").and_then(|v| v.parse().ok()).unwrap_or(0.007),
            grass: flag("grass", true),

            frame: 0,
            accum_count: 0,
            last_view: None,
            reset_accum: true,

            screenshot_requested: false,
            capture,

            input,
            focused: false,
            last_second: Instant::now(),
            frames_since_last_second: 0,
            fps: 0,

            rcx: None,
        }
    }

    fn update(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if now.duration_since(self.last_second) > Duration::from_secs(1) {
            self.fps = self.frames_since_last_second;
            self.frames_since_last_second = 0;
            self.last_second = now;
        }
        self.frames_since_last_second += 1;

        let Some(delta_time) = self.input.delta_time().as_ref().map(Duration::as_secs_f64) else {
            return;
        };

        if self.input.close_requested() {
            event_loop.exit();
            return;
        }

        // Screenshot runs from the command line ignore input, so they always show the same view
        let interactive = self.capture.is_none();

        if interactive && self.input.key_pressed(KeyCode::KeyF) {
            self.player = match self.player {
                Some(_) => None,
                None => Some(Player::new(camera_to_voxels(self.camera.position, self.world.size), &self.world)),
            };
        }

        let t = |k: KeyCode| (self.focused && interactive && self.input.key_held(k)) as u8 as f64;
        if let Some(player) = &mut self.player {
            // Walk where the camera faces, ignoring whether it looks up or down
            let yaw = self.camera.rotation.z;
            let forward = Vector2::new(-yaw.sin(), yaw.cos());
            let right = Vector2::new(yaw.cos(), yaw.sin());
            let mut wish = forward * (t(KeyCode::KeyW) - t(KeyCode::KeyS)) + right * (t(KeyCode::KeyD) - t(KeyCode::KeyA));
            if wish.norm() > 1.0 {
                wish.normalize_mut();
            }
            let jump = t(KeyCode::Space) > 0.0;
            let sprint = self.focused && self.input.held_shift();
            // Long hitches, like generating a world, shouldn't fling the player through the floor
            player.update(&self.world, wish, jump, sprint, delta_time.min(0.05));
            self.camera.position = voxels_to_camera(player.eye(&self.world), self.world.size);
        } else if self.focused && interactive {
            let v = Vector3::new(KeyCode::KeyD, KeyCode::KeyW, KeyCode::KeyQ).map(t)
                - Vector3::new(KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyE).map(t);

            let sprint = if self.input.held_shift() { 4.0 } else { 1.0 };
            self.camera.position += (self.camera.rotation_matrix()
                * v.push(0.0)
                * delta_time
                * self.move_speed
                * sprint)
                .xyz();
        }

        if self.focused && interactive {
            let sens = 0.001 * (self.camera.fov.to_radians() * 0.5).tan();

            let (dx, dy) = self.input.mouse_diff();
            self.camera.rotation.z -= dx as f64 * sens;
            self.camera.rotation.x -= dy as f64 * sens;
            self.camera.rotation.x = self.camera.rotation.x.clamp(-FRAC_PI_2, FRAC_PI_2);
            self.camera.rotation.y = self.camera.rotation.y.rem_euclid(TAU);

            let ds = self.input.scroll_diff();
            let tanfov = (self.camera.fov.to_radians() * 0.5).tan();
            self.camera.fov = ((tanfov * (ds.1 as f64 * -0.1).exp()).atan() * 2.0).to_degrees();
        }

        let view = [
            self.camera.position.x,
            self.camera.position.y,
            self.camera.position.z,
            self.camera.rotation.x,
            self.camera.rotation.y,
            self.camera.rotation.z,
            self.camera.fov,
            self.sun_azimuth as f64,
            self.sun_elevation as f64,
            self.shadows as u8 as f64,
            self.ambient_occlusion as u8 as f64,
            self.bounce_light as u8 as f64,
            self.render_mode as u8 as f64,
        ];
        self.still_frames = if self.last_camera_view == Some(view) { self.still_frames + 1 } else { 0 };
        self.last_camera_view = Some(view);

        let rcx = self.rcx.as_mut().unwrap();

        if interactive && self.input.mouse_pressed(MouseButton::Left) {
            self.focused = true;
            rcx.window
                .set_cursor_grab(CursorGrabMode::Confined)
                .unwrap();
            rcx.window.set_cursor_visible(false);
        }
        if self.input.key_pressed(KeyCode::F12) {
            self.screenshot_requested = true;
        }
        if self.input.key_pressed(KeyCode::Escape) {
            self.focused = false;
            rcx.window.set_cursor_grab(CursorGrabMode::None).unwrap();
            rcx.window.set_cursor_visible(true);
        }
    }

    // Swaps in a finished box of fine detail, and starts building a new one once the camera
    // has wandered far enough from the middle of the current one
    fn update_near_detail(&mut self) {
        let Some(landscape) = self.landscape.clone() else {
            self.near = None;
            return;
        };
        let main_vpm = self.world.voxels_per_meter;
        let fine_vpm = self.near_detail;
        if fine_vpm <= main_vpm || fine_vpm % main_vpm != 0 {
            self.near = None;
            return;
        }
        let scale = fine_vpm / main_vpm;

        if self.near_build.as_ref().is_some_and(|build| build.job.is_finished()) {
            let build = self.near_build.take().unwrap();
            let world = build.job.join().unwrap();
            // Unless the world was regenerated or the detail changed in the meantime
            if Arc::ptr_eq(&build.landscape, &landscape) && world.voxels_per_meter == fine_vpm {
                let set = get_voxel_set(
                    self.memory_allocator.clone(),
                    self.command_buffer_allocator.clone(),
                    self.descriptor_set_allocator.clone(),
                    &self.render_pipeline,
                    &self.queue,
                    2,
                    &world,
                );
                self.near = Some(NearDetail {
                    set,
                    origin: Vector3::from(world.origin).cast::<f64>() / scale as f64,
                    scale: scale as f64,
                    size: world.size,
                    center: build.center,
                });
                self.reset_accum = true;
            }
        }

        let camera = camera_to_voxels(self.camera.position, self.world.size);
        let far = NEAR_REBUILD_M * main_vpm as f64;
        let moved = self.near.as_ref().is_none_or(|near| {
            let offset = camera - near.center;
            offset.xy().norm() > far || offset.z.abs() > far * 0.6
        });
        if moved && self.near_build.is_none() {
            let size = [NEAR_WIDTH_M * fine_vpm, NEAR_WIDTH_M * fine_vpm, NEAR_HEIGHT_M * fine_vpm];
            let center = camera * scale as f64;
            // Brick aligned, and within the height of the world
            let corner = |axis: usize| ((center[axis] - size[axis] as f64 * 0.5) / 8.0).round() as i64 * 8;
            let top = self.world.size[2] as i64 * scale as i64 - size[2] as i64;
            let origin = [corner(0), corner(1), corner(2).clamp(0, top.max(0))];
            let job_landscape = landscape.clone();
            let job = std::thread::spawn(move || {
                let start = Instant::now();
                let world = job_landscape.build(origin, size, fine_vpm);
                println!("Built fine detail: {} bricks ({:?})", world.brick_count(), start.elapsed());
                world
            });
            self.near_build = Some(NearBuild {
                job,
                landscape,
                center: camera,
            });
        }
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        if self.render_pipeline.maybe_reload() {
            self.reset_accum = true;
            self.sun_map_for = None;
        }
        self.post_pipeline.maybe_reload();
        self.resample_pipeline.maybe_reload();
        self.update_near_detail();

        let rcx = self.rcx.as_mut().unwrap();

        if self.input.window_resized().is_some() {
            rcx.recreate_swapchain = true;
        }

        let window_size = rcx.window.inner_size();

        if window_size.width == 0 || window_size.height == 0 {
            return;
        }

        rcx.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if rcx.recreate_swapchain {
            let images;
            (rcx.swapchain, images) = rcx
                .swapchain
                .recreate(SwapchainCreateInfo {
                    image_extent: window_size.into(),
                    ..rcx.swapchain.create_info()
                })
                .unwrap();

            rcx.image_views = images
                .iter()
                .map(|i| ImageView::new(i.clone(), ImageViewCreateInfo::from_image(i)).unwrap())
                .collect();

            rcx.recreate_swapchain = false;
            rcx.recreate_images = true;
        }

        // Lower resolution while the view changes, full detail once it settles
        let scale = if self.still_frames >= 2 { self.still_render_scale } else { self.render_scale };
        if rcx.recreate_images || scale != rcx.render_scale_in_use {
            let window_extent: [u32; 2] = window_size.into();
            let render_extent = [
                ((window_extent[0] as f32 * scale) as u32).max(1),
                ((window_extent[1] as f32 * scale) as u32).max(1),
            ];
            rcx.images = get_render_images(
                self.memory_allocator.clone(),
                self.descriptor_set_allocator.clone(),
                &self.render_pipeline,
                &self.post_pipeline,
                &self.resample_pipeline,
                &self.sun_map,
                render_extent,
                window_extent,
            );

            rcx.render_scale_in_use = scale;
            rcx.recreate_images = false;
            self.reset_accum = true;
        }

        let (image_index, suboptimal, acquire_future) =
            match acquire_next_image(rcx.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    rcx.recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };

        if suboptimal {
            rcx.recreate_swapchain = true;
        }

        rcx.gui.immediate_ui(|gui| {
            let ctx = gui.context();

            egui::Window::new("Settings").show(&ctx, |ui| {
                ui.style_mut().spacing.slider_width = 250.0;
                ui.horizontal(|ui| {
                    for &mode in RenderMode::ALL {
                        ui.selectable_value(&mut self.render_mode, mode, format!("{:?}", mode));
                    }
                });
                ui.label(if self.player.is_some() {
                    "Walking: WASD to move, Space to jump or swim up, Shift to run, F to fly"
                } else {
                    "Flying: WASDQE to move, Shift to go faster, F to walk"
                });
                ui.separator();
                ui.add(egui::Slider::new(&mut self.camera.fov, 0.0..=180.0).text("FOV"));
                ui.add(egui::Slider::new(&mut self.render_scale, 0.125..=2.0).text("Render Scale While Moving"));
                ui.add(egui::Slider::new(&mut self.still_render_scale, 0.125..=2.0).text("Render Scale When Still"));
                ui.add(
                    egui::Slider::new(&mut self.move_speed, 0.01..=4.0)
                        .logarithmic(true)
                        .text("Move Speed (hold Shift to go faster)"),
                );

                egui::CollapsingHeader::new("Lighting")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.add(egui::Slider::new(&mut self.sun_azimuth, 0.0..=360.0).text("Sun Azimuth"));
                        ui.add(egui::Slider::new(&mut self.sun_elevation, -10.0..=90.0).text("Sun Elevation"));
                        ui.checkbox(&mut self.shadows, "Shadows");
                        ui.checkbox(&mut self.ambient_occlusion, "Ambient Occlusion");
                        ui.checkbox(&mut self.bounce_light, "Bounce Light");
                        ui.add(egui::Slider::new(&mut self.haze, 0.0..=0.06).text("Haze"));
                        ui.checkbox(&mut self.grass, "Grass Blades");
                        ui.checkbox(&mut self.sunbeams, "Sunbeams Through the Haze");
                        ui.checkbox(&mut self.denoise, "Smooth Out Noise While Moving");
                        ui.checkbox(&mut self.color_grade, "Warm Color Grade");
                        ui.add(egui::Slider::new(&mut self.bloom, 0.0..=1.5).text("Glow"));
                    });

                egui::CollapsingHeader::new("World")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for &scene in Scene::ALL {
                                ui.selectable_value(&mut self.scene, scene, scene.name());
                            }
                        });

                        let mut can_generate = true;
                        if self.scene.is_terrain() {
                            ui.add(egui::DragValue::new(&mut self.terrain.seed).prefix("Seed: "));
                            ui.add(
                                egui::Slider::new(&mut self.terrain.width_m, 64..=1024)
                                    .step_by(32.0)
                                    .text("World Width (m)"),
                            );
                            ui.add(
                                egui::Slider::new(&mut self.terrain.height_m, 32..=256)
                                    .step_by(8.0)
                                    .text("World Height (m)"),
                            );
                            ui.horizontal(|ui| {
                                ui.label("Voxels per metre:");
                                for &v in VOXELS_PER_METER_OPTIONS {
                                    ui.selectable_value(&mut self.terrain.voxels_per_meter, v, v.to_string());
                                }
                            });

                            let size = self.terrain.size();
                            ui.label(format!(
                                "{}×{}×{} voxels, each {:.1} cm",
                                size[0],
                                size[1],
                                size[2],
                                100.0 / self.terrain.voxels_per_meter as f32
                            ));
                            let max_bricks = self.device.physical_device().properties().max_image_dimension3_d * 8;
                            if size.iter().any(|&n| n > max_bricks) {
                                can_generate = false;
                                ui.colored_label(Color32::LIGHT_RED, "This world is too big for your GPU's 3D image limit.");
                            }

                            ui.horizontal(|ui| {
                                ui.label("Finer voxels near you, per metre:");
                                for &v in NEAR_DETAIL_OPTIONS {
                                    let name = if v == 0 { "Off".to_string() } else { v.to_string() };
                                    if ui.selectable_value(&mut self.near_detail, v, name).changed() {
                                        self.near = None;
                                        self.reset_accum = true;
                                    }
                                }
                            });
                        } else {
                            ui.add(
                                egui::Slider::new(&mut self.future_voxel_resolution, 8..=3200)
                                .custom_formatter(|n, _| format!("{}", (n as u32).div_ceil(8) * 8))
                                .custom_parser(|s| s.parse::<u32>().ok().map(|n| n.div_ceil(8) * 8).map(|n| n as f64))
                                .text("Voxel Resolution")
                            );
                            self.future_voxel_resolution = self.future_voxel_resolution.div_ceil(8) * 8;
                            ui.colored_label(Color32::LIGHT_RED, "Warning: Setting voxel resolution too high might crash the program.");
                            ui.label("The maximum possible resolution will depend on your GPU's Vulkan limits. This could be mitigated by splitting the world into multiple textures but that's not included in this simple example.");
                        }

                        if ui.add_enabled(can_generate, egui::Button::new("Generate")).clicked() {
                            let (world, landscape) = build_world(self.scene, &self.terrain, self.future_voxel_resolution);

                            let max_bytes = self.device.physical_device().properties().max_storage_buffer_range as usize;
                            if world.bricks.len() * 4 > max_bytes {
                                self.world_error = Some(format!(
                                    "That world needs {} MB of bricks but your GPU allows {} MB. Try fewer voxels per metre or a smaller world.",
                                    world.bricks.len() * 4 / (1024 * 1024),
                                    max_bytes / (1024 * 1024)
                                ));
                            } else {
                                self.voxel_set = get_voxel_set(
                                    self.memory_allocator.clone(),
                                    self.command_buffer_allocator.clone(),
                                    self.descriptor_set_allocator.clone(),
                                    &self.render_pipeline,
                                    &self.queue,
                                    1,
                                    &world,
                                );
                                self.world = world;
                                self.landscape = landscape;
                                self.near = None;
                                self.world_error = None;
                                self.sun_map_for = None;

                                if self.scene.is_terrain() != self.loaded_scene.is_terrain() {
                                    self.camera = self.scene.initial_camera();
                                    self.move_speed = self.scene.initial_move_speed();
                                }
                                // Land on the new ground
                                if self.player.is_some() {
                                    self.player = Some(Player::on_dry_ground(camera_to_voxels(self.camera.position, self.world.size), &self.world));
                                }
                                self.loaded_scene = self.scene;
                                self.reset_accum = true;
                            }
                        }
                        if let Some(error) = &self.world_error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                    });
            });
            egui::Window::new("Stats").show(&ctx, |ui| {
                fn format_with_commas(n: u64) -> String {
                    let mut s = n.to_string();

                    let mut i = 3;
                    while i < s.len() {
                        s.insert(s.len() - i, ',');
                        i += 4;
                    }

                    s
                }

                ui.label(format!("FPS: {}", self.fps));

                let [sx, sy, sz] = self.world.size.map(u64::from);
                ui.label(format!(
                    "Voxels: {}×{}×{} = {}",
                    format_with_commas(sx),
                    format_with_commas(sy),
                    format_with_commas(sz),
                    format_with_commas(sx * sy * sz)
                ));
                ui.label(format!(
                    "Stored bricks: {} ({} MB)",
                    format_with_commas(self.world.brick_count() as u64),
                    self.world.bricks.len() * 4 / (1024 * 1024)
                ));
                let window_extent: [u32; 2] = rcx.window.inner_size().into();
                let render_extent = rcx.images.render_image.extent();
                ui.label(format!(
                    "Pixels: {}×{} = {}",
                    format_with_commas(window_extent[0] as u64),
                    format_with_commas(window_extent[1] as u64),
                    format_with_commas((window_extent[0] * window_extent[1]) as u64)
                ));
                ui.label(format!(
                    "Rays: {}×{} = {}",
                    format_with_commas(render_extent[0] as u64),
                    format_with_commas(render_extent[1] as u64),
                    format_with_commas((render_extent[0] * render_extent[1]) as u64)
                ));
                if self.render_mode == RenderMode::Shaded {
                    ui.label(format!("Samples per pixel: {}", self.accum_count + 1));
                }
            });
        });

        let render_extent = rcx.images.render_image.extent();
        let resample_extent = rcx.images.resample_image.extent();
        self.camera.extent = [render_extent[0] as f64, render_extent[1] as f64];

        let pixel_to_ray = self.camera.pixel_to_ray_matrix();

        // The largest side of the world spans one unit of camera space
        let size = Vector3::from(self.world.size).cast::<f64>();
        let scale = size.max();
        let mut scale_and_center = Matrix4::from_diagonal(&Vector4::new(scale, scale, scale, 1.0));
        scale_and_center.set_column(3, &(0.5 * size).push(1.0));
        let pixel_to_ray = scale_and_center * pixel_to_ray;

        let (azimuth, elevation) = (self.sun_azimuth.to_radians(), self.sun_elevation.to_radians());
        let sun_direction = [
            elevation.cos() * azimuth.cos(),
            elevation.cos() * azimuth.sin(),
            elevation.sin(),
            self.world.sea_level.unwrap_or(0.0),
        ];

        let mut flags = 0;
        if self.shadows {
            flags |= FLAG_SHADOWS;
        }
        if self.ambient_occlusion {
            flags |= FLAG_AO;
        }
        if self.bounce_light {
            flags |= FLAG_GI;
        }
        if self.sunbeams {
            flags |= FLAG_SUNBEAMS;
        }
        if self.grass && self.world.grass.is_some() {
            flags |= FLAG_GRASS;
        }

        let mut post_flags = 0;
        // An sRGB swapchain encodes on its own, otherwise the shader has to
        if rcx.swapchain.image_format().numeric_format_color() != Some(NumericFormat::SRGB) {
            post_flags |= POST_ENCODE_SRGB;
        }
        if self.color_grade {
            post_flags |= POST_GRADE;
        }
        if self.denoise {
            post_flags |= POST_DENOISE;
        }

        let view = ViewKey {
            pixel_to_ray: pixel_to_ray.cast(),
            sun_direction,
            render_mode: self.render_mode as u32,
            flags,
            haze: self.haze,
        };
        // Last frame can be reprojected onto this one as long as only the camera moved
        let history_limit = match &self.last_view {
            _ if self.reset_accum => 0,
            Some(last) if *last == view => STILL_HISTORY,
            Some(last) if (last.sun_direction, last.render_mode, last.flags, last.haze) == (view.sun_direction, view.render_mode, view.flags, view.haze) => MOVING_HISTORY,
            _ => 0,
        };
        self.reset_accum = false;
        self.accum_count = if history_limit == STILL_HISTORY { (self.accum_count + 1).min(STILL_HISTORY) } else { 0 };
        let previous_world_to_pixel = self
            .last_view
            .as_ref()
            .map_or(Matrix4::identity(), |last| world_to_pixel(&last.pixel_to_ray.cast()));

        #[derive(BufferContents, Clone, Copy)]
        #[repr(C)]
        struct PushConstants {
            pixel_to_ray: Matrix4<f32>,
            previous_world_to_pixel: Matrix4<f32>,
            world_size: [u32; 4],
            sun_direction: [f32; 4],
            render_mode: u32,
            flags: u32,
            frame: u32,
            history_limit: u32,
            near_origin: [f32; 4],
            near_size: [u32; 4],
            haze: [f32; 4],
        }
        let [sx, sy, sz] = self.world.size;
        let (near_origin, near_size, near_set) = match &self.near {
            Some(near) => (
                [near.origin.x as f32, near.origin.y as f32, near.origin.z as f32, near.scale as f32],
                [near.size[0], near.size[1], near.size[2], 0],
                near.set.clone(),
            ),
            None => ([0.0; 4], [0; 4], self.empty_near_set.clone()),
        };
        let push_constants = PushConstants {
            pixel_to_ray: view.pixel_to_ray,
            previous_world_to_pixel: previous_world_to_pixel.cast(),
            world_size: [sx, sy, sz, self.world.voxels_per_meter],
            sun_direction: view.sun_direction,
            render_mode: view.render_mode,
            flags: view.flags,
            frame: self.frame,
            history_limit,
            near_origin,
            near_size,
            haze: [view.haze, 0.0, 0.0, 0.0],
        };
        let parity = (self.frame % 2) as usize;
        let render_set = rcx.images.render_sets[parity].clone();
        self.last_view = Some(view);
        self.frame = self.frame.wrapping_add(1);

        let mut builder = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        builder
            .clear_color_image(ClearColorImageInfo::image(rcx.images.render_image.clone()))
            .unwrap();

        builder
            .bind_pipeline_compute(self.render_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.render_pipeline.layout().clone(),
                0,
                vec![render_set, self.voxel_set.clone(), near_set],
            )
            .unwrap();

        let shaded = self.render_mode == RenderMode::Shaded;
        // The sun map only changes with the sun or the world
        if shaded && self.sunbeams && self.haze > 0.0 && self.sun_map_for != Some(sun_direction) {
            let sun_map_constants = PushConstants {
                render_mode: SUN_MAP_MODE,
                ..push_constants
            };
            builder
                .push_constants(self.render_pipeline.layout().clone(), 0, sun_map_constants)
                .unwrap();
            unsafe {
                builder
                    .dispatch([SUN_MAP_SIZE.div_ceil(8), SUN_MAP_SIZE.div_ceil(8), 1])
                    .unwrap();
            }
            self.sun_map_for = Some(sun_direction);
        }

        builder
            .push_constants(self.render_pipeline.layout().clone(), 0, push_constants)
            .unwrap();
        unsafe {
            builder
                .dispatch([
                    render_extent[0].div_ceil(8),
                    render_extent[1].div_ceil(8),
                    1,
                ])
                .unwrap();
        }

        if shaded {
            let images = &rcx.images;
            let render_extent = [render_extent[0], render_extent[1]];
            let constants = |stage: PostStage, step: u32| PostConstants {
                stage: stage as u32,
                step,
                flags: post_flags,
                bloom: self.bloom,
            };
            for (i, set) in images.denoise_sets[parity].iter().enumerate() {
                let step = 1 << i;
                dispatch_post(&mut builder, &self.post_pipeline, set, constants(PostStage::Denoise, step), render_extent);
            }
            if self.bloom > 0.0 {
                dispatch_post(&mut builder, &self.post_pipeline, &images.bloom_extract_set, constants(PostStage::BloomExtract, 0), images.bloom_extent);
                dispatch_post(&mut builder, &self.post_pipeline, &images.bloom_blur_sets[0], constants(PostStage::BloomBlurX, 0), images.bloom_extent);
                dispatch_post(&mut builder, &self.post_pipeline, &images.bloom_blur_sets[1], constants(PostStage::BloomBlurY, 0), images.bloom_extent);
            }
            dispatch_post(&mut builder, &self.post_pipeline, &images.tonemap_set, constants(PostStage::Tonemap, 0), render_extent);
        }

        builder
            .bind_pipeline_compute(self.resample_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.resample_pipeline.layout().clone(),
                0,
                vec![rcx.images.resample_set.clone()],
            )
            .unwrap();

        unsafe {
            builder
                .dispatch([
                    resample_extent[0].div_ceil(8),
                    resample_extent[1].div_ceil(8),
                    1,
                ])
                .unwrap();
        }

        let mut info = BlitImageInfo::images(
            rcx.images.resample_image.clone(),
            rcx.image_views[image_index as usize].image().clone(),
        );
        info.filter = Filter::Nearest;
        builder.blit_image(info).unwrap();

        let mut capture_done = false;
        if let Some(capture) = &mut self.capture {
            let now = Instant::now();
            if let Some(last) = capture.last_frame {
                capture.frame_times.push(now - last);
            }
            capture.last_frame = Some(now);
            capture_done = capture.frame_times.len() + 1 >= capture.frames as usize;
        }

        // Screenshots are taken from the resampled image, so they leave out the GUI
        let screenshot_buffer = (self.screenshot_requested || capture_done).then(|| {
            let buffer = Buffer::new_slice::<u8>(
                self.memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                    ..Default::default()
                },
                resample_extent[0] as u64 * resample_extent[1] as u64 * 4,
            )
            .unwrap();
            builder
                .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                    rcx.images.resample_image.clone(),
                    buffer.clone(),
                ))
                .unwrap();
            buffer
        });

        let command_buffer = builder.build().unwrap();

        let render_future = rcx
            .previous_frame_end
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap();

        let gui_future = rcx
            .gui
            .draw_on_image(render_future, rcx.image_views[image_index as usize].clone());

        let frame_end = gui_future
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(rcx.swapchain.clone(), image_index),
            )
            .then_signal_fence_and_flush();
        match frame_end.map_err(Validated::unwrap) {
            Ok(frame_end) => {
                // Screenshots read the frame back, so it has to be finished first
                if screenshot_buffer.is_some() {
                    frame_end.wait(None).unwrap();
                }
                rcx.previous_frame_end = Some(frame_end.boxed());
            }
            Err(VulkanError::OutOfDate) => {
                rcx.recreate_swapchain = true;
                rcx.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => panic!("failed to flush frame: {e}"),
        }

        if let Some(buffer) = screenshot_buffer {
            let encoded = post_flags & POST_ENCODE_SRGB != 0;
            if capture_done && let Some(capture) = self.capture.take() {
                save_screenshot(&buffer, resample_extent, encoded, &capture.path);

                let mut times: Vec<f64> = capture.frame_times.iter().map(|t| t.as_secs_f64() * 1000.0).collect();
                times.sort_by(f64::total_cmp);
                if !times.is_empty() {
                    let average = times.iter().sum::<f64>() / times.len() as f64;
                    println!(
                        "{} frames: average {:.1} ms ({:.1} FPS), median {:.1} ms, fastest {:.1} ms, slowest {:.1} ms",
                        times.len(),
                        average,
                        1000.0 / average,
                        times[times.len() / 2],
                        times[0],
                        times[times.len() - 1]
                    );
                }
                event_loop.exit();
            } else {
                let seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let path = PathBuf::from("screenshots").join(format!("innert-{seconds}.png"));
                save_screenshot(&buffer, resample_extent, encoded, &path);
            }
            self.screenshot_requested = false;
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(INITIAL_WINDOW_RESOLUTION)
                        .with_window_icon(Some(load_icon(include_bytes!("../assets/icon.png"))))
                        .with_title("Innert"),
                )
                .unwrap(),
        );
        let surface = Surface::from_window(self.instance.clone(), window.clone()).unwrap();

        let (swapchain, images) = get_swapchain_images(&self.device, &surface, &window);
        let image_views = images
            .iter()
            .map(|i| ImageView::new(i.clone(), ImageViewCreateInfo::from_image(i)).unwrap())
            .collect::<Vec<_>>();

        let window_extent: [u32; 2] = window.inner_size().into();
        let render_extent = [
            (window_extent[0] as f32 * self.render_scale) as u32,
            (window_extent[1] as f32 * self.render_scale) as u32,
        ];
        let images = get_render_images(
            self.memory_allocator.clone(),
            self.descriptor_set_allocator.clone(),
            &self.render_pipeline,
            &self.post_pipeline,
            &self.resample_pipeline,
            &self.sun_map,
            render_extent,
            window_extent,
        );

        let gui = Gui::new(
            event_loop,
            surface,
            self.queue.clone(),
            swapchain.image_format(),
            GuiConfig {
                is_overlay: true,
                ..Default::default()
            },
        );

        let recreate_swapchain = false;

        self.rcx = Some(RenderContext {
            window,
            swapchain,
            image_views,

            images,

            gui,

            recreate_swapchain,
            recreate_images: false,
            render_scale_in_use: self.render_scale,

            previous_frame_end: Some(sync::now(self.device.clone()).boxed()),
        });
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        self.input.step();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        if !self.rcx.as_mut().unwrap().gui.update(&event) {
            self.input.process_window_event(&event);
        }

        if event == WindowEvent::RedrawRequested {
            self.render(event_loop);
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        self.input.process_device_event(&event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.input.end_step();
        self.update(event_loop);
        let rcx = self.rcx.as_mut().unwrap();
        rcx.window.request_redraw();
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app).unwrap();
}
