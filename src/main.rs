// No console window behind the game, except when debugging. Problems are shown in a box, see
// show_error().
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use egui_winit_vulkano::{
    Gui, GuiConfig,
    egui::{self, Align2, Color32, RichText},
};
use nalgebra::{Matrix3, Matrix4, Rotation3, UnitQuaternion, Vector2, Vector3, Vector4};
use std::path::PathBuf;
use std::{
    f64::consts::{FRAC_PI_2, TAU},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use vulkano::{
    Validated, Version, VulkanError, VulkanLibrary,
    buffer::{
        Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer,
        allocator::{SubbufferAllocator, SubbufferAllocatorCreateInfo},
    },
    command_buffer::{
        AutoCommandBufferBuilder, BlitImageInfo, BufferCopy, BufferImageCopy, ClearColorImageInfo, CommandBufferUsage,
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

mod animals;
mod audio;
mod camera;
mod combat;
mod game;
mod hot_reload;
mod leaves;
mod player;
mod voxelize;
mod water;
mod weapons;
mod worldgen;

use crate::animals::AnimalInstance;
use crate::camera::Camera;
use crate::game::Game;
use crate::hot_reload::HotReloadComputePipeline;
use crate::player::Player;
use crate::water::Water;
use crate::worldgen::{RiverCache, Terrain, TerrainSettings, World};

const INITIAL_VOXEL_RESOLUTION: u32 = 24;
// Every voxel is the same size, 1/16 m, in the world and everything in it, as in John Lin's
// engine. That leaves room for this much of the landscape at once.
const INITIAL_TERRAIN: TerrainSettings = TerrainSettings {
    width_m: 224,
    height_m: 64,
    voxels_per_meter: 16,
    seed: 1,
    grass_tufts: true,
};
const VOXELS_PER_METER_OPTIONS: &[u32] = &[1, 2, 4, 8, 16];
const INITIAL_WINDOW_RESOLUTION: PhysicalSize<u32> = PhysicalSize::new(960, 960);

// How many frames each pixel averages. While still, more stops improving the image noticeably.
// While moving, fewer keeps old frames from smearing across the new ones.
const STILL_HISTORY: u32 = 4096;
const MOVING_HISTORY: u32 = 12;
// While animals are about even a still view keeps changing, as their shadows move over the ground
const ANIMATED_HISTORY: u32 = 32;

// Has to match the FLAG_* constants in shaders\traverse.comp
const FLAG_SHADOWS: u32 = 1;
const FLAG_AO: u32 = 2;
const FLAG_GI: u32 = 4;
const FLAG_SUNBEAMS: u32 = 8;
const FLAG_GRASS: u32 = 16;
const FLAG_WATER: u32 = 32;
const FLAG_SMOOTH: u32 = 64;
const FLAG_WIND: u32 = 128;

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

/// The world in voxels, and for generated landscapes what it was built from, to build finer pieces
/// of later. A landscape is endless, and the world holds the piece of it whose corner is at
/// `origin`, in voxels from the corner of the whole landscape.
fn build_world(scene: Scene, terrain: &TerrainSettings, voxel_resolution: u32, origin: [i64; 2], rivers: &RiverCache) -> (World, Option<Arc<Terrain>>) {
    match scene {
        Scene::Terrain => {
            let (landscape, world) = worldgen::generate_terrain(terrain, origin, rivers);
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
    // The fine voxels, already uploaded to the GPU
    job: std::thread::JoinHandle<(World, Arc<DescriptorSet>)>,
    landscape: Arc<Terrain>,
    center: Vector3<f64>,
}

/// The next piece of the endless landscape, being built in the background, uploaded to the GPU
/// along with its water
struct WindowBuild {
    job: std::thread::JoinHandle<(Terrain, World, GpuWorld, Water)>,
}

// The world holds a piece of the endless landscape around the camera. Once the camera gets this
// far from its middle, in meters, the piece around it is built and moved over to.
const RECENTER_M: f64 = 30.0;
/// The most flames and crystals lighting things up at once, nearest the camera first
const MAX_LIGHTS: usize = 24;
/// Voxel faces the light cache can hold, LIGHT_CACHE_WORDS each
const LIGHT_CACHE_ENTRIES: u64 = 1 << 21;
/// Has to match CACHE_WORDS in shaders\traverse.comp
const LIGHT_CACHE_WORDS: u64 = 12;

// The box of fine detail around the camera, in meters. It gets rebuilt once the camera
// gets this far from where it was last built.
const NEAR_WIDTH_M: u32 = 40;
const NEAR_HEIGHT_M: u32 = 20;
const NEAR_REBUILD_M: f64 = 8.0;
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
    // upscale.comp's, by frame parity, and what it's gathered at the window's resolution, which
    // read and write by turns
    upscale_sets: [Arc<DescriptorSet>; 2],
    histories: [Arc<ImageView>; 2],
}

/// `histories` are upscale.comp's from before, kept if the window's the same size
#[allow(clippy::too_many_arguments)]
fn get_render_images(
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    render_pipeline: &ComputePipeline,
    post_pipeline: &ComputePipeline,
    resample_pipeline: &ComputePipeline,
    upscale_pipeline: &ComputePipeline,
    sun_map: &Arc<ImageView>,
    render_extent: [u32; 2],
    window_extent: [u32; 2],
    histories: Option<[Arc<ImageView>; 2]>,
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

    let histories = histories
        .filter(|h| h[0].image().extent()[..2] == window_extent[..])
        .unwrap_or_else(|| [0, 1].map(|_| float_image(window_extent, Format::R16G16B16A16_SFLOAT)));
    let layout = upscale_pipeline.layout().set_layouts()[0].clone();
    let upscale_sets = [0, 1].map(|parity| {
        DescriptorSet::new(
            descriptor_set_allocator.clone(),
            layout.clone(),
            [
                WriteDescriptorSet::image_view(0, render_image_view.clone()),
                WriteDescriptorSet::image_view(1, depths[parity].clone()),
                WriteDescriptorSet::image_view(2, guide.clone()),
                WriteDescriptorSet::image_view(3, histories[1 - parity].clone()),
                WriteDescriptorSet::image_view(4, histories[parity].clone()),
                WriteDescriptorSet::image_view(5, resample_image_view.clone()),
            ],
            [],
        )
        .unwrap()
    });

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
        upscale_sets,
        histories,
    }
}

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct UpscaleConstants {
    pixel_to_ray: Matrix4<f32>,
    previous_world_to_pixel: Matrix4<f32>,
    frame: u32,
    flags: u32,
    most: f32,
    padding: f32,
}

// Has to match upscale.comp
const UPSCALE_HISTORY: u32 = 1;

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

/// What a footstep on `material` sounds like
fn ground_of(material: u8) -> audio::Ground {
    use worldgen::material::*;
    match material {
        SAND | SANDSTONE | SANDSTONE_DARK => audio::Ground::Sand,
        SNOW => audio::Ground::Snow,
        WATER | FOAM => audio::Ground::Water,
        STONE | ROCK | GRAVEL | CLAY | TEMPLE_STONE | TEMPLE_DARK | TEMPLE_RED | TEMPLE_TEAL | BRICK_STONE | MORTAR | LIMESTONE | WOOD | DEAD_WOOD | CLAY_POT => audio::Ground::Stone,
        _ => audio::Ground::Grass,
    }
}

// Every model of animals, weapons and such, see Game::model_words()
fn model_buffer(memory_allocator: &Arc<StandardMemoryAllocator>, words: Vec<u32>) -> Subbuffer<[u32]> {
    Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        words,
    )
    .unwrap()
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

/// A world's voxels on the GPU
struct GpuWorld {
    set: Arc<DescriptorSet>,
    // Kept to change bricks in place, see App::blast()
    grid: Arc<Image>,
    bricks: Subbuffer<[u32]>,
    // How many bricks there's room for
    capacity: usize,
}

/// Room for this many more bricks in the main world than it starts with, for holes blown in it:
/// a solid brick that gets a hole in it has to be stored
const SPARE_BRICKS: usize = 32768;

// Uploads a world, for traverse.comp's descriptor set `set_index`: 1 for the main world, 2 for the fine detail
fn get_voxel_set(
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    render_pipeline: &ComputePipeline,
    queue: &Arc<Queue>,
    set_index: usize,
    world: &World,
) -> GpuWorld {
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
    // The main world has room for more bricks, for holes blown in it, see App::blast()
    let capacity = brick_words.len() / worldgen::BRICK_WORDS + if set_index == 1 { SPARE_BRICKS } else { 0 };
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
        (capacity * worldgen::BRICK_WORDS) as u64,
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
        WriteDescriptorSet::buffer(1, bricks_buffer.clone()),
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
    GpuWorld {
        set: DescriptorSet::new(descriptor_set_allocator.clone(), layout.clone(), writes, []).unwrap(),
        grid: image,
        bricks: bricks_buffer,
        capacity,
    }
}

// Hides the cursor and keeps it in the window while playing, so the mouse turns the camera
fn grab_cursor(window: &Window, grab: bool) {
    if grab {
        let _ = window
            .set_cursor_grab(CursorGrabMode::Confined)
            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
    } else {
        let _ = window.set_cursor_grab(CursorGrabMode::None);
    }
    window.set_cursor_visible(!grab);
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
    upscale_pipeline: HotReloadComputePipeline,
    water_pipeline: HotReloadComputePipeline,

    // Which parts of the world sunlight reaches, for sunbeams. Computed for this sun direction.
    sun_map: Arc<ImageView>,
    sun_map_for: Option<[f32; 4]>,

    voxel_set: GpuWorld,
    // Kept on the CPU too, for walking around in it
    world: World,
    // What a generated world was built from
    landscape: Option<Arc<Terrain>>,
    // Rivers traced so far, for building the next piece of the landscape
    rivers: Arc<RiverCache>,
    window_build: Option<WindowBuild>,
    // Flames and crystals in the world, in voxels of it: x, y, z, reach, then color, and 0 as they
    // stay put, see Light in shaders\traverse.comp
    lights: Vec<[f32; 8]>,
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
    // Each voxel lit as part of a smooth surface, as in John Lin's engine
    smooth_shading: bool,
    // Leaves and flowers swaying in the wind
    wind: bool,
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
    // World to window pixel last frame, for upscale.comp to find where things were, if it ran
    last_world_to_window: Option<Matrix4<f64>>,

    // Animals, food and what the player carries
    game: Game,
    animals_on: bool,
    // Every animal's model, and every weapon's, see Game::model_words()
    animal_models: Subbuffer<[u32]>,
    // Which of Game::models_version() they are
    models_version: u32,
    // Where the animals are this frame gets uploaded from here
    instance_allocator: SubbufferAllocator,
    // Light bounced onto each voxel face, gathered over frames, see LightCache in traverse.comp
    light_cache: Subbuffer<[u32]>,
    // What the light in it was gathered for. When that changes it's thrown away.
    light_cache_for: Option<([f32; 4], u32)>,
    // For things that move on their own, like waterfalls
    start_time: Instant,
    // Flowing water, simulated on the GPU
    water: Water,
    last_frame: Instant,
    // Where the player stood last update, for the wake they push through water
    last_feet: Option<Vector3<f64>>,
    wake: Option<([f64; 2], [f64; 2])>,
    // Set by scripts in screenshot runs, see run_script(): the player walks straight ahead
    scripted_walk: bool,
    // and the mouse buttons for the next frame
    script_controls: combat::Controls,
    // None without anything to play sound on, and in screenshot runs
    audio: Option<audio::Audio>,
    // Holes blown in the landscape: where, in meters from its corner, and how big. They're blown
    // again in each new piece of it the world moves on to.
    craters: Vec<([f64; 3], f64)>,
    // Meters walked since the last footstep
    step_distance: f64,
    // Which waterfall "Visit a Waterfall" went to last
    waterfall_index: usize,

    menu_open: bool,
    // Whether the game has started, so the menu says Resume instead of Play
    started: bool,
    show_settings: bool,
    inventory_open: bool,
    // Set from the menu, done at the start of the next frame
    generate_requested: bool,
    waterfall_requested: bool,

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
/// `--shots 10,50` also saves those frames, next to the last one with the frame number added to the
/// name, and `--script <name>` plays through one of App::run_script()'s scripts meanwhile.
struct Capture {
    path: PathBuf,
    frames: u32,
    shots: Vec<usize>,
    script: Option<String>,
    /// Seconds each frame moves the game on by, instead of however long it took, so shots land
    /// at known moments
    step: Option<f64>,
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
            shots: value("--shots").map_or(Vec::new(), |s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect()),
            script: value("--script").cloned(),
            step: value("--step").and_then(|n| n.parse().ok()),
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
        let library = VulkanLibrary::new()
            .unwrap_or_else(|e| fail(&format!("Vulkan isn't installed on this computer ({e}).\n\n{DRIVER_HINT}")));

        let mut required_extensions = Surface::required_extensions(event_loop)
            .unwrap_or_else(|e| fail(&format!("Can't draw to a window here ({e}).\n\n{DRIVER_HINT}")));

        // Without a driver that does Vulkan, it can't even draw to a window
        if !library.supported_extensions().contains(&required_extensions) {
            fail(&format!("This computer's graphics driver doesn't support Vulkan, or there isn't one installed.\n\n{DRIVER_HINT}"));
        }
        // For debugging tools, where they're there
        required_extensions.ext_debug_utils = library.supported_extensions().ext_debug_utils;

        let instance = Instance::new(
            library,
            InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: required_extensions,
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| fail(&format!("Couldn't start Vulkan ({e}).\n\n{DRIVER_HINT}")));

        let mut device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };

        let devices: Vec<_> = instance
            .enumerate_physical_devices()
            .unwrap_or_else(|e| fail(&format!("Couldn't find any graphics cards ({e}).\n\n{DRIVER_HINT}")))
            .collect();
        let found = devices
            .iter()
            .map(|p| format!("{} (Vulkan {})", p.properties().device_name, p.api_version()))
            .collect::<Vec<_>>()
            .join(", ");
        let (physical_device, queue_family_index) = devices
            .into_iter()
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
            .unwrap_or_else(|| {
                let found = if found.is_empty() { "none".to_string() } else { found };
                fail(&format!("None of the graphics cards here can run Innert. Found: {found}.\n\n{DRIVER_HINT}"))
            });

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );
        // Not a graphics card of its own, so slower: it renders fewer pixels to begin with
        let integrated = physical_device.properties().device_type != PhysicalDeviceType::DiscreteGpu;

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
        .unwrap_or_else(|e| fail(&format!("Couldn't start the graphics card ({e}).\n\n{DRIVER_HINT}")));

        let queue = queues.next().unwrap();

        let (memory_allocator, descriptor_set_allocator, command_buffer_allocator) =
            get_allocators(&device);

        // Read from where they are while working on them, so they reload when changed. Copies are
        // built in for running elsewhere.
        let shaders_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders");
        let shader = |name: &str, built_in: &'static str| {
            HotReloadComputePipeline::new(device.clone(), &shaders_dir.join(name), built_in)
        };
        let render_pipeline = shader("traverse.comp", include_str!("../shaders/traverse.comp"));
        let post_pipeline = shader("post.comp", include_str!("../shaders/post.comp"));
        let resample_pipeline = shader("resample.comp", include_str!("../shaders/resample.comp"));
        let upscale_pipeline = shader("upscale.comp", include_str!("../shaders/upscale.comp"));
        let water_pipeline = shader("water.comp", include_str!("../shaders/water.comp"));

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
        if let Some(v) = setting("width_m").and_then(|v| v.parse().ok()) {
            terrain.width_m = v;
        }
        if let Some(v) = setting("tufts") {
            terrain.grass_tufts = v == "true" || v == "1";
        }
        let rivers = Arc::new(RiverCache::default());
        // For trying out each kind of land: `--set biome=jungle` starts in the nearest jungle
        let start_in = setting("biome").and_then(|kind| worldgen::find_climate(&terrain, kind, [160.0, 160.0]));
        let origin = start_in.map_or([0, 0], |spot| {
            let (vpm, size) = (terrain.voxels_per_meter as f64, terrain.size());
            [0, 1].map(|i| ((spot[i] as f64 * vpm) as i64 - size[i] as i64 / 2).div_euclid(64) * 64)
        });
        let (world, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, origin, &rivers);
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
        )
        .set;

        let input = WinitInputHelper::new();

        let mut camera = scene.initial_camera();
        if start_in.is_some() {
            // Over the middle of the world, a little above the ground, looking out across it
            let [sx, sy, sz] = world.size.map(|n| n as i64);
            let ground = (0..sz).rev().find(|&z| !worldgen::is_passable(world.material_at(sx / 2, sy / 2, z))).unwrap_or(0);
            let eye = Vector3::new(sx as f64 * 0.5, sy as f64 * 0.5, ground as f64 + 4.0 * world.voxels_per_meter as f64);
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_direction(Vector3::new(1.0, 0.3, -0.25));
        }
        if let Some(position) = setting("camera").and_then(parse_vector) {
            camera.position = position;
        }
        if let Some(target) = setting("look_at").and_then(parse_vector) {
            camera.look_at(target);
        }
        let flag = |key: &str, default: bool| setting(key).map_or(default, |v| v == "true" || v == "1");

        // Start on foot, except for screenshot runs which need the camera to stay put
        let capture = Capture::from_args();
        let springs = landscape.as_ref().map_or(Vec::new(), |l| l.springs());
        let mut water = Water::new(&memory_allocator, &command_buffer_allocator, &descriptor_set_allocator, &queue, &water_pipeline, &world, &springs);
        water.active &= flag("water", true);
        let water_level = |x: f64, y: f64| water.surface_at(x, y);

        let walk = scene.is_terrain() && flag("walk", capture.is_none());
        let player = walk.then(|| Player::on_dry_ground(camera_to_voxels(camera.position, world.size), &world, &water_level));

        let start = player.as_ref().map_or(camera_to_voxels(camera.position, world.size), |p| p.position);
        let mut game = Game::new(&world, &water_level, terrain.seed, start);
        // For trying out weapons: `--set weapon_seed=7 weapon_luck=3` starts with other ones, rarer
        // the luckier, and `--set weapon=bow` with the bow in hand
        if let Some(seed) = setting("weapon_seed").and_then(|v| v.parse().ok()) {
            game.arm(seed, setting("weapon_luck").and_then(|v| v.parse().ok()).unwrap_or(1.0));
        }
        if let Some(family) = setting("weapon") {
            game.select_family(family);
        }
        if capture.is_some() {
            println!("{}", game.describe_weapons());
        }
        let models_version = game.models_version();
        let animal_models = model_buffer(&memory_allocator, game.model_words());
        // Screenshot runs are silent
        let audio = if capture.is_none() { audio::Audio::new() } else { None };
        let light_cache = Buffer::new_slice::<u32>(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
            LIGHT_CACHE_ENTRIES * LIGHT_CACHE_WORDS,
        )
        .unwrap();
        let instance_allocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::STORAGE_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );
        let menu_open = capture.is_none() && flag("menu", true);
        // `--set waterfall=2` starts looking at the second waterfall
        let visit_waterfall = setting("waterfall").and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0);

        let mut app = App {
            instance,
            device,
            queue,

            memory_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,

            render_pipeline,
            post_pipeline,
            resample_pipeline,
            upscale_pipeline,
            water_pipeline,

            sun_map,
            sun_map_for: None,

            voxel_set,
            player,
            world,
            landscape,
            rivers,
            window_build: None,
            lights: Vec::new(),
            // No finer voxels round the camera, so they're the same size all over
            near_detail: setting("near_detail").and_then(|v| v.parse().ok()).unwrap_or(0),
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
            render_scale: setting("render_scale").and_then(|v| v.parse().ok()).unwrap_or(if integrated { 0.35 } else { 0.5 }),
            still_render_scale: setting("still_render_scale").and_then(|v| v.parse().ok()).unwrap_or(if integrated { 0.45 } else { 0.6 }),
            still_frames: 0,
            last_camera_view: None,

            // Bright, clear daylight from ahead and to one side of the starting view, with little
            // haze, so the voxels read crisply. Grass is made of voxels rather than triangles.
            sun_azimuth: setting("sun_azimuth").and_then(|v| v.parse().ok()).unwrap_or(150.0),
            sun_elevation: setting("sun_elevation").and_then(|v| v.parse().ok()).unwrap_or(38.0),
            shadows: flag("shadows", true),
            ambient_occlusion: flag("ao", true),
            bounce_light: flag("bounce", true),
            smooth_shading: flag("smooth", true),
            wind: flag("wind", true),
            sunbeams: flag("sunbeams", false),
            denoise: flag("denoise", true),
            color_grade: flag("grade", true),
            bloom: setting("bloom").and_then(|v| v.parse().ok()).unwrap_or(0.3),
            haze: setting("haze").and_then(|v| v.parse().ok()).unwrap_or(0.0012),
            grass: flag("grass", false),

            frame: 0,
            accum_count: 0,
            last_view: None,
            reset_accum: true,
            last_world_to_window: None,

            game,
            animals_on: flag("animals", true),
            animal_models,
            models_version,
            instance_allocator,
            light_cache,
            light_cache_for: None,
            start_time: Instant::now(),
            water,
            last_frame: Instant::now(),
            last_feet: None,
            wake: None,
            scripted_walk: false,
            script_controls: combat::Controls::default(),
            audio,
            craters: Vec::new(),
            step_distance: 0.0,
            waterfall_index: 0,

            menu_open,
            started: !menu_open,
            show_settings: false,
            inventory_open: false,
            generate_requested: false,
            waterfall_requested: false,

            screenshot_requested: false,
            capture,

            input,
            focused: false,
            last_second: Instant::now(),
            frames_since_last_second: 0,
            fps: 0,

            rcx: None,
        };
        app.adopt_features();
        if let Some(n) = visit_waterfall {
            app.waterfall_index = n - 1;
            app.visit_waterfall();
        }
        // For checking how buildings and caves look: `--set view_light=3` looks at the third flame
        // or crystal nearest the start, from `--set view_distance=2.5` meters away
        if let Some(n) = setting("view_light").and_then(|v| v.parse::<usize>().ok()) {
            let distance = setting("view_distance").and_then(|v| v.parse::<f64>().ok()).unwrap_or(2.5);
            let start = camera_to_voxels(app.camera.position, app.world.size);
            let mut lights: Vec<Vector3<f64>> = app.lights.iter().map(|l| Vector3::new(l[0], l[1], l[2]).cast::<f64>()).collect();
            lights.sort_by(|a, b| (a - start).norm().total_cmp(&(b - start).norm()));
            if let Some(&target) = lights.get(n.saturating_sub(1)) {
                println!("looking at light {n} of {}", lights.len());
                app.view_from_near(target, distance);
            }
        }
        // For checking how buildings look: `--set view_site=pyramid` looks at the nearest pyramid from
        // outside, and `pyramid-inside` from inside its chamber
        if let Some(what) = setting("view_site")
            && let Some(landscape) = app.landscape.clone()
        {
            let (kind, inside) = what.strip_suffix("-inside").map_or((what, false), |k| (k, true));
            let vpm = app.world.voxels_per_meter as f64;
            let origin = Vector3::new(app.world.origin[0] as f64, app.world.origin[1] as f64, 0.0);
            let start = camera_to_voxels(app.camera.position, app.world.size);
            let to_voxels = |p: [f32; 3]| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64) * vpm - origin;
            let site = landscape
                .sites()
                .iter()
                .filter(|s| s.kind.replace(' ', "_") == kind)
                .min_by(|a, b| (to_voxels(a.position) - start).norm().total_cmp(&(to_voxels(b.position) - start).norm()));
            if let Some(site) = site {
                let middle = to_voxels(site.position);
                let facing = Vector3::new(site.facing[0] as f64, site.facing[1] as f64, 0.0);
                println!("looking at a {kind}, {:.0} m away", (middle - start).norm() / vpm);
                if inside {
                    let eye = middle + (facing * 2.8 + Vector3::new(0.0, 0.0, 1.9)) * vpm;
                    let target = middle + (facing * -3.2 + Vector3::new(0.0, 0.0, 1.0)) * vpm;
                    app.look_from(eye, target);
                } else {
                    app.view_from_near(middle + Vector3::new(0.0, 0.0, 5.0 * vpm), 30.0);
                }
            }
        }
        // For checking how an animal or a prop looks: `--set view_animal=bear` looks at the nearest one
        if let Some(name) = setting("view_animal")
            && let Some((position, height)) = app.game.find_animal(name, camera_to_voxels(app.camera.position, app.world.size))
        {
            let vpm = app.world.voxels_per_meter as f64;
            let target = position + Vector3::new(0.0, 0.0, height * vpm);
            let distance = setting("view_distance").and_then(|v| v.parse::<f64>().ok()).unwrap_or(4.0 * height + 0.6);
            app.view_from_near(target, distance);
        }
        // For checking how water splashes: `--set pour=4` tips 4 cubic meters out where the camera looks
        if let Some(cubic_meters) = setting("pour").and_then(|v| v.parse::<f64>().ok()) {
            let vpm = app.world.voxels_per_meter as f64;
            let eye = camera_to_voxels(app.camera.position, app.world.size);
            let look = (app.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
            if let Some((at, _)) = app.water.raycast(&app.world, eye, look, 200.0 * vpm) {
                app.water.pour(at.x, at.y, 1.0 * vpm, cubic_meters * vpm.powi(3));
            }
        }
        app
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if let Some(rcx) = &self.rcx {
            grab_cursor(&rcx.window, focused);
        }
    }

    /// Takes the lights and props of the landscape the world was just built from, in its voxels
    fn adopt_features(&mut self) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let to_voxels = |p: [f32; 3]| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64) * vpm - origin;
        let Some(landscape) = &self.landscape else {
            self.lights.clear();
            self.game.set_props(Vec::new());
            self.game.set_chests(Vec::new());
            return;
        };
        self.lights = landscape
            .lights()
            .iter()
            .map(|l| {
                let p = to_voxels(l.position);
                [p.x as f32, p.y as f32, p.z as f32, (l.reach as f64 * vpm) as f32, l.color[0], l.color[1], l.color[2], 0.0]
            })
            .collect();
        let (chests, props): (Vec<&worldgen::Prop>, Vec<&worldgen::Prop>) = landscape.props().iter().partition(|p| p.kind == worldgen::PropKind::Chest);
        self.game.set_props(props.iter().map(|p| (animals::Look::of_prop(p.kind), to_voxels(p.position), p.heading as f64)).collect());
        self.game.set_chests(
            chests
                .iter()
                .map(|p| game::Chest {
                    position: to_voxels(p.position),
                    heading: p.heading as f64,
                    // Where it is in the whole landscape, to the decimeter
                    key: p.position.map(|v| (v * 10.0).round() as i64),
                    luck: p.luck as f64,
                })
                .collect(),
        );
    }

    /// Blows a hole in the world `radius` meters round `center`, in voxels of it: the voxels go,
    /// the GPU's copy of them changes in place, water can flow into the hole and bits of what was
    /// there fly out. It's remembered for when the world moves on to another piece of the
    /// landscape and back.
    fn blast(&mut self, center: Vector3<f64>, radius: f64) {
        let vpm = self.world.voxels_per_meter as f64;
        let (changed, blown) = self.world.carve(center.into(), radius * vpm);
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        self.craters.push((((center + origin) / vpm).into(), radius));
        if changed.is_empty() {
            return;
        }
        self.game.debris(center, &blown, vpm);

        // The frames still being drawn read what's about to change, so let them finish
        if let Some(rcx) = &mut self.rcx {
            rcx.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
        }
        if self.world.bricks.len() / worldgen::BRICK_WORDS > self.voxel_set.capacity {
            // Out of room for more bricks: upload the whole world again, with room to spare
            self.voxel_set = get_voxel_set(
                self.memory_allocator.clone(),
                self.command_buffer_allocator.clone(),
                self.descriptor_set_allocator.clone(),
                &self.render_pipeline,
                &self.queue,
                1,
                &self.world,
            );
        } else {
            self.upload_bricks(&changed);
        }
        let reach = (radius * vpm).ceil() as i64 + 2;
        let (x, y) = (center.x as i64, center.y as i64);
        self.water.sync_ground(&self.world, [x - reach, y - reach], [x + reach + 1, y + reach + 1], &self.memory_allocator, &self.command_buffer_allocator, &self.queue);
    }

    /// Sends the brick grid `cells` of the main world to the GPU, along with their bricks
    fn upload_bricks(&mut self, cells: &[usize]) {
        let [sx, sy, _] = self.world.size;
        let (bx, by) = (sx as usize / 8, sy as usize / 8);
        let grid = self.voxel_set.grid.clone();
        let mut data: Vec<u32> = cells.iter().map(|&c| self.world.brick_grid[c]).collect();
        let grid_regions: Vec<BufferImageCopy> = cells
            .iter()
            .enumerate()
            .map(|(k, &c)| BufferImageCopy {
                buffer_offset: (k * 4) as u64,
                image_subresource: grid.subresource_layers(),
                image_offset: [(c % bx) as u32, ((c / bx) % by) as u32, (c / (bx * by)) as u32],
                image_extent: [1, 1, 1],
                ..Default::default()
            })
            .collect();
        let mut brick_regions = Vec::new();
        for &c in cells {
            if let Some((index, words)) = self.world.brick_words(c) {
                brick_regions.push(BufferCopy {
                    src_offset: (data.len() * 4) as u64,
                    dst_offset: (index * worldgen::BRICK_WORDS * 4) as u64,
                    size: (worldgen::BRICK_WORDS * 4) as u64,
                    ..Default::default()
                });
                data.extend_from_slice(words);
            }
        }
        let staging = Buffer::from_iter(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            data,
        )
        .unwrap();
        let mut builder = AutoCommandBufferBuilder::primary(self.command_buffer_allocator.clone(), self.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
        builder
            .copy_buffer_to_image(CopyBufferToImageInfo {
                regions: grid_regions.into(),
                ..CopyBufferToImageInfo::buffer_image(staging.clone(), grid)
            })
            .unwrap();
        if !brick_regions.is_empty() {
            builder
                .copy_buffer(CopyBufferInfo {
                    regions: brick_regions.into(),
                    ..CopyBufferInfo::buffers(staging, self.voxel_set.bricks.clone())
                })
                .unwrap();
        }
        builder.build().unwrap().execute(self.queue.clone()).unwrap().then_signal_fence_and_flush().unwrap().wait(None).unwrap();
    }

    /// The wind, the water nearby and the birds in the trees, louder or quieter for where the
    /// camera is
    fn ambience(&mut self, dt: f64) {
        if self.audio.is_none() {
            return;
        }
        let view = self.view();
        let vpm = view.vpm;
        let eye = view.eye;
        let outdoors = self.loaded_scene.is_terrain() && !self.menu_open;
        // How much of what's around is water, what's nearest counting most
        let (mut wet, mut all) = (0.0, 0.0);
        for ring in [2.0, 5.0, 9.0, 14.0] {
            for k in 0..12 {
                let a = k as f64 / 12.0 * TAU;
                let (x, y) = (eye.x + a.cos() * ring * vpm, eye.y + a.sin() * ring * vpm);
                all += 1.0 / ring;
                if self.water.depth_at(x, y) > 0.1 * vpm {
                    wet += 1.0 / ring;
                }
            }
        }
        // Windier high up
        let height = (eye.z - self.world.sea_level.unwrap_or(0.0) as f64) / vpm;
        let wind = 0.45 + 0.55 * ((height - 15.0) / 30.0).clamp(0.0, 1.0);
        let birds = outdoors && self.sun_elevation > 5.0 && self.game.leaves_falling() > 4;
        let quiet = if outdoors { 1.0 } else { 0.35 };
        if let Some(sound) = &mut self.audio {
            sound.ambience(wind as f32 * quiet, ((wet / all) as f32 * 1.6).min(1.0) * quiet, birds, &view, dt);
        }
    }

    /// Where the camera is and which way it looks, for placing what's held and aiming
    fn view(&self) -> combat::View {
        let rotation = self.camera.rotation_matrix().fixed_view::<3, 3>(0, 0).into_owned();
        let extent = self.camera.extent;
        let aspect = extent[0] / extent[1].max(1.0);
        let tan = (self.camera.fov.to_radians() * 0.5).tan();
        combat::View {
            eye: camera_to_voxels(self.camera.position, self.world.size),
            rotation: UnitQuaternion::from_rotation_matrix(&Rotation3::from_matrix_unchecked(rotation)),
            tan: [tan * aspect.max(1.0), tan / aspect.min(1.0)],
            vpm: self.world.voxels_per_meter as f64,
        }
    }

    /// Once the camera has wandered far enough from the middle of the world, builds the piece of
    /// the landscape around it in the background, and moves everything over to it once it's ready
    fn update_window(&mut self) {
        if self.landscape.is_none() {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let [sx, sy, _] = self.world.size;

        if self.window_build.as_ref().is_some_and(|build| build.job.is_finished()) {
            let build = self.window_build.take().unwrap();
            let (terrain, world, voxel_set, water) = build.job.join().unwrap();
            self.move_to(Arc::new(terrain), world, voxel_set, water);
            return;
        }

        let camera = camera_to_voxels(self.camera.position, self.world.size);
        let middle = Vector3::new(sx as f64 * 0.5, sy as f64 * 0.5, camera.z);
        if self.window_build.is_some() || (camera - middle).xy().norm() < RECENTER_M * vpm {
            return;
        }
        // Centered on the camera, lined up with the blocks the water is simulated in
        let align = 64;
        let origin = [
            (self.world.origin[0] + camera.x as i64 - sx as i64 / 2).div_euclid(align) * align,
            (self.world.origin[1] + camera.y as i64 - sy as i64 / 2).div_euclid(align) * align,
        ];
        // The same landscape, whatever's changed in the settings since
        let settings = TerrainSettings {
            width_m: self.world.size[0] / self.world.voxels_per_meter,
            height_m: self.world.size[2] / self.world.voxels_per_meter,
            voxels_per_meter: self.world.voxels_per_meter,
            ..self.terrain
        };
        let rivers = self.rivers.clone();
        let (memory_allocator, command_buffer_allocator, descriptor_set_allocator, queue) = (
            self.memory_allocator.clone(),
            self.command_buffer_allocator.clone(),
            self.descriptor_set_allocator.clone(),
            self.queue.clone(),
        );
        let (render_pipeline, water_pipeline) = ((*self.render_pipeline).clone(), (*self.water_pipeline).clone());
        let craters = self.craters.clone();
        let job = std::thread::spawn(move || {
            let (terrain, mut world) = worldgen::generate_terrain(&settings, origin, &rivers);
            // The holes blown in the landscape are still there
            let vpm = world.voxels_per_meter as f64;
            for (center, radius) in craters {
                let local = [center[0] * vpm - world.origin[0] as f64, center[1] * vpm - world.origin[1] as f64, center[2] * vpm];
                let reach = radius * vpm + 2.0;
                if local[0] > -reach && local[1] > -reach && local[0] < world.size[0] as f64 + reach && local[1] < world.size[1] as f64 + reach {
                    world.carve(local, radius * vpm);
                }
            }
            let voxel_set = get_voxel_set(
                memory_allocator.clone(),
                command_buffer_allocator.clone(),
                descriptor_set_allocator.clone(),
                &render_pipeline,
                &queue,
                1,
                &world,
            );
            let water = Water::new(
                &memory_allocator,
                &command_buffer_allocator,
                &descriptor_set_allocator,
                &queue,
                &water_pipeline,
                &world,
                &terrain.springs(),
            );
            (terrain, world, voxel_set, water)
        });
        self.window_build = Some(WindowBuild { job });
    }

    /// Moves over to a new piece of the landscape. Everything in the world moves with it.
    /// `voxel_set` and `water` are for `world`, already uploaded
    fn move_to(&mut self, landscape: Arc<Terrain>, world: World, voxel_set: GpuWorld, mut water: Water) {
        let started = Instant::now();
        let shift = Vector3::new(
            (world.origin[0] - self.world.origin[0]) as f64,
            (world.origin[1] - self.world.origin[1]) as f64,
            0.0,
        );
        self.voxel_set = voxel_set;
        water.active &= self.water.active;
        // The water flows on where the two overlap, once the GPU has finished with the old one
        if let Some(rcx) = &mut self.rcx {
            // Dropping a frame's future waits for it to finish
            rcx.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
        }
        water.carry_over(&self.water, [shift.x as i64, shift.y as i64], &self.command_buffer_allocator, &self.queue);
        self.water = water;

        let camera = camera_to_voxels(self.camera.position, self.world.size) - shift;
        self.world = world;
        self.landscape = Some(landscape);
        self.camera.position = voxels_to_camera(camera, self.world.size);
        if let Some(player) = &mut self.player {
            player.position -= shift;
        }
        self.last_feet = None;
        self.near = None;
        self.near_build = None;
        self.sun_map_for = None;
        self.reset_accum = true;
        self.adopt_features();
        let water = &self.water;
        let near = self.player.as_ref().map_or(camera, |p| p.position);
        self.game.recenter(&self.world, &|x, y| water.surface_at(x, y), shift, self.terrain.seed, near);
        println!("Moved over to the new piece of landscape in {:?}", started.elapsed());
    }

    /// Builds the world picked in the settings, in place of the current one
    fn generate_world(&mut self) {
        // A new landscape has rivers of its own
        let rivers = Arc::new(RiverCache::default());
        let (world, landscape) = build_world(self.scene, &self.terrain, self.future_voxel_resolution, [0, 0], &rivers);
        self.window_build = None;
        self.craters.clear();

        let max_bytes = self.device.physical_device().properties().max_storage_buffer_range as usize;
        if world.bricks.len() * 4 > max_bytes {
            self.world_error = Some(format!(
                "That world needs {} MB of bricks but your GPU allows {} MB. Try fewer voxels per metre or a smaller world.",
                world.bricks.len() * 4 / (1024 * 1024),
                max_bytes / (1024 * 1024)
            ));
            return;
        }
        self.voxel_set = get_voxel_set(
            self.memory_allocator.clone(),
            self.command_buffer_allocator.clone(),
            self.descriptor_set_allocator.clone(),
            &self.render_pipeline,
            &self.queue,
            1,
            &world,
        );
        let springs = landscape.as_ref().map_or(Vec::new(), |l| l.springs());
        self.water = Water::new(
            &self.memory_allocator,
            &self.command_buffer_allocator,
            &self.descriptor_set_allocator,
            &self.queue,
            &self.water_pipeline,
            &world,
            &springs,
        );
        self.world = world;
        self.landscape = landscape;
        self.rivers = rivers;
        self.light_cache_for = None;
        self.near = None;
        self.world_error = None;
        self.sun_map_for = None;
        self.adopt_features();

        if self.scene.is_terrain() != self.loaded_scene.is_terrain() {
            self.camera = self.scene.initial_camera();
            self.move_speed = self.scene.initial_move_speed();
        }
        let water = &self.water;
        let water_level = |x: f64, y: f64| water.surface_at(x, y);
        // Land on the new ground
        if self.player.is_some() {
            self.player = Some(Player::on_dry_ground(camera_to_voxels(self.camera.position, self.world.size), &self.world, &water_level));
        }
        if self.scene.is_terrain() {
            let start = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
            self.game.populate(&self.world, &water_level, self.terrain.seed, start);
        } else {
            self.game.clear();
        }
        self.loaded_scene = self.scene;
        self.reset_accum = true;
    }

    /// Flies over to look at one of the waterfalls, a different one each time
    fn visit_waterfall(&mut self) {
        let Some(landscape) = &self.landscape else {
            return;
        };
        let falls = landscape.waterfalls();
        if falls.is_empty() {
            return;
        }
        let (top, flow) = falls[self.waterfall_index % falls.len()];
        self.waterfall_index += 1;
        let vpm = self.world.voxels_per_meter as f64;
        let target = Vector3::new(top[0] as f64, top[1] as f64, top[2] as f64 - 1.5) * vpm;
        let flow = flow.map(f64::from);
        let heading = flow[1].atan2(flow[0]);
        // Downstream of it, somewhere in the open with nothing in the way
        let mut eye = target + Vector3::new(flow[0] * 9.0, flow[1] * 9.0, 3.0) * vpm;
        'search: for distance in [9.0, 12.0, 7.0, 15.0] {
            for turn in [0.0, 0.4, -0.4, 0.8, -0.8, 1.2, -1.2] {
                for height in [2.5, 4.0, 6.0] {
                    let angle = heading + turn;
                    let candidate = target + Vector3::new(angle.cos() * distance, angle.sin() * distance, height) * vpm;
                    if self.clear_view(candidate, target) {
                        eye = candidate;
                        break 'search;
                    }
                }
            }
        }
        self.look_from(eye, target);
    }

    /// Fills the bucket from the water the camera looks at
    fn scoop_water(&mut self, eye: Vector3<f64>, look: Vector3<f64>) {
        let vpm = self.world.voxels_per_meter as f64;
        let Some((at, true)) = self.water.raycast(&self.world, eye, look, game::REACH * vpm) else {
            self.game.say("Click on water to fill the bucket.");
            return;
        };
        let radius = 1.2 * vpm;
        let available = self.water.volume_near(at.x, at.y, radius) / vpm.powi(3);
        let taken = self.game.bucket_room().min(available * 0.8);
        if taken > 0.01 {
            self.water.pour(at.x, at.y, radius, -taken * vpm.powi(3));
            self.game.fill_bucket(taken);
        }
    }

    /// Tips the bucket out where the camera looks, or in front of the player
    fn pour_water(&mut self, eye: Vector3<f64>, look: Vector3<f64>) {
        let vpm = self.world.voxels_per_meter as f64;
        let at = self
            .water
            .raycast(&self.world, eye, look, game::REACH * vpm)
            .map_or(eye + look * 2.0 * vpm, |(p, _)| p);
        let water = self.game.empty_bucket();
        if water > 0.0 {
            self.water.pour(at.x, at.y, 1.5 * vpm, water * vpm.powi(3));
        }
    }

    /// Whether there's nothing at all between two points in voxels, not even leaves or water,
    /// other than right by the second
    fn clear_view(&self, from: Vector3<f64>, to: Vector3<f64>) -> bool {
        let vpm = self.world.voxels_per_meter as f64;
        let distance = (to - from).norm();
        let steps = (distance * 2.0) as usize;
        (0..steps).all(|i| {
            let p = from + (to - from) * (i as f64 / steps as f64);
            (p - to).norm() < 1.5 * vpm
                || self.world.material_at(p.x as i64, p.y as i64, p.z as i64) == worldgen::material::AIR
                    && p.z >= self.water.surface_at(p.x, p.y)
        })
    }

    /// Looks at `target` from about `distance` meters away, from wherever has a clear view of it
    fn view_from_near(&mut self, target: Vector3<f64>, distance: f64) {
        let vpm = self.world.voxels_per_meter as f64;
        let mut eye = target + Vector3::new(distance, 0.0, 0.5) * vpm;
        'search: for height in [0.3, 0.8, 1.5, 3.0] {
            for turn in 0..16 {
                let angle = turn as f64 / 16.0 * TAU;
                let candidate = target + Vector3::new(angle.cos() * distance, angle.sin() * distance, height * distance.max(1.0) / 2.5) * vpm;
                if self.clear_view(candidate, target) {
                    eye = candidate;
                    break 'search;
                }
            }
        }
        self.look_from(eye, target);
    }

    /// Puts the camera at `eye` looking at `target`, both in voxels, flying so it stays there
    fn look_from(&mut self, eye: Vector3<f64>, target: Vector3<f64>) {
        self.camera.position = voxels_to_camera(eye, self.world.size);
        self.camera.look_at(voxels_to_camera(target, self.world.size));
        self.player = None;
        self.reset_accum = true;
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
        let delta_time = self.capture.as_ref().and_then(|c| c.step).unwrap_or(delta_time);

        if self.input.close_requested() {
            event_loop.exit();
            return;
        }

        // Screenshot runs from the command line ignore input, so they always show the same view
        let interactive = self.capture.is_none();

        if interactive && self.input.key_pressed(KeyCode::Escape) {
            if self.inventory_open {
                self.inventory_open = false;
                self.set_focus(true);
            } else if self.menu_open && self.started {
                self.menu_open = false;
                self.set_focus(true);
            } else {
                self.menu_open = true;
                self.set_focus(false);
            }
        }
        let playing = interactive && !self.menu_open;
        if playing && (self.input.key_pressed(KeyCode::KeyI) || self.input.key_pressed(KeyCode::Tab)) {
            self.inventory_open = !self.inventory_open;
            self.set_focus(!self.inventory_open);
        }
        if playing && self.input.key_pressed(KeyCode::F1) {
            self.show_settings = !self.show_settings;
        }

        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        // Scripts fight by themselves, see run_script()
        let mut controls = std::mem::take(&mut self.script_controls);
        if playing && !self.focused && !self.inventory_open && self.input.mouse_pressed(MouseButton::Left) {
            self.set_focus(true);
        } else if playing && self.focused {
            let bucket = self.game.holding_bucket() && !self.game.has_target();
            let armed = self.game.holding_weapon();
            if armed {
                controls = combat::Controls {
                    attack: self.input.mouse_pressed(MouseButton::Left),
                    holding: self.input.mouse_held(MouseButton::Left),
                    heavy: self.input.mouse_pressed(MouseButton::Right),
                    aiming: self.input.mouse_held(MouseButton::Right),
                };
            } else if self.input.mouse_pressed(MouseButton::Left) {
                if bucket {
                    self.scoop_water(eye, look);
                } else {
                    self.game.primary(eye);
                }
            }
            if self.input.key_pressed(KeyCode::KeyE) {
                self.game.interact(eye);
            }
            if self.input.mouse_pressed(MouseButton::Right) && !armed {
                if self.game.holding_bucket() {
                    self.pour_water(eye, look);
                } else {
                    self.game.eat_selected();
                }
            }
            let digits = [
                KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5,
                KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9,
            ];
            if let Some(slot) = digits.iter().position(|&k| self.input.key_pressed(k)) {
                self.game.select(slot);
            }
            // On foot the wheel goes through what's carried, flying it zooms
            let wheel = self.input.scroll_diff().1;
            if self.focused && self.player.is_some() && wheel != 0.0 {
                self.game.scroll(if wheel < 0.0 { 1 } else { -1 });
            }
        }

        // The world stands still while the menu is open
        if !self.menu_open && self.animals_on && self.loaded_scene.is_terrain() {
            let feet = self.player.as_ref().map(|p| p.position);
            let view = self.view();
            let water = &self.water;
            self.game.update(&self.world, &|x, y| water.surface_at(x, y), feet, &view, controls, delta_time.min(0.1));
            let cues = self.game.take_cues();
            if let Some(sound) = &mut self.audio {
                for cue in cues {
                    sound.play(cue, &view);
                }
            }
            // Explosions blowing holes in the world
            for (center, radius) in self.game.take_craters() {
                self.blast(center, radius);
            }
        }
        self.ambience(delta_time);

        if playing && self.input.key_pressed(KeyCode::KeyF) {
            self.player = match self.player {
                Some(_) => None,
                None => Some(Player::new(camera_to_voxels(self.camera.position, self.world.size), &self.world)),
            };
        }

        let t = |k: KeyCode| (self.focused && interactive && self.input.key_held(k)) as u8 as f64;
        let mut footstep = None;
        if let Some(player) = &mut self.player {
            // Walk where the camera faces, ignoring whether it looks up or down
            let yaw = self.camera.rotation.z;
            let forward = Vector2::new(-yaw.sin(), yaw.cos());
            let right = Vector2::new(yaw.cos(), yaw.sin());
            let mut wish = forward * (t(KeyCode::KeyW) - t(KeyCode::KeyS)) + right * (t(KeyCode::KeyD) - t(KeyCode::KeyA));
            if self.scripted_walk {
                wish = forward;
            }
            if wish.norm() > 1.0 {
                wish.normalize_mut();
            }
            let jump = t(KeyCode::Space) > 0.0;
            let sprint = self.focused && self.input.held_shift();
            // Long hitches, like generating a world, shouldn't fling the player through the floor
            let water = &self.water;
            let before = player.position;
            player.update(&self.world, &|x, y| water.surface_at(x, y), wish, jump, sprint, delta_time.min(0.05));
            self.camera.position = voxels_to_camera(player.eye(&self.world), self.world.size);

            // Wading through water pushes it along
            let vpm = self.world.voxels_per_meter as f64;
            let feet = player.position;
            let moved = self.last_feet.map_or(Vector3::zeros(), |last| (feet - last) / delta_time.max(1e-3));
            self.last_feet = Some(feet);
            let wading = self.water.depth_at(feet.x, feet.y) > 0.1 * vpm;
            self.wake = (wading && !self.menu_open).then_some(([feet.x, feet.y], [moved.x, moved.y]));

            // A footstep every stride, sounding like whatever's underfoot
            if player.on_ground() {
                self.step_distance += (feet - before).xy().norm() / vpm;
                if self.step_distance > if sprint { 0.95 } else { 0.7 } {
                    self.step_distance = 0.0;
                    let under = self.world.material_at(feet.x.floor() as i64, feet.y.floor() as i64, feet.z.floor() as i64 - 1);
                    footstep = Some(if self.water.depth_at(feet.x, feet.y) > 0.05 * vpm { audio::Ground::Water } else { ground_of(under) });
                }
            }
        } else {
            self.last_feet = None;
            self.wake = None;
            if self.focused && interactive {
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
        }
        if let Some(ground) = footstep {
            let view = self.view();
            if let Some(sound) = &mut self.audio {
                sound.play(audio::Cue::here(audio::Sound::Step(ground), 0.35), &view);
            }
        }

        if self.focused && interactive {
            let sens = 0.001 * (self.camera.fov.to_radians() * 0.5).tan();

            let (dx, dy) = self.input.mouse_diff();
            self.camera.rotation.z -= dx as f64 * sens;
            self.camera.rotation.x -= dy as f64 * sens;
            self.camera.rotation.x = self.camera.rotation.x.clamp(-FRAC_PI_2, FRAC_PI_2);
            self.camera.rotation.y = self.camera.rotation.y.rem_euclid(TAU);

            if self.player.is_none() {
                let ds = self.input.scroll_diff();
                let tanfov = (self.camera.fov.to_radians() * 0.5).tan();
                self.camera.fov = ((tanfov * (ds.1 as f64 * -0.1).exp()).atan() * 2.0).to_degrees();
            }
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

        if self.input.key_pressed(KeyCode::F12) {
            self.screenshot_requested = true;
        }
        self.run_script();
    }

    /// Plays through things that need input, for checking them in screenshot runs: `--script bucket`
    /// scoops up water and pours it back, `wade` walks through a lake, `hunt` hits a squirrel,
    /// `fight` attacks with each weapon and `loot` opens a chest. What happens is printed.
    fn run_script(&mut self) {
        let Some(capture) = &self.capture else {
            return;
        };
        let Some(script) = capture.script.clone() else {
            return;
        };
        let frame = capture.frame_times.len();
        let vpm = self.world.voxels_per_meter as f64;
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        let before = self.game.last_message().map(str::to_owned);
        match (script.as_str(), frame) {
            ("bucket", 1) => self.stand_in_water(0.6, false),
            ("wade", 1) => self.stand_in_water(0.5, true),
            ("bucket", 40) => {
                let depth = |w: &Water| w.raycast(&self.world, eye, look, game::REACH * vpm).map(|(p, _)| w.depth_at(p.x, p.y) / vpm);
                println!("frame {frame}: water depth where looking {:?} m, bucket {:.2} m³", depth(&self.water), self.game.bucket());
                self.scoop_water(eye, look);
                println!("frame {frame}: scooped, bucket now {:.2} m³", self.game.bucket());
            }
            ("bucket", 160) => {
                self.pour_water(eye, look);
                println!("frame {frame}: poured, bucket now {:.2} m³", self.game.bucket());
            }
            ("wade", 2) => self.scripted_walk = true,
            ("hunt", 20) => {
                println!("frame {frame}: aiming at something: {}", self.game.has_target());
                self.game.primary(eye);
                println!("frame {frame}: carrying {}", self.game.carried());
            }
            ("hunt", 40) => {
                self.game.interact(eye);
                println!("frame {frame}: carrying {}", self.game.carried());
            }
            // Fights what's in front with the sword, then the bow, then the staff
            ("fight", f) => {
                let hold = |attack| combat::Controls { attack, holding: attack, ..Default::default() };
                let aim = |attack| combat::Controls { attack, holding: attack, heavy: false, aiming: true };
                self.script_controls = match f {
                    10 => {
                        self.game.select_weapon(weapons::Kind::Melee);
                        hold(true)
                    }
                    11..=70 | 211..=290 => hold(true),
                    // Drawing the bow, then shooting
                    91..=139 | 151..=189 => aim(false),
                    140 | 190 => aim(true),
                    90 => {
                        self.game.select_weapon(weapons::Kind::Ranged);
                        hold(false)
                    }
                    210 => {
                        self.game.select_weapon(weapons::Kind::Magic);
                        hold(false)
                    }
                    _ => hold(false),
                };
                if [5, 75, 145, 200, 300].contains(&f) {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
            // Casts spells at the ground a little way ahead, blowing holes in it
            ("boom", 1) => {
                self.game.select_weapon(weapons::Kind::Magic);
                self.camera.rotation.x = -0.9;
            }
            ("boom", f) => {
                self.script_controls = combat::Controls { attack: f > 10 && f < 200, holding: f > 10 && f < 200, ..Default::default() };
                if f % 50 == 0 {
                    println!("frame {frame}: {} holes blown, {} bricks stored", self.craters.len(), self.world.brick_count());
                }
            }
            // Keeps attacking with what's in hand, and `draw` keeps a bow drawn, for screenshots
            ("swing", _) => self.script_controls = combat::Controls { attack: true, holding: true, ..Default::default() },
            // Turns steadily while swinging, to see how things look on the move
            ("pan", _) => {
                self.camera.rotation.z += 0.004;
                self.script_controls = combat::Controls { attack: true, holding: true, ..Default::default() };
            }
            // Heavy blows on foot, so a hammer dents the ground
            ("heavy", f) => {
                if f == 1 && self.player.is_none() {
                    self.player = Some(Player::new(eye, &self.world));
                }
                self.script_controls = combat::Controls { heavy: true, aiming: true, ..Default::default() };
                if f % 40 == 0 {
                    println!("frame {frame}: {} holes blown", self.craters.len());
                }
            }
            ("draw", f) => self.script_controls = combat::Controls { aiming: f > 5, ..Default::default() },
            ("loot", 20) => {
                println!("frame {frame}: aiming at something: {}", self.game.has_target());
                self.game.interact(eye);
                println!("frame {frame}: carrying {}\n{}", self.game.carried(), self.game.describe_weapons());
            }
            // Hops the camera to one side and back every so often, so the world gets moved over
            // to new pieces of the landscape and back while it watches
            ("wobble", f) if f > 0 && f % 700 == 0 => {
                let side = if (f / 700) % 2 == 1 { 45.0 } else { -45.0 };
                let moved = eye + Vector3::new(side * vpm, 0.0, 0.0);
                self.camera.position = voxels_to_camera(moved, self.world.size);
                println!("frame {frame}: hopped {side} m");
            }
            _ => {}
        }
        if let Some(player) = &self.player
            && script == "wade"
            && frame % 60 == 0
        {
            let p = player.position;
            println!("frame {frame}: player at water depth {:.2} m, making a wake: {}", self.water.depth_at(p.x, p.y) / vpm, self.wake.is_some());
        }
        let after = self.game.last_message().map(str::to_owned);
        if after != before
            && let Some(message) = after
        {
            println!("frame {frame}: message \"{message}\"");
        }
    }

    /// Puts the camera just above some water near where it is, looking down at it, `deep` meters
    /// deep at least. With `walk`, as a player standing in it.
    fn stand_in_water(&mut self, deep: f64, walk: bool) {
        let vpm = self.world.voxels_per_meter as f64;
        let start = camera_to_voxels(self.camera.position, self.world.size);
        for ring in 0..200 {
            let count = (ring * 6).max(1);
            for i in 0..count {
                let angle = i as f64 / count as f64 * TAU;
                let (x, y) = (start.x + angle.cos() * ring as f64 * vpm, start.y + angle.sin() * ring as f64 * vpm);
                if self.water.depth_at(x, y) < deep * vpm {
                    continue;
                }
                let surface = self.water.surface_at(x, y);
                let eye = Vector3::new(x, y, surface + 1.7 * vpm);
                if walk {
                    self.player = Some(Player::new(eye, &self.world));
                    self.camera.rotation.x = -0.3;
                } else {
                    self.player = None;
                    self.camera.position = voxels_to_camera(eye, self.world.size);
                    self.camera.rotation.x = -0.9;
                }
                println!("standing in {:.2} m of water, {:.0} m from the start", self.water.depth_at(x, y) / vpm, ring);
                return;
            }
        }
        println!("no water near the start");
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
            let (world, set) = build.job.join().unwrap();
            // Unless the world was regenerated or the detail changed in the meantime
            if Arc::ptr_eq(&build.landscape, &landscape) && world.voxels_per_meter == fine_vpm {
                // Its origin is from the corner of the whole landscape, the world's isn't
                let window = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
                self.near = Some(NearDetail {
                    set,
                    origin: Vector3::from(world.origin).cast::<f64>() / scale as f64 - window,
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
            let window = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            let center = (camera + window) * scale as f64;
            // Brick aligned, and within the height of the world
            let corner = |axis: usize| ((center[axis] - size[axis] as f64 * 0.5) / 8.0).round() as i64 * 8;
            let top = self.world.size[2] as i64 * scale as i64 - size[2] as i64;
            let origin = [corner(0), corner(1), corner(2).clamp(0, top.max(0))];
            let job_landscape = landscape.clone();
            let (memory_allocator, command_buffer_allocator, descriptor_set_allocator, queue) = (
                self.memory_allocator.clone(),
                self.command_buffer_allocator.clone(),
                self.descriptor_set_allocator.clone(),
                self.queue.clone(),
            );
            let render_pipeline = (*self.render_pipeline).clone();
            let job = std::thread::spawn(move || {
                let start = Instant::now();
                let world = job_landscape.build(origin, size, fine_vpm);
                // Uploaded here too, so the frames carry on smoothly meanwhile
                let set = get_voxel_set(memory_allocator, command_buffer_allocator, descriptor_set_allocator, &render_pipeline, &queue, 2, &world).set;
                println!("Built fine detail: {} bricks ({:?})", world.brick_count(), start.elapsed());
                (world, set)
            });
            self.near_build = Some(NearBuild {
                job,
                landscape,
                center: camera,
            });
        }
    }

    fn render(&mut self, event_loop: &ActiveEventLoop) {
        if self.generate_requested {
            self.generate_requested = false;
            self.generate_world();
        }
        if self.waterfall_requested {
            self.waterfall_requested = false;
            self.visit_waterfall();
        }
        if self.render_pipeline.maybe_reload() {
            self.reset_accum = true;
            self.sun_map_for = None;
        }
        self.post_pipeline.maybe_reload();
        self.resample_pipeline.maybe_reload();
        self.upscale_pipeline.maybe_reload();
        self.water_pipeline.maybe_reload();
        self.update_window();
        self.update_near_detail();
        // For drawing what's held, and the numbers floating over what was hit
        let view = self.view();

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
            let kept = rcx.images.histories.clone();
            rcx.images = get_render_images(
                self.memory_allocator.clone(),
                self.descriptor_set_allocator.clone(),
                &self.render_pipeline,
                &self.post_pipeline,
                &self.resample_pipeline,
                &self.upscale_pipeline,
                &self.sun_map,
                render_extent,
                window_extent,
                Some(kept.clone()),
            );
            // New ones have nothing in them yet
            if !Arc::ptr_eq(&rcx.images.histories[0], &kept[0]) {
                self.last_world_to_window = None;
            }

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

        // What the menu and inventory ask for, done once the GUI is drawn
        let mut focus_change = None;
        let mut quit = false;

        rcx.gui.immediate_ui(|gui| {
            let ctx = gui.context();

            if self.menu_open {
                // Dim the world behind the menu a little
                ctx.layer_painter(egui::LayerId::background())
                    .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(60));
                egui::Area::new(egui::Id::new("menu"))
                    .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                    .show(&ctx, |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_black_alpha(180))
                            .corner_radius(18.0)
                            .inner_margin(36.0)
                            .show(ui, |ui| {
                                ui.set_width(340.0);
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new("INNERT").size(60.0).strong().color(Color32::WHITE));
                                    ui.label(RichText::new("a little voxel wilderness").size(15.0).color(Color32::from_gray(200)));
                                    ui.add_space(26.0);

                                    let button = |ui: &mut egui::Ui, text: &str| {
                                        ui.add_sized([260.0, 44.0], egui::Button::new(RichText::new(text).size(20.0)))
                                    };
                                    if button(ui, if self.started { "Resume" } else { "Play" }).clicked() {
                                        self.menu_open = false;
                                        self.started = true;
                                        focus_change = Some(true);
                                    }
                                    ui.add_space(8.0);
                                    if button(ui, "New World").clicked() {
                                        self.scene = Scene::Terrain;
                                        self.generate_requested = true;
                                        self.menu_open = false;
                                        self.started = true;
                                        focus_change = Some(true);
                                    }
                                    ui.horizontal(|ui| {
                                        ui.add_space(70.0);
                                        ui.add(egui::DragValue::new(&mut self.terrain.seed).prefix("Seed: "));
                                        if ui.button("Random").clicked() {
                                            let nanos = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .map_or(0, |d| d.subsec_nanos());
                                            self.terrain.seed = nanos % 100_000;
                                        }
                                    });
                                    ui.add_space(8.0);
                                    if button(ui, if self.show_settings { "Hide Settings" } else { "Settings" }).clicked() {
                                        self.show_settings = !self.show_settings;
                                    }
                                    ui.add_space(8.0);
                                    if button(ui, "Quit").clicked() {
                                        quit = true;
                                    }
                                    ui.add_space(22.0);
                                    ui.label(
                                        RichText::new(
                                            "WASD move · Space jump · Shift run · F fly\n\
                                             Click swing or cast · bow: hold right click, click to shoot\n\
                                             Right click heavy swing, or eat · E pick up, open, pet\n\
                                             Number keys pick what's in hand · I inventory\n\
                                             F1 settings · Esc menu",
                                        )
                                        .size(13.0)
                                        .color(Color32::from_gray(190)),
                                    );
                                });
                            });
                    });
            }

            if !self.menu_open && self.loaded_scene.is_terrain() && self.animals_on {
                self.game.hud(&ctx, &view);
                if self.inventory_open {
                    let mut open = true;
                    self.game.inventory(&ctx, &mut open);
                    if !open {
                        self.inventory_open = false;
                        focus_change = Some(true);
                    }
                }
            }

            if !self.show_settings {
                return;
            }
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
                        ui.checkbox(&mut self.smooth_shading, "Smooth Voxel Shading");
                        ui.checkbox(&mut self.wind, "Wind in the Leaves");
                        ui.add(egui::Slider::new(&mut self.haze, 0.0..=0.06).text("Haze"));
                        ui.checkbox(&mut self.grass, "Smooth Grass Blades (not voxels)");
                        ui.checkbox(&mut self.sunbeams, "Sunbeams Through the Haze");
                        ui.checkbox(&mut self.denoise, "Smooth Out Noise While Moving");
                        ui.checkbox(&mut self.color_grade, "Warm Color Grade");
                        ui.add(egui::Slider::new(&mut self.bloom, 0.0..=1.5).text("Glow"));
                    });
                    if let Some(sound) = &mut self.audio {
                        ui.add(egui::Slider::new(&mut sound.volume, 0.0..=1.0).text("Volume"));
                    }

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
                            self.generate_requested = true;
                        }
                        if let Some(error) = &self.world_error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                        if self.loaded_scene.is_terrain() {
                            ui.separator();
                            ui.checkbox(&mut self.animals_on, "Animals and Food");
                            if ui.button("Visit a Waterfall").clicked() {
                                self.waterfall_requested = true;
                            }
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

        if let Some(focused) = focus_change {
            self.focused = focused;
            grab_cursor(&rcx.window, focused);
        }
        if quit {
            event_loop.exit();
        }

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
        if self.water.active {
            flags |= FLAG_WATER;
        }
        if self.smooth_shading {
            flags |= FLAG_SMOOTH;
        }
        if self.wind {
            flags |= FLAG_WIND;
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

        // A weapon was found, so there's a new model to draw
        if self.game.models_version() != self.models_version {
            self.models_version = self.game.models_version();
            self.animal_models = model_buffer(&self.memory_allocator, self.game.model_words());
        }

        // The weapon in hand, and the animals, food and things flying near the camera, for the
        // shader to trace
        let playing = self.loaded_scene.is_terrain() && self.animals_on;
        let instances: Vec<AnimalInstance> = if playing { self.game.instances(&self.world, &view) } else { Vec::new() };
        let instance_buffer = self
            .instance_allocator
            .allocate_slice::<AnimalInstance>(instances.len().max(1) as u64)
            .unwrap();
        instance_buffer.write().unwrap()[..instances.len()].copy_from_slice(&instances);

        // Light from spells and fire first, then the flames and crystals nearest the camera whose
        // light reaches it
        let camera_voxels = camera_to_voxels(self.camera.position, self.world.size).cast::<f32>();
        let mut lights: Vec<(f32, [f32; 8])> = self
            .lights
            .iter()
            .map(|l| ((Vector3::new(l[0], l[1], l[2]) - camera_voxels).norm() - l[3], *l))
            .filter(|(d, _)| *d < 0.0)
            .collect();
        lights.sort_by(|a, b| a.0.total_cmp(&b.0));
        let fighting = if playing { self.game.lights(&view) } else { Vec::new() };
        let lights: Vec<[f32; 8]> = fighting.into_iter().chain(lights.into_iter().map(|(_, l)| l)).take(MAX_LIGHTS).collect();
        let light_buffer = self.instance_allocator.allocate_slice::<[f32; 8]>(lights.len().max(1) as u64).unwrap();
        light_buffer.write().unwrap()[..lights.len()].copy_from_slice(&lights);

        let animal_set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.render_pipeline.layout().set_layouts()[3].clone(),
            [
                WriteDescriptorSet::buffer(0, self.animal_models.clone()),
                WriteDescriptorSet::buffer(1, instance_buffer),
                WriteDescriptorSet::image_view(2, self.water.ground_view.clone()),
                WriteDescriptorSet::image_view(3, self.water.surface_view.clone()),
                WriteDescriptorSet::image_view(4, self.water.blocks_view.clone()),
                WriteDescriptorSet::buffer(5, light_buffer),
                WriteDescriptorSet::buffer(6, self.light_cache.clone()),
            ],
            [],
        )
        .unwrap();
        // Things fade into the sky before the edge of the world, which is never nearer than this
        let fade = if self.landscape.is_some() {
            ((self.world.size[0].min(self.world.size[1]) as f64 * 0.5 - RECENTER_M * self.world.voxels_per_meter as f64) as f32).max(0.0)
        } else {
            0.0
        };

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
            Some(last) if *last == view && !instances.is_empty() => ANIMATED_HISTORY,
            Some(last) if *last == view => STILL_HISTORY,
            Some(last) if (last.sun_direction, last.render_mode, last.flags, last.haze) == (view.sun_direction, view.render_mode, view.flags, view.haze) => MOVING_HISTORY,
            _ => 0,
        };
        self.reset_accum = false;
        self.accum_count = if history_limit == STILL_HISTORY || history_limit == ANIMATED_HISTORY {
            (self.accum_count + 1).min(history_limit)
        } else {
            0
        };
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
            window: [i32; 4],
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
        // The last of near_size is free, so it carries how many animals there are
        let near_size = [near_size[0], near_size[1], near_size[2], instances.len() as u32];
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
            haze: [view.haze, self.start_time.elapsed().as_secs_f32(), lights.len() as f32, fade],
            window: [self.world.origin[0] as i32, self.world.origin[1] as i32, 0, 0],
        };
        // Light gathered under another sun, or in another world, is no good
        let cache_for = (sun_direction, self.terrain.seed);
        let clear_light_cache = self.light_cache_for != Some(cache_for);
        self.light_cache_for = Some(cache_for);
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
        if clear_light_cache {
            builder.fill_buffer(self.light_cache.clone(), 0).unwrap();
        }

        // The water flows on, except while the menu is open
        let now = Instant::now();
        let frame_time = if self.menu_open { 0.0 } else { (now - self.last_frame).as_secs_f64() };
        self.last_frame = now;
        let camera = camera_to_voxels(self.camera.position, self.world.size);
        self.water.record(&mut builder, &self.water_pipeline, frame_time, [camera.x, camera.y], self.wake);

        builder
            .bind_pipeline_compute(self.render_pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                self.render_pipeline.layout().clone(),
                0,
                vec![render_set, self.voxel_set.set.clone(), near_set, animal_set],
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

        // Up to the window's resolution: gathered over frames when shaded at or below it, otherwise
        // just averaged down or stretched
        let upscale = shaded && render_extent[0] <= resample_extent[0] && render_extent[1] <= resample_extent[1];
        let (pipeline, set) = if upscale {
            (self.upscale_pipeline.clone(), rcx.images.upscale_sets[parity].clone())
        } else {
            (self.resample_pipeline.clone(), rcx.images.resample_set.clone())
        };
        builder
            .bind_pipeline_compute(pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, vec![set])
            .unwrap();
        if upscale {
            // world_to_pixel() gives rendered pixels, whose centers are at whole numbers, and so
            // are the window's
            let ratio = [0, 1].map(|i| resample_extent[i] as f64 / render_extent[i] as f64);
            let mut to_window = Matrix4::identity();
            to_window.m11 = ratio[0];
            to_window.m13 = 0.5 * ratio[0] - 0.5;
            to_window.m22 = ratio[1];
            to_window.m23 = 0.5 * ratio[1] - 0.5;
            let world_to_window = to_window * world_to_pixel(&pixel_to_ray);
            let previous = self.last_world_to_window.replace(world_to_window);
            let constants = UpscaleConstants {
                pixel_to_ray: push_constants.pixel_to_ray,
                previous_world_to_pixel: previous.unwrap_or(world_to_window).cast(),
                frame: push_constants.frame,
                flags: if previous.is_some() { UPSCALE_HISTORY } else { 0 },
                // Smoother once the view settles, quicker to catch up while it moves
                most: if self.still_frames >= 2 { 12.0 } else { 4.0 },
                padding: 0.0,
            };
            builder.push_constants(pipeline.layout().clone(), 0, constants).unwrap();
        } else {
            self.last_world_to_window = None;
        }

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
        let mut capture_shot = None;
        if let Some(capture) = &mut self.capture {
            let now = Instant::now();
            if let Some(last) = capture.last_frame {
                capture.frame_times.push(now - last);
            }
            capture.last_frame = Some(now);
            let frame = capture.frame_times.len() + 1;
            capture_done = frame >= capture.frames as usize;
            if capture.shots.contains(&frame) {
                let stem = capture.path.file_stem().map_or("shot".into(), |s| s.to_string_lossy().into_owned());
                capture_shot = Some(capture.path.with_file_name(format!("{stem}-{frame}.png")));
            }
        }

        // Screenshots are taken from the resampled image, so they leave out the GUI
        let screenshot_buffer = (self.screenshot_requested || capture_done || capture_shot.is_some()).then(|| {
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
                // Screenshots read the frame back, so it has to be finished first. Screenshot runs
                // also wait for every frame, so their timings are the GPU's.
                if screenshot_buffer.is_some() || self.capture.is_some() {
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
            } else if let Some(path) = capture_shot {
                save_screenshot(&buffer, resample_extent, encoded, &path);
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
        let mut attributes = Window::default_attributes()
            .with_inner_size(INITIAL_WINDOW_RESOLUTION)
            .with_window_icon(Some(load_icon(include_bytes!("../assets/icon.png"))))
            .with_title("Innert");
        // Screenshot runs happen out of sight: off the screen, without taking focus or showing in the taskbar
        if self.capture.is_some() {
            attributes = attributes.with_active(false).with_position(winit::dpi::PhysicalPosition::new(-30000, -30000));
            #[cfg(windows)]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attributes = attributes.with_skip_taskbar(true);
            }
        }
        let window = Arc::new(event_loop.create_window(attributes).unwrap());
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
            &self.upscale_pipeline,
            &self.sun_map,
            render_extent,
            window_extent,
            None,
        );
        self.last_world_to_window = None;

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

/// Tells the player something went wrong. On Windows that's in a box, as there's no console to
/// see it in, except for screenshot runs, which have no one to click it away.
fn show_error(text: &str) {
    eprintln!("{text}");
    #[cfg(windows)]
    if !std::env::args().any(|a| a == "--screenshot") {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBoxW(window: *mut std::ffi::c_void, text: *const u16, caption: *const u16, kind: u32) -> i32;
        }
        const MB_ICONERROR: u32 = 0x10;
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide("Innert").as_ptr(), MB_ICONERROR);
        }
    }
}

/// Stops, saying why
fn fail(text: &str) -> ! {
    show_error(text);
    std::process::exit(1)
}

/// What most often fixes it when the graphics card can't be used
const DRIVER_HINT: &str = "Innert needs a graphics card that supports Vulkan 1.3, which most made in the last several \
    years do. Installing the latest driver from NVIDIA, AMD or Intel usually sorts it out.";

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let place = info.location().map_or(String::new(), |l| format!(" ({}:{})", l.file(), l.line()));
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        show_error(&format!("Innert ran into a problem and has to close.\n\n{what}{place}"));
    }));
    let event_loop = EventLoop::new().unwrap_or_else(|e| fail(&format!("Couldn't open a window: {e}")));
    let mut app = App::new(&event_loop);
    event_loop.run_app(&mut app).unwrap();
}
