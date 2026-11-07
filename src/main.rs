// No console window behind the game, except when debugging. Problems are shown in a box, see
// show_error().
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use egui_winit_vulkano::{
    Gui,
    egui::{self, Align2, Color32, RichText},
};
use nalgebra::{Matrix3, Matrix4, Rotation3, UnitQuaternion, Vector2, Vector3, Vector4};
use std::path::PathBuf;
use std::{
    collections::VecDeque,
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
    format::{Format, FormatFeatures, NumericFormat},
    image::{
        Image, ImageCreateInfo, ImageType, ImageUsage,
        sampler::Filter,
        view::{ImageView, ImageViewCreateInfo},
    },
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::{
        MemoryHeapFlags, MemoryPropertyFlags,
        allocator::{AllocationCreateInfo, GenericMemoryAllocatorCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    },
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    swapchain::{
        PresentFuture, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
        acquire_next_image,
    },
    sync::{self, GpuFuture, future::FenceSignalFuture},
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::{DeviceEvent, DeviceId, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::KeyCode,
    raw_window_handle::HasDisplayHandle,
    window::{CursorGrabMode, Icon, Window, WindowId},
};
use winit_input_helper::WinitInputHelper;

mod animals;
mod audio;
mod bosses;
mod camera;
mod combat;
mod effects;
mod game;
mod gpu_timing;
mod hot_reload;
mod leaves;
mod limits;
mod loading;
mod memory;
mod perf;
mod player;
mod quality;
mod shafts;
mod skips;
mod staging;
mod startup;
mod stream;
mod voxelize;
mod water;
mod weapons;
mod worldgen;

use crate::animals::AnimalInstance;
use crate::camera::Camera;
use crate::game::Game;
use crate::hot_reload::HotReloadComputePipeline;
use crate::loading::Step;
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

/// Out of a hundred ruins of the kinds bosses like, how many have one
const BOSS_CHANCE: i64 = 30;

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
// The haze leaves the sunlight it scatters to shaders\shafts.comp
const FLAG_SHAFTS: u32 = 256;
const FLAG_REFLECTIONS: u32 = 1 << 16;

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

/// Puts a whole piece of the landscape built in one go, `world` from `landscape`, into a world that
/// moves on over the landscape a strip at a time from there, see stream.rs: with what it has besides
/// its voxels, and where the rivers flow into it
fn into_window(world: World, landscape: &Terrain) -> (World, Vec<worldgen::PieceFeatures>, Vec<[f32; 4]>) {
    let strip = stream::STRIP_M * world.voxels_per_meter;
    let mut window = World::window(world.size, strip, [world.origin[0], world.origin[1]], world.voxels_per_meter, world.sea_level);
    let pool = window.page_pool().expect("the world that moves on has pages");
    let mut installing = World::install(worldgen::Strip::new(world, &pool, strip));
    while !window.install_step(&mut installing, u32::MAX) {}
    (window, vec![landscape.piece_features()], landscape.river_springs())
}

/// How far off things fade into the sky in a world `size` voxels across that moves on `strip`
/// voxels at a time, at `vpm` voxels a meter, with `view_distance` of as far as they can be seen:
/// the edge of the world is never nearer than half of it less a strip, see stream.rs
fn fade_distance(size: [u32; 3], strip: u32, vpm: f64, view_distance: f32) -> f64 {
    (size[0].min(size[1]) as f64 * 0.5 - strip as f64 - FADE_SHORT_M * vpm).max(0.0) * view_distance.clamp(0.25, 1.0) as f64
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
    /// How many of App::craters are blown in it: once there are more, it's built again
    craters: usize,
}

struct NearBuild {
    // The fine voxels, already uploaded to the GPU
    job: std::thread::JoinHandle<(World, Arc<DescriptorSet>)>,
    /// Which world it was built for, see App::world_builds: one built since has no use for it
    world_build: u32,
    center: Vector3<f64>,
    craters: usize,
}

/// Frames after the world's moved on before the part of the GPU's memory it was drawn with that's
/// no longer in it is used again for what comes in: by then the frames drawn with it are long done,
/// see stream.rs
const RETIRED_FRAMES: u32 = 8;
/// How far short of the edge of the world things fade into the sky, past a strip's width, in
/// meters: the world moves on a strip at a time once the camera's half a strip from its middle, so
/// the edge is never nearer than half the world less a strip, see stream.rs. 14 m gives 82 m in a
/// 224 m world, as far as one saw before it moved on in strips: 4 m gave 92 m for 0.08 ms more
const FADE_SHORT_M: f64 = 14.0;
/// Frames sent to the GPU before the oldest is waited for, see App::max_in_flight
const MAX_IN_FLIGHT: usize = 2;
/// Of what the GPU's memory has room for, see perf::vram(), this much is kept free for what comes
/// and goes, like making more room for bricks, and never less than VRAM_MARGIN_MIN bytes
const VRAM_MARGIN: f64 = 0.1;
const VRAM_MARGIN_MIN: u64 = 256 << 20;
/// The render scale the game starts at, with a graphics card of its own. A higher one comes down
/// for want of room in the GPU's memory, but never below this.
const DEFAULT_RENDER_SCALE: f32 = 0.45;
/// How long a render scale slider has to be left alone before the images are made again for it
const SCALE_SETTLE: Duration = Duration::from_millis(300);
/// How often the GPU's memory is looked at in play, to come down in render scale should something
/// else take some of it, and back up once it's let go of
const VRAM_CHECK: Duration = Duration::from_secs(2);
/// How long after coming down for want of room the render scale may go back up, so it doesn't go
/// up and down while whatever took the room comes and goes
const RAISE_AFTER: Duration = Duration::from_secs(30);
/// The most flames and crystals lighting things up at once, nearest the camera first
const MAX_LIGHTS: usize = 16;
/// Has to match CACHE_WORDS in shaders\traverse.comp. How many entries the light cache has is
/// up to how much memory the graphics card has, see memory.rs: the shader goes by how big it is.
const LIGHT_CACHE_WORDS: u64 = 13;

// The box of fine detail around the camera, in meters. It gets rebuilt once the camera
// gets this far from where it was last built.
const NEAR_WIDTH_M: u32 = 40;
const NEAR_HEIGHT_M: u32 = 20;
const NEAR_REBUILD_M: f64 = 8.0;
const NEAR_DETAIL_OPTIONS: &[u32] = &[0, 32];

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
    // Blocks of 64 MB, `--set block_mb=256` for vulkano's own size. Anything over half a block gets
    // memory of its own, just as big and let go of when it is; the tens of megabytes of the
    // world's maps and the light cache otherwise went into blocks of 256 MB that are never let
    // go of, a block for each kind of memory, much of them empty.
    let block = settings_from_args().iter().find(|(k, _)| k == "block_mb").and_then(|(_, v)| v.parse::<u64>().ok()).unwrap_or(64) << 20;
    let properties = device.physical_device().memory_properties();
    let mut memory_type_bits = u32::MAX;
    for (i, memory_type) in properties.memory_types.iter().enumerate() {
        // As StandardMemoryAllocator::new_default() leaves out
        if memory_type.property_flags.intersects(MemoryPropertyFlags::LAZILY_ALLOCATED | MemoryPropertyFlags::PROTECTED | MemoryPropertyFlags::DEVICE_COHERENT | MemoryPropertyFlags::RDMA_CAPABLE) {
            memory_type_bits &= !(1 << i);
        }
    }
    let memory_type_bits = limits::memory_types(properties, memory_type_bits);
    let block_sizes = vec![block; properties.memory_types.len()];
    let memory_allocator = Arc::new(StandardMemoryAllocator::new(
        device.clone(),
        GenericMemoryAllocatorCreateInfo { block_sizes: &block_sizes, memory_type_bits, ..Default::default() },
    ));
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
    let physical = device.physical_device();
    let caps = physical.surface_capabilities(surface, Default::default()).unwrap();

    // One the rendered image can be blitted to, see limits::surface_format()
    let blits = |format: Format| physical.format_properties(format).is_ok_and(|p| p.optimal_tiling_features.contains(FormatFeatures::BLIT_DST));
    let (image_format, image_color_space) = limits::surface_format(&physical.surface_formats(surface, Default::default()).unwrap(), blits);
    // Immediate where it's there, see limits::present_mode()
    let present_mode = limits::present_mode(&physical.surface_present_modes(surface, Default::default()).unwrap_or_default());

    let composite_alpha = caps.supported_composite_alpha.into_iter().next().unwrap();

    Swapchain::new(
        device.clone(),
        surface.clone(),
        SwapchainCreateInfo {
            min_image_count: caps.max_image_count.map_or(caps.min_image_count.max(3), |most| caps.min_image_count.max(3).min(most)),
            image_format,
            image_color_space,
            image_extent: window.inner_size().into(),
            // Blitted to, and copied from for shots of the screen where it can be
            image_usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_DST | (caps.supported_usage_flags & ImageUsage::TRANSFER_SRC),
            composite_alpha,
            present_mode,
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
    storage_image_view(memory_allocator, extent, format, ImageUsage::empty())
}

/// A storage image `extent` big, usable for `more` as well
fn storage_image_view(
    memory_allocator: Arc<StandardMemoryAllocator>,
    extent: [u32; 2],
    format: Format,
    more: ImageUsage,
) -> Arc<ImageView> {
    let image = Image::new(
        memory_allocator,
        ImageCreateInfo {
            usage: ImageUsage::STORAGE | more,
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
    // For sun shafts, see shafts.rs: traverse.comp's depths and guide, and what the denoiser leaves.
    // The depths are -1 for the sky, which screenshot runs look at too, see perf::PerfLog
    depths: [Arc<ImageView>; 2],
    guide: Arc<ImageView>,
    hdr: Arc<ImageView>,
}

/// Screenshot runs say how much of the graphics card's memory is used by then, to see what takes it
fn note_vram(device: &Device, by: &str) {
    if std::env::args().any(|a| a == "--screenshot")
        && let Some(vram) = perf::vram(device)
    {
        println!("VRAM once {by}: {:.0} MB of {:.0}", vram.used as f64 / perf::MB, vram.budget as f64 / perf::MB);
    }
}

/// Waits for the frames sent to the GPU to be done, so what they were drawn with can change or be
/// let go of
fn wait_for_frames(rcx: &mut RenderContext, device: &Arc<Device>) {
    for frame in rcx.in_flight.drain(..) {
        let _ = frame.wait(None);
    }
    // Letting go of what's left of the last one waits for it too
    rcx.previous_frame_end = Some(sync::now(device.clone()).boxed());
}

/// How big the image traced into is at render scale `scale` for a window `window` big
fn render_extent(window: [u32; 2], scale: f32) -> [u32; 2] {
    window.map(|n| ((n as f32 * scale) as u32).max(1))
}

/// The render scale nearest `wanted` that everything sized to the render resolution fits into
/// `room` bytes of the GPU's memory at, for a window `window` big. Never below the usual one,
/// DEFAULT_RENDER_SCALE: under it there's next to nothing to be had, tens of megabytes, for a
/// blurrier picture.
fn fitting_scale(wanted: f32, window: [u32; 2], room: u64) -> f32 {
    let lowest = wanted.min(DEFAULT_RENDER_SCALE);
    let mut scale = wanted;
    while scale > lowest && perf::render_bytes(render_extent(window, scale), window) > room {
        scale = (scale * 0.97).max(lowest);
    }
    scale
}

impl RenderImages {
    /// How much of the GPU's memory they take, near enough
    fn bytes(&self) -> u64 {
        let [rw, rh, _] = self.render_image.extent();
        let [ww, wh, _] = self.resample_image.extent();
        perf::render_bytes([rw, rh], [ww, wh])
    }

    /// Whether they're for rendering `render` big into a window `window` big
    fn fits(&self, render: [u32; 2], window: [u32; 2]) -> bool {
        self.render_image.extent()[..2] == render[..] && self.resample_image.extent()[..2] == window[..]
    }
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
    // Copied from for a look at how much of the view is sky, see App::check_sky()
    let depths = [0, 1].map(|_| storage_image_view(memory_allocator.clone(), render_extent, Format::R32_SFLOAT, ImageUsage::TRANSFER_SRC));
    // Half floats are plenty for colors and which way faces point, and quicker to read many times
    let guide = float_image(render_extent, Format::R16G16B16A16_SFLOAT);

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
    let temps = [0, 1].map(|_| float_image(render_extent, Format::R16G16B16A16_SFLOAT));
    let bloom_extent = render_extent.map(|n| n.div_ceil(BLOOM_SCALE).max(1));
    let blooms = [0, 1].map(|_| float_image(bloom_extent, Format::R32G32B32A32_SFLOAT));

    let layout = post_pipeline.layout().set_layouts()[0].clone();
    // `accum` is what the denoiser's first pass reads, the others read `hdr_in`
    let post_set = |hdr_in: &Arc<ImageView>, hdr_out: &Arc<ImageView>, depth: &Arc<ImageView>, bloom_in: usize, accum: &Arc<ImageView>| {
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
                WriteDescriptorSet::image_view(7, accum.clone()),
            ],
            [],
        )
        .unwrap()
    };
    // The denoiser reads this frame's colors and ends up in temps[0]
    let denoise_sets = [0, 1].map(|parity| {
        [
            post_set(&temps[1], &temps[0], &depths[parity], 0, &colors[parity]),
            post_set(&temps[0], &temps[1], &depths[parity], 0, &colors[parity]),
            post_set(&temps[1], &temps[0], &depths[parity], 0, &colors[parity]),
        ]
    });
    // Bloom ends up in blooms[0], which tone mapping then reads
    let bloom_extract_set = post_set(&temps[0], &temps[1], &depths[0], 1, &colors[0]);
    let bloom_blur_sets = [
        post_set(&temps[0], &temps[1], &depths[0], 0, &colors[0]),
        post_set(&temps[0], &temps[1], &depths[0], 1, &colors[0]),
    ];
    let tonemap_set = post_set(&temps[0], &temps[1], &depths[0], 0, &colors[0]);

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
        hdr: temps[0].clone(),
        depths,
        guide,
    }
}

/// The view, for traverse.comp and shafts.comp: in a small buffer written each frame, rather than
/// in their push constants, which AMD's graphics cards and some of Intel's have room for only 128
/// bytes of, see limits.rs. Each frame's is a buffer of its own, see App::instance_allocator, so
/// the frames still being drawn keep theirs. Has to match View in those shaders.
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct View {
    pixel_to_ray: Matrix4<f32>,
    /// World to last frame's rendered pixel
    previous_world_to_pixel: Matrix4<f32>,
}

/// The top three rows of `m`, whose last is 0, 0, 0, 1, as upscale.comp takes them
fn top_rows(m: &Matrix4<f64>) -> [[f32; 4]; 3] {
    [0, 1, 2].map(|i| [0, 1, 2, 3].map(|j| m[(i, j)] as f32))
}

/// No more than 128 bytes, see limits.rs
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct UpscaleConstants {
    /// See top_rows()
    pixel_to_ray: [[f32; 4]; 3],
    /// World to window pixel last frame
    previous_world_to_pixel: [[f32; 4]; 3],
    frame: u32,
    flags: u32,
    most: f32,
    padding: f32,
}

// Has to match upscale.comp
const UPSCALE_HISTORY: u32 = 1;
const UPSCALE_FAST: u32 = 2;

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
    // What else the set binds, to bind again when there's more room for bricks, see grow()
    views: Vec<(u32, Arc<ImageView>)>,
    set_index: usize,
    // The bricks before there was more room for them, to be copied over with the next frame, see
    // grow_with_frame()
    copy_from: Option<Subbuffer<[u32]>>,
}

impl GpuWorld {
    /// How much of the GPU's memory it takes: its bricks, their grid, and its grass
    fn bytes(&self) -> u64 {
        let texels = |image: &Image| image.extent().iter().map(|&n| n as u64).product::<u64>() * image.format().block_size();
        let grass = self.views.iter().find(|(binding, _)| *binding == 2).map_or(0, |(_, view)| texels(view.image()));
        self.bricks.size() + texels(&self.grid) + grass
    }

    /// Makes room for at least `needed` bricks, and SPARE_BRICKS more: a bigger buffer, the bricks
    /// there are copied into on the GPU, which is far quicker than sending the world over again
    fn grow(&mut self, needed: usize, memory_allocator: &Arc<StandardMemoryAllocator>, command_buffer_allocator: &Arc<StandardCommandBufferAllocator>, descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>, render_pipeline: &ComputePipeline, queue: &Arc<Queue>) {
        self.grow_with_frame(needed, spare_bricks(), memory_allocator, descriptor_set_allocator, render_pipeline);
        self.settle(command_buffer_allocator, queue);
    }

    /// The same, `spare` more, but copied over with the next frame, see copy_over(), so no frame
    /// waits for it: drawn with the new buffer from then on, the old one goes once the frames
    /// before are done with it
    fn grow_with_frame(&mut self, needed: usize, spare: usize, memory_allocator: &Arc<StandardMemoryAllocator>, descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>, render_pipeline: &ComputePipeline) {
        // No more than a shader can see, see memory::most_bricks()
        let capacity = (needed + spare).min(memory::most_bricks()).max(needed);
        let bricks = bricks_buffer(memory_allocator, (capacity * worldgen::BRICK_WORDS) as u64);
        let mut writes: Vec<WriteDescriptorSet> = self.views.iter().map(|(binding, view)| WriteDescriptorSet::image_view(*binding, view.clone())).collect();
        writes.push(WriteDescriptorSet::buffer(1, bricks.clone()));
        let layout = render_pipeline.layout().set_layouts().get(self.set_index).unwrap().clone();
        self.set = DescriptorSet::new(descriptor_set_allocator.clone(), layout, writes, []).unwrap();
        // Should it grow twice before a frame, all of it's copied from the first
        let old = std::mem::replace(&mut self.bricks, bricks);
        self.copy_from.get_or_insert(old);
        self.capacity = capacity;
    }

    /// Copies the bricks over into the bigger buffer with the frame being recorded, if there is one
    /// since the last, before anything else's sent into it
    fn copy_over(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        if let Some(old) = self.copy_from.take() {
            let len = old.len().min(self.bricks.len());
            builder.copy_buffer(CopyBufferInfo::buffers(old.slice(0..len), self.bricks.clone().slice(0..len))).unwrap();
        }
    }

    /// Copies them over now, waiting for it, before anything's sent into them outside a frame
    fn settle(&mut self, command_buffer_allocator: &Arc<StandardCommandBufferAllocator>, queue: &Arc<Queue>) {
        if self.copy_from.is_none() {
            return;
        }
        let mut builder = AutoCommandBufferBuilder::primary(command_buffer_allocator.clone(), queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
        self.copy_over(&mut builder);
        builder.build().unwrap().execute(queue.clone()).unwrap().then_signal_fence_and_flush().unwrap().wait(None).unwrap();
    }
}

/// Room for this many more bricks in the main world than it starts with, for holes blown in it.
/// `--set spare_bricks=100` leaves less, to try out making more room.
fn spare_bricks() -> usize {
    settings_from_args().iter().find(|(k, _)| k == "spare_bricks").and_then(|(_, v)| v.parse().ok()).unwrap_or(worldgen::SPARE_BRICKS)
}

/// Room for a world's brick grid, one texel per 8x8x8 brick, `bricks` across, see worldgen::World
fn grid_image(memory_allocator: &Arc<StandardMemoryAllocator>, bricks: [u32; 3]) -> Arc<Image> {
    Image::new(
        memory_allocator.clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim3d,
            format: Format::R32_UINT,
            extent: bricks,
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap()
}

/// Room for the map of where grass grows, see worldgen::World::grass
fn grass_image(memory_allocator: &Arc<StandardMemoryAllocator>, extent: [u32; 2]) -> Arc<Image> {
    Image::new(
        memory_allocator.clone(),
        ImageCreateInfo {
            format: Format::R32_UINT,
            extent: [extent[0], extent[1], 1],
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo::default(),
    )
    .unwrap()
}

/// Room for `words` words of bricks
fn bricks_buffer(memory_allocator: &Arc<StandardMemoryAllocator>, words: u64) -> Subbuffer<[u32]> {
    Buffer::new_slice::<u32>(
        memory_allocator.clone(),
        BufferCreateInfo {
            // Copied from when there's to be more room for bricks, see GpuWorld::grow()
            usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST | BufferUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
        words,
    )
    .unwrap()
}

// Uploads a world, for traverse.comp's descriptor set `set_index`: 1 for the main world, 2 for the
// fine detail. The main world has room for `room` bricks, or more if it has more, see memory.rs.
#[allow(clippy::too_many_arguments)]
fn get_voxel_set(
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    render_pipeline: &ComputePipeline,
    queue: &Arc<Queue>,
    set_index: usize,
    world: &World,
    room: usize,
) -> GpuWorld {
    // One texel per 8x8x8 brick of the room there is, see worldgen::World, all of them sent over below
    let image = grid_image(&memory_allocator, world.grid_size());

    // Everything goes over a piece at a time through the same bit of host memory, each piece
    // waited for, see staging.rs: the frames being drawn while a new piece of the landscape is
    // sent over in the background get the GPU in between, and no new host memory is made for it
    let [w, h, _] = world.grid_size().map(|n| n as usize);
    let grid = &world.brick_grid;
    staging::send(
        &memory_allocator,
        &command_buffer_allocator,
        queue,
        grid.len(),
        staging::whole(w * h),
        |piece, first| piece.copy_from_slice(&grid[first..first + piece.len()]),
        |builder, piece, first| {
            let (z, depth) = ((first / (w * h)) as u32, (piece.len() as usize / (w * h)) as u32);
            builder
                .copy_buffer_to_image(CopyBufferToImageInfo {
                    regions: [BufferImageCopy {
                        image_subresource: image.subresource_layers(),
                        image_offset: [0, 0, z],
                        image_extent: [w as u32, h as u32, depth],
                        ..Default::default()
                    }]
                    .into(),
                    ..CopyBufferToImageInfo::buffer_image(piece, image.clone())
                })
                .unwrap();
        },
    );

    // The main world has room for more bricks, for holes blown in it, see App::blast(), and for
    // the world that moves on for the strips coming in, see stream.rs
    let needed = world.brick_room().max(1);
    let spare = if set_index == 1 { spare_bricks() } else { 0 };
    // No more room than a shader can see, see memory::most_bricks()
    let capacity = (needed + spare).max(if set_index == 1 { room } else { 0 }).min(memory::most_bricks()).max(needed);
    let bricks_buffer = bricks_buffer(&memory_allocator, (capacity * worldgen::BRICK_WORDS) as u64);
    match &world.pages {
        // Each page where it goes
        Some(_) => {
            let page_words = worldgen::PAGE_BRICKS * worldgen::BRICK_WORDS;
            let pages: Vec<(u32, &[u32])> = (0..(world.brick_room() / worldgen::PAGE_BRICKS) as u32).filter_map(|p| Some((p, world.page(p)?))).collect();
            staging::send(
                &memory_allocator,
                &command_buffer_allocator,
                queue,
                pages.len() * page_words,
                staging::whole(page_words),
                |piece, first| {
                    for (k, chunk) in piece.chunks_mut(page_words).enumerate() {
                        chunk.copy_from_slice(pages[first / page_words + k].1);
                    }
                },
                |builder, piece, first| {
                    let regions: Vec<BufferCopy> = (0..piece.len() as usize / page_words)
                        .map(|k| BufferCopy {
                            src_offset: (k * page_words * 4) as u64,
                            dst_offset: pages[first / page_words + k].0 as u64 * page_words as u64 * 4,
                            size: page_words as u64 * 4,
                            ..Default::default()
                        })
                        .collect();
                    builder.copy_buffer(CopyBufferInfo { regions: regions.into(), ..CopyBufferInfo::buffers(piece, bricks_buffer.clone()) }).unwrap();
                },
            );
        }
        None => {
            // An empty world still needs a buffer to bind
            let brick_words: &[u32] = if world.bricks.is_empty() { &[0; worldgen::BRICK_WORDS] } else { &world.bricks };
            staging::send(
                &memory_allocator,
                &command_buffer_allocator,
                queue,
                brick_words.len(),
                staging::PIECE_WORDS,
                |piece, first| piece.copy_from_slice(&brick_words[first..first + piece.len()]),
                |builder, piece, first| {
                    let to = bricks_buffer.clone().slice(first as u64..first as u64 + piece.len());
                    builder.copy_buffer(CopyBufferInfo::buffers(piece, to)).unwrap();
                },
            );
        }
    }

    let image_view =
        ImageView::new(image.clone(), ImageViewCreateInfo::from_image(&image)).unwrap();
    let mut views = vec![(0, image_view.clone())];
    let mut writes = vec![
        WriteDescriptorSet::image_view(0, image_view),
        WriteDescriptorSet::buffer(1, bricks_buffer.clone()),
    ];

    // The main world also has the map of where grass grows, see worldgen::World::grass
    if set_index == 1 {
        let (extent, grass): ([u32; 2], &[u32]) = match &world.grass {
            Some(grass) => (world.phys, grass),
            None => ([1, 1], &[0]),
        };
        // All of it sent over below
        let grass_image = grass_image(&memory_allocator, extent);
        let row = extent[0] as usize;
        staging::send(
            &memory_allocator,
            &command_buffer_allocator,
            queue,
            grass.len(),
            staging::whole(row),
            |piece, first| piece.copy_from_slice(&grass[first..first + piece.len()]),
            |builder, piece, first| {
                let (y, rows) = ((first / row) as u32, (piece.len() as usize / row) as u32);
                builder
                    .copy_buffer_to_image(CopyBufferToImageInfo {
                        regions: [BufferImageCopy {
                            image_subresource: grass_image.subresource_layers(),
                            image_offset: [0, y, 0],
                            image_extent: [extent[0], rows, 1],
                            ..Default::default()
                        }]
                        .into(),
                        ..CopyBufferToImageInfo::buffer_image(piece, grass_image.clone())
                    })
                    .unwrap();
            },
        );
        let grass_view =
            ImageView::new(grass_image.clone(), ImageViewCreateInfo::from_image(&grass_image)).unwrap();
        views.push((2, grass_view.clone()));
        writes.push(WriteDescriptorSet::image_view(2, grass_view));
    }


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
        views,
        set_index,
        copy_from: None,
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
    /// Works out again how far empty bricks skip where strips join the world, see skips.rs
    skips_pipeline: HotReloadComputePipeline,

    // Which parts of the world sunlight reaches, for sunbeams. Computed for this sun direction.
    sun_map: Arc<ImageView>,
    sun_map_for: Option<[f32; 4]>,

    voxel_set: GpuWorld,
    // Kept on the CPU too, for walking around in it
    world: World,
    /// How a generated world moves on over the landscape a strip at a time, see stream.rs. None
    /// for a model.
    stream: Option<stream::Stream>,
    /// How many times the world's been built anew, for what's built in the background to know if
    /// it's still of use
    world_builds: u32,
    // Rivers traced so far, for building the next piece of the landscape
    rivers: Arc<RiverCache>,
    // Where the camera was last frame and when, and which way and how fast it's been going, in
    // voxels a second, to build the next strip of the landscape ahead of it
    last_camera: Option<(Vector3<f64>, Instant)>,
    camera_drift: Vector3<f64>,
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
    // Beams of sunlight in the haze, see shafts.rs
    shafts: shafts::Shafts,
    grass: bool,
    denoise: bool,
    color_grade: bool,
    bloom: f32,
    // Which of the presets the settings above came from, see quality.rs, and what else it sets
    quality: quality::Preset,
    view_distance: f32,
    reflections: bool,
    fast_upscale: bool,
    // Picked in the settings, put in place at the start of the next frame
    pending_quality: Option<quality::Preset>,

    frame: u32,
    accum_count: u32,
    last_view: Option<ViewKey>,
    reset_accum: bool,
    // World to window pixel last frame, for upscale.comp to find where things were, if it ran
    last_world_to_window: Option<Matrix4<f64>>,

    // Animals, food and what the player carries
    game: Game,
    animals_on: bool,
    // Every animal's model and the rest that never change, see Game::model_words(), and every
    // weapon's, see Game::weapon_model_words()
    animal_models: Subbuffer<[u32]>,
    weapon_models: Subbuffer<[u32]>,
    // Which of Game::models_version() the weapons' are
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
    // Set by scripts in screenshot runs, see run_script(): the player walks straight ahead, maybe
    // running
    scripted_walk: bool,
    scripted_sprint: bool,
    // and the mouse buttons for the next frame
    script_controls: combat::Controls,
    // How far a shot's kick has turned the camera up, so it's turned back down as the kick dies away
    kicked: f64,
    // None without anything to play sound on, and in screenshot runs
    audio: Option<audio::Audio>,
    // Holes blown in the landscape: where, in meters from its corner, and how big. They're blown
    // again in each new piece of it the world moves on to.
    craters: Vec<([f64; 3], f64)>,
    /// Brick cells carved or painted and not yet sent to the GPU, and the columns whose ground
    /// the water needs to know about, see flush_edits(). With `batching` they wait to go together.
    edited: Vec<usize>,
    edited_ground: Option<([i64; 2], [i64; 2])>,
    batching: bool,
    // Stains splashed on the ground the same way: where, how big, of what, and whether it burnt
    // the plants there
    stains: Vec<([f64; 3], f64, u8, bool)>,
    // Meters walked since the last footstep
    step_distance: f64,
    // How fast the player's going over the ground in meters a second, smoothed, and how far
    // they've walked in all
    walk_speed: f64,
    walked: f64,
    // Which waterfall "Visit a Waterfall" went to last
    waterfall_index: usize,

    menu_open: bool,
    // Whether the game has started, so the menu says Resume instead of Play
    started: bool,
    show_settings: bool,
    inventory_open: bool,
    // Whether a merchant's stall was open last frame, to take the mouse back once it closes
    shopping: bool,
    // Set from the menu, done at the start of the next frame
    generate_requested: bool,
    waterfall_requested: bool,

    screenshot_requested: bool,
    capture: Option<Capture>,
    /// How many frames may be sent to the GPU before the oldest is waited for. With no limit, as
    /// when nothing on the screen held them back, seconds' worth piled up: what was asked of the
    /// GPU meanwhile, like sending over the next piece of the world, waited behind them all, and
    /// the GPU's memory they held grew. `--set in_flight=0` for no limit.
    max_in_flight: usize,
    /// How long frames take, smoothed, in milliseconds
    frame_ms: f64,
    /// The render scales last set, while moving and when still, and when: a slider being dragged
    /// only has the images made again once it's let go of a moment
    scales_set: (f32, f32),
    scales_set_at: Instant,
    /// When the GPU's memory was last looked at, see VRAM_CHECK
    vram_checked: Instant,
    /// When the render scale last came down for want of room in the GPU's memory, see RAISE_AFTER
    scale_lowered_at: Instant,
    /// How the graphics card's memory was shared out at the start, see memory.rs
    memory: memory::Plan,
    /// `--set perf_log=60` in screenshot runs, see perf.rs
    perf: Option<perf::PerfLog>,
    /// `--set sky_check=30`: every so many frames how far away a grid of pixels is gets copied
    /// back, to see how much of the view is sky, with how far the camera was looking down then
    sky_check: u32,
    sky_probe: Option<(Subbuffer<[f32]>, f64)>,
    blank_shots: u32,
    play: perf::Play,

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
    /// Those made for the other of the render scales while moving and when still, if they're not
    /// the same, kept as long as the GPU's memory has room for both, so the view settling or
    /// starting to move again doesn't make them anew each time, see App::make_render_images()
    other_images: Option<(f32, RenderImages)>,

    gui: Gui,

    recreate_swapchain: bool,
    recreate_images: bool,
    render_scale_in_use: f32,
    /// What render scale was asked for when they were made: as much less of it as `render_scale_in_use`
    /// is, there wasn't room for in the GPU's memory
    render_scale_wanted: f32,

    // Lets the CPU get on with the next frame while the GPU works on this one
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    /// The frames sent to the GPU that may not be done yet, oldest first, see App::max_in_flight
    in_flight: VecDeque<Arc<FrameEnd>>,
    /// What each of the window's images was last shown with, see loading::Presented
    presented: loading::Presented,
}

/// What's waited for to know a frame's done
type FrameEnd = FenceSignalFuture<PresentFuture<Box<dyn GpuFuture>>>;

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
    /// How long the GPU takes over each part of a frame, if `time_gpu`
    time_gpu: bool,
    gpu: Option<gpu_timing::GpuTimer>,
    /// Whether shots show what's on screen, the HUD and windows over the game, rather than just
    /// the game
    gui: bool,
    /// `--seconds 1200` ends the run after that long, whichever comes first with `--frames`
    seconds: Option<f64>,
    started: Instant,
    /// `--set nowait=1` doesn't wait for each frame to finish before starting the next, as in
    /// play, where the frames being drawn overlap: then frame times are what a player sees
    nowait: bool,
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
            // `--set gpu_times=1` prints how long the GPU took over each part of a frame
            time_gpu: settings_from_args().iter().any(|(k, v)| k == "gpu_times" && (v == "1" || v == "true")),
            gpu: None,
            // `--set gui=1` takes shots of the whole screen, HUD and all
            gui: settings_from_args().iter().any(|(k, v)| k == "gui" && (v == "1" || v == "true")),
            seconds: value("--seconds").and_then(|n| n.parse().ok()),
            started: Instant::now(),
            nowait: settings_from_args().iter().any(|(k, v)| k == "nowait" && (v == "1" || v == "true")),
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

/// Saves a copy of the screen as it was shown, in `format`, already encoded for display
fn save_screen(buffer: &vulkano::buffer::Subbuffer<[u8]>, extent: [u32; 3], format: Format, path: &Path) {
    let mut pixels = buffer.read().unwrap().to_vec();
    let bgra = matches!(format, Format::B8G8R8A8_UNORM | Format::B8G8R8A8_SRGB);
    for p in pixels.chunks_exact_mut(4) {
        if bgra {
            p.swap(0, 2);
        }
        p[3] = 255;
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

/// The graphics card and what's needed to use it. It's started first thing while loading, and the
/// loading screen's drawn with it while the rest is made, see loading.rs.
#[derive(Clone)]
struct Gpu {
    instance: Arc<Instance>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
}

impl Gpu {
    /// One that draws to windows on `display`. Stops, saying why, if there's none that can run
    /// the game.
    fn new(display: &impl HasDisplayHandle) -> Self {
        let library = VulkanLibrary::new()
            .unwrap_or_else(|e| fail(&format!("Vulkan isn't installed on this computer ({e}).\n\n{DRIVER_HINT}")));

        let mut required_extensions = Surface::required_extensions(display)
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
        // `--set gpu=intel` picks the one whose name has that in it, `gpu=1` the second found, as
        // where a laptop has a graphics card of its own and its processor's too
        let wanted = settings_from_args().into_iter().find(|(k, _)| k == "gpu").map(|(_, v)| v.to_lowercase());
        // And why those passed over were, see limits.rs
        let mut unsuited = Vec::new();
        let picked = devices
            .into_iter()
            .enumerate()
            .filter(|(i, p)| wanted.as_ref().is_none_or(|w| w.parse::<usize>().map_or_else(|_| p.properties().device_name.to_lowercase().contains(w.as_str()), |n| n == *i)))
            .map(|(_, p)| p)
            .filter(|p| {
                p.api_version() >= Version::V1_3 || p.supported_extensions().khr_dynamic_rendering
            })
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter(|p| match limits::unsuitable(p) {
                Some(why) => {
                    unsuited.push(format!("{} has no {why}", p.properties().device_name));
                    false
                }
                None => true,
            })
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        // Every card has one that does both, but not certainly the first that draws
                        q.queue_flags.contains(QueueFlags::GRAPHICS | QueueFlags::COMPUTE)
                            && p.presentation_support(i as u32, display).unwrap()
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
            });
        let (physical_device, queue_family_index) = picked.unwrap_or_else(|| {
            let found = if found.is_empty() { "none".to_string() } else { found };
            let unsuited = if unsuited.is_empty() { String::new() } else { format!("\n\n{}.", unsuited.join(".\n")) };
            let asked = wanted.as_ref().map_or(String::new(), |w| match w.parse::<usize>() {
                Ok(n) => format!(" numbered {n}, counting from 0,"),
                Err(_) => format!(" called {w:?}"),
            });
            fail(&format!("None of the graphics cards here{asked} can run Innert. Found: {found}.{unsuited}\n\n{DRIVER_HINT}"))
        });

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );
        // What it can do, which the shaders are checked against and the world sized to fit, see
        // limits.rs
        let limits = limits::set(&physical_device);
        if Capture::from_args().is_some() {
            println!("Limits{}: {limits:?}", if limits::min_mode() { " (no more than AMD's and Intel's)" } else { "" });
        }
        // Not a graphics card of its own, so slower: it gets a lower quality preset, see quality.rs
        let integrated = physical_device.properties().device_type != PhysicalDeviceType::DiscreteGpu;

        if physical_device.api_version() < Version::V1_3 {
            device_extensions.khr_dynamic_rendering = true;
        }
        // To see how much of its memory is free, see perf::vram(). As if there were none with
        // `--set limits=min`, to try out how the game does without.
        device_extensions.ext_memory_budget = physical_device.supported_extensions().ext_memory_budget && !limits::min_mode();
        // Which the world being built meanwhile is waiting on, to know how big it can be
        memory::set_card(perf::card_vram(&physical_device, device_extensions.ext_memory_budget).map(|v| v.budget));
        // And the quality preset that suits it, see quality.rs
        let card_memory = physical_device.memory_properties().memory_heaps.iter().filter(|h| h.flags.intersects(MemoryHeapFlags::DEVICE_LOCAL)).map(|h| h.size).max().unwrap_or(0);
        quality::set_card(quality::suited(&physical_device.properties().device_name, physical_device.properties().vendor_id, !integrated, card_memory));

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
        Gpu {
            instance,
            device,
            queue,
            memory_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,
        }
    }
}

/// The compute shaders the world's drawn with and its water moved
struct Pipelines {
    render: HotReloadComputePipeline,
    post: HotReloadComputePipeline,
    resample: HotReloadComputePipeline,
    upscale: HotReloadComputePipeline,
    water: HotReloadComputePipeline,
    shafts: HotReloadComputePipeline,
    skips: HotReloadComputePipeline,
}

impl Pipelines {
    // Read from where they are while working on them, so they reload when changed. Copies are
    // built in for running elsewhere.
    const SHADERS: [(&str, &str); 7] = [
        ("traverse.comp", include_str!("../shaders/traverse.comp")),
        ("post.comp", include_str!("../shaders/post.comp")),
        ("resample.comp", include_str!("../shaders/resample.comp")),
        ("upscale.comp", include_str!("../shaders/upscale.comp")),
        ("water.comp", include_str!("../shaders/water.comp")),
        ("shafts.comp", include_str!("../shaders/shafts.comp")),
        ("skips.comp", include_str!("../shaders/skips.comp")),
    ];

    fn path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("shaders").join(name)
    }

    /// Whether they've all been compiled before as they are now, which makes them quick to get
    fn cached() -> bool {
        Self::SHADERS.iter().all(|(name, built_in)| hot_reload::is_cached(&Self::path(name), built_in))
    }

    /// Each made on a thread of its own, as compiling traverse.comp takes far the longest. None if
    /// one of them couldn't be made, which has been said already.
    fn new(device: &Arc<Device>) -> Option<Self> {
        let making = Self::SHADERS.map(|(name, built_in)| {
            let device = device.clone();
            std::thread::spawn(move || HotReloadComputePipeline::new(device, &Self::path(name), built_in))
        });
        let [render, post, resample, upscale, water, shafts, skips] = making;
        Some(Pipelines {
            render: render.join().ok()?,
            post: post.join().ok()?,
            resample: resample.join().ok()?,
            upscale: upscale.join().ok()?,
            water: water.join().ok()?,
            shafts: shafts.join().ok()?,
            skips: skips.join().ok()?,
        })
    }
}

/// Where the game starts, and the piece of the landscape around it, built while the graphics card
/// starts up, as it doesn't need it
struct Placed {
    scene: Scene,
    terrain: TerrainSettings,
    rivers: Arc<RiverCache>,
    /// The world that moves on over the landscape a strip at a time, see stream.rs
    world: World,
    /// What it has besides its voxels, and where rivers flow into it
    pieces: Vec<worldgen::PieceFeatures>,
    river_springs: Vec<[f32; 4]>,
    camera: Camera,
    // Whether the player starts right where the camera is, in a dungeon, a cave, a castle or a
    // town or city down under the ground, rather than up on the ground over it
    indoors: bool,
    /// How the graphics card's memory is shared out, see memory.rs
    memory: memory::Plan,
}

impl Placed {
    fn place(progress: &Arc<loading::Progress>) -> Self {
        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());

        let scene = Scene::Terrain;
        let mut terrain = INITIAL_TERRAIN;
        // As fine and as wide as the quality preset wants it, see quality.rs
        let preset = quality::chosen().settings();
        terrain.voxels_per_meter = setting("voxels_per_meter").and_then(|v| v.parse().ok()).unwrap_or(preset.voxels_per_meter);
        terrain.width_m = setting("width_m").and_then(|v| v.parse().ok()).unwrap_or(preset.world_width_m);
        // As wide as asked for, or as there's room for in the graphics card's memory, once it's
        // been picked, see memory.rs
        let window = setting("window").and_then(|v| v.split_once('x')).and_then(|(w, h)| Some([w.parse().ok()?, h.parse().ok()?])).unwrap_or([1920, 1080]);
        let render = perf::render_bytes(render_extent(window, DEFAULT_RENDER_SCALE), window);
        let memory = memory::plan(memory::card_budget(), terrain.width_m, terrain.height_m, terrain.voxels_per_meter, render);
        terrain.width_m = memory.width_m;
        println!("Memory plan: {memory:?}");
        if let Some(v) = setting("tufts") {
            terrain.grass_tufts = v == "true" || v == "1";
        }
        let rivers = Arc::new(RiverCache::default());
        // For trying out a dungeon: `--set dungeon=1` starts at the way into the nearest one
        let dungeon = setting("dungeon").and_then(|_| {
            let (_, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, [0, 0], &rivers);
            landscape.and_then(|l| l.nearest_dungeon([112.0, 112.0]))
        });
        // `--set cave=1` starts in the great cavern at the end of the nearest cave, where a boss is
        let cavern = setting("cave").filter(|_| dungeon.is_none()).and_then(|_| {
            let (_, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, [0, 0], &rivers);
            landscape.and_then(|l| l.nearest_cavern([112.0, 112.0]))
        });
        // `--set town=1` starts in the square of the nearest town, `town=far` past the end of one of
        // its streets, looking down it, `town=stall` in front of a merchant's stall on its square,
        // `town=cave` in the square of the nearest town down in a cavern and `town=tunnel` outside
        // the mouth of the tunnel down to it. With `biome=snow` too, the nearest built for the snow.
        let town_view = setting("town").filter(|_| dungeon.is_none() && cavern.is_none());
        let underground = matches!(town_view, Some("cave" | "tunnel"));
        let town = town_view.and_then(|_| {
            let kind = setting("biome");
            let near = kind.and_then(|kind| worldgen::find_climate(&terrain, kind, [160.0, 160.0])).unwrap_or([112.0, 112.0]);
            let (_, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, [0, 0], &rivers);
            landscape.and_then(|l| l.nearest_town(near, underground, kind))
        });
        // `--set royal=1` starts out in front of the gate of the nearest royal castle looking up at
        // it, `royal=court` in its courtyard looking up at the keep, `royal=hall` inside the door of
        // its great hall, `royal=throne` in its throne room facing the princess, and `royal=air`
        // high over one front corner. With `biome=snow` too, the nearest to the snow, and with
        // `castle=small`, `medium` or `large` the nearest of that size.
        let royal_view = setting("royal").filter(|_| dungeon.is_none() && cavern.is_none() && town.is_none());
        let inside_castle = matches!(royal_view, Some("hall" | "throne" | "court"));
        let royal = royal_view.and_then(|_| {
            let near = setting("biome").and_then(|kind| worldgen::find_climate(&terrain, kind, [160.0, 160.0])).unwrap_or([112.0, 112.0]);
            let (_, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, [0, 0], &rivers);
            landscape.and_then(|l| l.nearest_castle(near, setting("castle")))
        });
        // `--set undercity=1` starts at the top of the stair down into the nearest fallen city,
        // looking down the shaft, `undercity=entrance` out on the land looking at the way down,
        // `undercity=air` high over it, and `foot`, `gate`, `street`, `plaza`, `canal`, `hall` or
        // `heart` down in the city: at the foot of the shaft, inside its gate, on its main street,
        // in its square, down in its canal, in front of its temple or in the temple's hall
        let sunken_view = setting("undercity").filter(|_| dungeon.is_none() && cavern.is_none() && town.is_none() && royal.is_none());
        let in_city = matches!(sunken_view, Some("foot" | "gate" | "street" | "plaza" | "canal" | "hall" | "heart"));
        let sunken = sunken_view.and_then(|_| {
            let (_, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, [0, 0], &rivers);
            landscape.and_then(|l| l.nearest_undercity([112.0, 112.0]))
        });
        // `--set river=1` starts over the widest river within half a kilometer
        let river_at = setting("river").filter(|_| dungeon.is_none() && cavern.is_none() && town.is_none()).and_then(|_| {
            let (_, landscape) = build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, [0, 0], &rivers);
            landscape.and_then(|l| l.widest_river_near([112.0, 112.0], &rivers))
        });
        // `--set camp=fort` starts outside the nearest bandits' camp of that kind, or `cabins`,
        // `ruin`, `hideout`, `raiders`, `awnings` or `yurts`, on the land `biome` says if it does.
        // Where they're planned is looked over in rough pieces of the landscape, built quickly, for
        // the first that really is there with nothing in the way.
        let camp_kind = setting("camp").and_then(worldgen::CampKind::named).filter(|_| dungeon.is_none() && cavern.is_none());
        let camp_at = camp_kind.and_then(|kind| {
            let near = setting("biome").and_then(|b| worldgen::find_climate(&terrain, b, [160.0, 160.0])).unwrap_or([112.0, 112.0]);
            let rough = TerrainSettings { voxels_per_meter: 4, grass_tufts: false, ..terrain };
            worldgen::camp_spots(&terrain, kind, near, setting("biome")).into_iter().take(6).find_map(|spot| {
                let origin = [0, 1].map(|i| ((spot[i] * 4.0) as i64 - rough.size()[i] as i64 / 2).div_euclid(64) * 64);
                let (landscape, _) = worldgen::generate_terrain(&rough, origin, &rivers);
                let off = |c: &&worldgen::Camp| (c.fire[0] - spot[0]).hypot(c.fire[1] - spot[1]);
                landscape.bandit_camps().iter().filter(|c| c.kind == kind).min_by(|a, b| off(a).total_cmp(&off(b))).map(|c| [c.fire[0], c.fire[1]])
            })
        });
        // For trying out each kind of land: `--set biome=jungle` starts in the nearest jungle
        let start_in = camp_at
            .or(dungeon.map(|d| [d[0], d[1]]))
            .or(cavern.map(|(floor, ..)| [floor[0], floor[1]]))
            .or(town.map(|(square, _)| [square[0], square[1]]))
            .or(royal.as_ref().map(|r| {
                // Out in front of it, there's room in the world for the view of it from there
                let out = if inside_castle { 0.0 } else { 25.0 };
                [r.middle[0] + r.out[0] * out, r.middle[1] + r.out[1] * out]
            }))
            // Halfway from the foot of the shaft to the temple, so all of it's in the world
            .or(sunken.as_ref().map(|s| [(s.foot[0] + s.heart[0]) * 0.5, (s.foot[1] + s.heart[1]) * 0.5]))
            .or(river_at)
            .or_else(|| setting("biome").and_then(|kind| worldgen::find_climate(&terrain, kind, [160.0, 160.0])));
        // Lined up with the strips the world moves on by, see stream.rs
        let strip = (stream::STRIP_M * terrain.voxels_per_meter) as i64;
        // Otherwise round where the camera starts, as it would in a piece of the landscape at its
        // corner, or where `--set camera=x,y,z` says in that: the world moves on a strip at a time
        // from there, so it's to start out round the camera, not 18 m from its edge
        let start_camera = setting("camera").and_then(parse_vector).unwrap_or(scene.initial_camera().position);
        // As in the widest piece there is, so it starts at the same spot, the lake shore on the
        // default seed, however wide the world's planned to be: counted in a narrower one, a 2 GB
        // card's started out somewhere else altogether, walled in
        let widest = memory::WIDTHS_M[0] * terrain.voxels_per_meter;
        let start_at = camera_to_voxels(start_camera, [widest, widest, terrain.size()[2]]);
        let origin = match start_in {
            Some(spot) => {
                let (vpm, size) = (terrain.voxels_per_meter as f64, terrain.size());
                [0, 1].map(|i| ((spot[i] as f64 * vpm) as i64 - size[i] as i64 / 2 + strip / 2).div_euclid(strip) * strip)
            }
            // The nearest whole number of strips, so it's no more than half a strip from the middle
            // and the world doesn't move on straight away
            None => [0, 1].map(|i| (start_at[i] as i64 - terrain.size()[i] as i64 / 2 + strip / 2).div_euclid(strip) * strip),
        };
        // Where the camera starts in that world
        let start_camera = voxels_to_camera(start_at - Vector3::new(origin[0] as f64, origin[1] as f64, 0.0), terrain.size());
        let (world, landscape) = progress.building_world(|| build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, origin, &rivers));
        // The first piece of the world is built with every core, the rest in the background while
        // the game goes on, leaving it some
        worldgen::leave_cores(2);
        if setting("sites").is_some()
            && let Some(l) = &landscape
        {
            let mut kinds: Vec<&str> = l.sites().iter().map(|s| s.kind).collect();
            kinds.sort();
            kinds.dedup();
            println!("Sites: {}", kinds.iter().map(|k| format!("{k} ×{}", l.sites().iter().filter(|s| s.kind == *k).count())).collect::<Vec<_>>().join(", "));
        }
        // `--set falls_cave=1` starts in the cave behind the waterfall nearest the middle of the
        // world, if there's one
        let falls_cave = setting("falls_cave").filter(|_| dungeon.is_none() && cavern.is_none()).and_then(|_| {
            let middle = [0, 1].map(|i| (origin[i] as f32 + world.size[i] as f32 * 0.5) / world.voxels_per_meter as f32);
            let off = |s: &&worldgen::Site| (s.position[0] - middle[0]).hypot(s.position[1] - middle[1]);
            landscape.as_ref()?.sites().iter().filter(|s| s.kind == "waterfall cave").min_by(|a, b| off(a).total_cmp(&off(b))).map(|s| (s.position, s.facing))
        });
        // Over that river, looking down it
        let river = river_at.and_then(|_| landscape.as_ref()?.widest_river());
        // `--set camp=1` starts outside the bandits' camp nearest the middle of the world, and
        // `camp=fort` and so on outside the one of that kind
        let camp = setting("camp").filter(|_| dungeon.is_none() && cavern.is_none()).and_then(|_| {
            let middle = [0, 1].map(|i| (origin[i] as f32 + world.size[i] as f32 * 0.5) / world.voxels_per_meter as f32);
            let off = |c: &&worldgen::Camp| (c.fire[0] - middle[0]).hypot(c.fire[1] - middle[1]);
            let camps = landscape.as_ref()?.bandit_camps().iter().filter(|c| camp_kind.is_none_or(|k| c.kind == k));
            let c = camps.min_by(|a, b| off(a).total_cmp(&off(b)))?;
            println!(
                "Camp: {} at {:.0}, {:.0}, {} bandits and {} brutes{}, its hoard {} with luck {:.1}",
                c.kind.name(),
                c.fire[0],
                c.fire[1],
                c.bandits,
                c.brutes,
                if c.warlord { " led by a warlord" } else { "" },
                c.hint,
                c.luck
            );
            Some(*c)
        });

        let mut camera = scene.initial_camera();
        if start_in.is_none() {
            camera.position = start_camera;
        }
        if let Some(door) = dungeon {
            // On the landing in front of the door looking towards the halls below, or with
            // `--set dungeon=hall` or `boss` in the hall nearest it or the boss's
            let vpm = world.voxels_per_meter as f64;
            let at = |p: [f32; 3]| Vector3::new(p[0] as f64 * vpm - origin[0] as f64, p[1] as f64 * vpm - origin[1] as f64, p[2] as f64 * vpm);
            let mut eye = at(door) + Vector3::new(0.0, 0.0, 1.7 * vpm);
            let halls: Vec<(&str, Vector3<f64>)> = landscape.as_ref().map_or(Vec::new(), |l| l.sites().iter().filter(|s| s.kind.starts_with("dungeon")).map(|s| (s.kind, at(s.position))).collect());
            let nearest = |kind: &str, from: Vector3<f64>| halls.iter().filter(|(k, _)| *k == kind).map(|&(_, p)| p).min_by(|a, b| (a - from).norm().total_cmp(&(b - from).norm()));
            let hall = nearest("dungeon hall", eye);
            match setting("dungeon") {
                Some("hall") => eye = hall.map_or(eye, |h| h + Vector3::new(-3.0 * vpm, 0.0, 1.7 * vpm)),
                Some("boss") => eye = nearest("dungeon", eye).map_or(eye, |h| h + Vector3::new(-7.0 * vpm, 0.0, 1.7 * vpm)),
                _ => {}
            }
            camera.position = voxels_to_camera(eye, world.size);
            let towards = hall.filter(|h| (h - eye).xy().norm() > vpm).map_or(Vector3::x(), |h| Vector3::new(h.x - eye.x, h.y - eye.y, 0.0).normalize());
            camera.look_direction(towards + Vector3::new(0.0, 0.0, -0.2));
        } else if let Some((floor, way_in, wide)) = cavern {
            // Inside the great cavern, near the way in, looking across it
            let vpm = world.voxels_per_meter as f64;
            let middle = Vector3::new(floor[0] as f64 * vpm - origin[0] as f64, floor[1] as f64 * vpm - origin[1] as f64, floor[2] as f64 * vpm);
            let back = Vector3::new(way_in[0] as f64, way_in[1] as f64, 0.0) * wide as f64 * 0.8 * vpm;
            let eye = middle + back + Vector3::new(0.0, 0.0, 1.7 * vpm);
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(middle + Vector3::new(0.0, 0.0, 1.0 * vpm), world.size));
        } else if let Some((inside, out)) = falls_cave {
            // Just inside the cave behind a waterfall, looking out through it
            let vpm = world.voxels_per_meter as f64;
            let at = Vector3::new(inside[0] as f64 * vpm - origin[0] as f64, inside[1] as f64 * vpm - origin[1] as f64, inside[2] as f64 * vpm + 1.5 * vpm);
            // `left` or `right` looks along the way in instead
            let out = match setting("falls_cave") {
                Some("left") => Vector3::new(-out[1] as f64, out[0] as f64, -0.1),
                Some("right") => Vector3::new(out[1] as f64, -out[0] as f64, -0.1),
                _ => Vector3::new(out[0] as f64, out[1] as f64, -0.15),
            };
            camera.position = voxels_to_camera(at, world.size);
            camera.look_at(voxels_to_camera(at + out * 4.0 * vpm, world.size));
        } else if let Some((at, flow, wide)) = river {
            let vpm = world.voxels_per_meter as f64;
            let middle = Vector3::new(at[0] as f64 * vpm - origin[0] as f64, at[1] as f64 * vpm - origin[1] as f64, at[2] as f64 * vpm);
            let flow = Vector3::new(flow[0] as f64, flow[1] as f64, 0.0);
            println!("The widest river about is {:.1} m across", wide * 2.0);
            // `river=high` from high over it, to see where it runs
            let (up, back) = if setting("river") == Some("high") { (60.0, 40.0) } else { (12.0, 16.0) };
            let mut eye = middle + (Vector3::new(0.0, 0.0, up) - flow * back) * vpm;
            eye.z = eye.z.min(world.size[2] as f64 - 2.0 * vpm);
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(middle + flow * 16.0 * vpm, world.size));
        } else if let Some(c) = &camp {
            // Out past the way in, `--set camp_distance=16` meters, looking in at the fire, a little
            // over the ground there. `--set camp_view=chest` is in front of where the hoard turns
            // up, looking at it, and `camp_view=air` high over the way in, looking down on it all.
            let vpm = world.voxels_per_meter as f64;
            let at = |p: [f32; 3]| Vector3::new(p[0] as f64 * vpm - origin[0] as f64, p[1] as f64 * vpm - origin[1] as f64, p[2] as f64 * vpm);
            let up = |m: f64| Vector3::new(0.0, 0.0, m * vpm);
            let out = Vector3::new(c.way[0] as f64, c.way[1] as f64, 0.0);
            let distance = setting("camp_distance").and_then(|v| v.parse::<f64>().ok()).unwrap_or(16.0);
            let (eye, target) = match setting("camp_view") {
                Some("chest") => {
                    let facing = Vector3::new(c.heading.cos() as f64, c.heading.sin() as f64, 0.0);
                    (at(c.chest) + facing * 2.4 * vpm + up(1.3), at(c.chest) + up(0.3))
                }
                Some("air") => (at(c.fire) + out * (distance + 4.0) * vpm + up(distance), at(c.fire)),
                _ => {
                    let eye = at(c.fire) + out * distance * vpm;
                    let ground = (0..world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(world.material_at(eye.x as i64, eye.y as i64, z))).unwrap_or(0) as f64;
                    (Vector3::new(eye.x, eye.y, (eye.z + 2.5 * vpm).max(ground + 1.8 * vpm)), at(c.fire) + up(1.0))
                }
            };
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(target, world.size));
        } else if let Some(r) = &royal {
            let vpm = world.voxels_per_meter as f64;
            let at = |p: [f32; 3]| Vector3::new(p[0] as f64 * vpm - origin[0] as f64, p[1] as f64 * vpm - origin[1] as f64, p[2] as f64 * vpm);
            let up = |m: f64| Vector3::new(0.0, 0.0, m * vpm);
            let (eye, target) = match royal_view {
                Some("hall") => (at(r.hall) + up(1.7), at(r.dais) + up(1.6)),
                Some("throne") => (at(r.throne) + up(1.65), at(r.princess) + up(1.3)),
                Some("court") => (at(r.court) + up(1.7), at(r.tower)),
                // From high over one front corner, to see it all
                Some("air") => {
                    let (out, side) = (Vector3::new(r.out[0] as f64, r.out[1] as f64, 0.0), Vector3::new(-r.out[1] as f64, r.out[0] as f64, 0.0));
                    (at(r.middle) + (out * 70.0 + side * 45.0) * vpm + up(18.0), at(r.middle) + up(6.0))
                }
                _ => {
                    // Out along the way up to the gate, a little over the ground there
                    let out = at(r.gate) + Vector3::new(r.out[0] as f64, r.out[1] as f64, 0.0) * 30.0 * vpm;
                    let ground = (0..world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(world.material_at(out.x as i64, out.y as i64, z))).unwrap_or(0) as f64;
                    (Vector3::new(out.x, out.y, (ground + 3.0 * vpm).max(at(r.gate).z + 4.0 * vpm)), at(r.middle) + up(9.0))
                }
            };
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(target, world.size));
        } else if let Some(s) = &sunken {
            let vpm = world.voxels_per_meter as f64;
            let at = |p: [f32; 3]| Vector3::new(p[0] as f64 * vpm - origin[0] as f64, p[1] as f64 * vpm - origin[1] as f64, p[2] as f64 * vpm);
            let up = |m: f64| Vector3::new(0.0, 0.0, m * vpm);
            let ahead = Vector3::new(s.forward[0] as f64, s.forward[1] as f64, 0.0);
            let out = Vector3::new(s.out[0] as f64, s.out[1] as f64, 0.0);
            let ground = |p: Vector3<f64>| (0..world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(world.material_at(p.x as i64, p.y as i64, z))).unwrap_or(0) as f64 + 1.0;
            let (eye, target) = match sunken_view {
                Some("entrance") => {
                    // Out on the land, a little way off, looking back at the way down
                    let eye = at(s.entrance) + out * 16.0 * vpm;
                    (Vector3::new(eye.x, eye.y, ground(eye) + 3.5 * vpm), at(s.entrance) + up(0.5))
                }
                Some("air") => {
                    let eye = at(s.entrance) + out * 24.0 * vpm;
                    (Vector3::new(eye.x, eye.y, ground(eye) + 26.0 * vpm), at(s.foot) + up(6.0))
                }
                Some("foot") => (at(s.foot) + up(1.7) - ahead * 2.5 * vpm, at(s.gate) + up(1.4)),
                Some("gate") => (at(s.gate) + up(1.8), at(s.plaza) + up(2.0)),
                Some("street") => (at(s.street) + up(1.8) - ahead * 1.5 * vpm, at(s.plaza) + up(2.2)),
                Some("plaza") => (at(s.plaza) + up(2.2) - ahead * 3.0 * vpm, at(s.plaza) + ahead * 14.0 * vpm + up(2.2)),
                Some("canal") => (at(s.canal) + up(1.7), at(s.street) + up(1.0)),
                Some("hall") => (at(s.hall) + up(1.8) - ahead * 4.0 * vpm, at(s.heart) + up(2.0)),
                Some("heart") => (at(s.heart) + up(2.2) - ahead * 4.5 * vpm, at(s.heart) + ahead * 6.0 * vpm + up(2.2)),
                // Over the shaft by the top of the stair, looking down it at its far side
                _ => {
                    let eye = at(s.foot) + out * 3.0 * vpm;
                    (Vector3::new(eye.x, eye.y, at(s.head).z + 2.5 * vpm), at(s.foot) - out * 3.0 * vpm + up(1.0))
                }
            };
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(target, world.size));
        } else if let Some((square, way_in)) = town {
            // In the square looking across it, or out by the way in looking towards it
            let vpm = world.voxels_per_meter as f64;
            let at = |p: [f32; 3]| Vector3::new(p[0] as f64 * vpm - origin[0] as f64, p[1] as f64 * vpm - origin[1] as f64, p[2] as f64 * vpm);
            let (square, way_in) = (at(square), at(way_in));
            let out = Vector3::new(way_in.x - square.x, way_in.y - square.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
            let up = |m: f64| Vector3::new(0.0, 0.0, m * vpm);
            let stall = landscape.as_ref().and_then(|l| {
                let sites = l.sites().iter().filter(|s| s.kind == "town stall");
                sites.map(|s| (at(s.position), Vector3::new(s.facing[0] as f64, s.facing[1] as f64, 0.0))).min_by(|a, b| (a.0 - square).norm().total_cmp(&(b.0 - square).norm()))
            });
            let (eye, target) = match (town_view, stall) {
                (Some("far"), _) => (way_in + up(5.0), square + up(1.5)),
                (Some("tunnel"), _) => (way_in + out * 7.0 * vpm + up(1.7), way_in + up(1.4)),
                // Where a customer would stand, the merchant behind the counter
                (Some("stall"), Some((spot, facing))) => (spot + facing * 3.2 * vpm + up(1.6), spot + up(1.2)),
                // A little way along a street, looking down it across the square
                _ => (square + out * 10.0 * vpm + up(1.7), square - out * 8.0 * vpm + up(1.2)),
            };
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(target, world.size));
        } else if start_in.is_some() {
            // Over the middle of the world, a little above the ground, looking out across it
            let [sx, sy, sz] = world.size.map(|n| n as i64);
            let ground = (0..sz).rev().find(|&z| !worldgen::is_passable(world.material_at(sx / 2, sy / 2, z))).unwrap_or(0);
            // `--set rise=20` meters over the ground instead, and `--set look=-1,0.5,-0.4` looking
            // that way, for views of the land around from wherever it starts
            let rise = setting("rise").and_then(|v| v.parse::<f64>().ok()).unwrap_or(4.0);
            let eye = Vector3::new(sx as f64 * 0.5, sy as f64 * 0.5, ground as f64 + rise * world.voxels_per_meter as f64);
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_direction(setting("look").and_then(parse_vector).unwrap_or(Vector3::new(1.0, 0.3, -0.25)));
        }
        if setting("camera").and_then(parse_vector).is_some() {
            camera.position = start_camera;
        }
        // `--set bush=1` puts the camera inside the nearest bush, to see out of it
        if setting("bush").is_some() {
            let eye = camera_to_voxels(camera.position, world.size);
            let vpm = world.voxels_per_meter as i64;
            let leafy = |m: u8| matches!(m, worldgen::material::LEAVES | worldgen::material::LEAVES_DARK);
            let mut best: Option<(i64, Vector3<f64>)> = None;
            for dy in (-30 * vpm..=30 * vpm).step_by(4) {
                for dx in (-30 * vpm..=30 * vpm).step_by(4) {
                    let (x, y) = (eye.x as i64 + dx, eye.y as i64 + dy);
                    let Some(ground) = (0..world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(world.material_at(x, y, z)) && !leafy(world.material_at(x, y, z))) else {
                        continue;
                    };
                    let z = ground + vpm;
                    if leafy(world.material_at(x, y, z)) && leafy(world.material_at(x, y, z + vpm / 2)) && best.is_none_or(|(d, _)| dx * dx + dy * dy < d) {
                        best = Some((dx * dx + dy * dy, Vector3::new(x as f64, y as f64, z as f64)));
                    }
                }
            }
            if let Some((_, inside)) = best {
                camera.position = voxels_to_camera(inside, world.size);
            }
        }
        if let Some(target) = setting("look_at").and_then(parse_vector) {
            camera.look_at(target);
        }
        let indoors = dungeon.is_some() || cavern.is_some() || town.is_some() && underground || royal.is_some() && inside_castle || sunken.is_some() && in_city;
        // Into the world that moves on from here a strip at a time
        let (world, pieces, river_springs) = match &landscape {
            Some(landscape) => into_window(world, landscape),
            None => (world, Vec::new(), Vec::new()),
        };
        Placed { scene, terrain, rivers, world, pieces, river_springs, camera, indoors, memory }
    }
}

/// What App::new() puts the game together from: everything that takes a while, made on the loader
/// thread while the loading screen's up, see loading.rs
struct Loaded {
    gpu: Gpu,
    pipelines: Pipelines,
    sun_map: Arc<ImageView>,
    placed: Placed,
    voxel_set: GpuWorld,
    empty_near_set: Arc<DescriptorSet>,
    water: Water,
}

impl Loaded {
    /// Starts the graphics card for drawing to windows on `display` while the world's built on
    /// another thread, hands it to `started` as soon as it's going, to draw the loading screen
    /// with, then sends the world over to it. None if loading was called off, or something went
    /// wrong, which has been said already.
    fn load(display: &impl HasDisplayHandle, progress: &Arc<loading::Progress>, started: impl FnOnce(&Gpu)) -> Option<Self> {
        let (placing, placed) = std::sync::mpsc::channel();
        {
            let progress = progress.clone();
            std::thread::Builder::new()
                .name("world".into())
                .spawn(move || {
                    let _ = placing.send(Placed::place(&progress));
                })
                .unwrap();
        }
        let gpu = progress.run(Step::Graphics, || Gpu::new(display));
        started(&gpu);
        if progress.cancelled() {
            return None;
        }
        if !Pipelines::cached() {
            progress.compiling_shaders();
        }
        note_vram(&gpu.device, "the graphics card started");
        let pipelines = progress.run(Step::Shaders, || Pipelines::new(&gpu.device))?;
        note_vram(&gpu.device, "the shaders made");
        // Building the world never touches the graphics card, so it's left to it should the window
        // be closed meanwhile, rather than waited for
        let placed = loop {
            match placed.recv_timeout(Duration::from_millis(50)) {
                Ok(placed) => break placed,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if !progress.cancelled() => continue,
                // Or it went wrong, which has been said
                Err(_) => return None,
            }
        };
        if progress.cancelled() {
            return None;
        }

        let settings = settings_from_args();
        let flag = |key: &str, default: bool| settings.iter().find(|(k, _)| k == key).map_or(default, |(_, v)| v == "true" || v == "1");
        let Gpu { memory_allocator, descriptor_set_allocator, command_buffer_allocator, queue, .. } = &gpu;
        let Placed { world, pieces, river_springs, memory, .. } = &placed;
        let (voxel_set, empty_near_set) = progress.run(Step::Upload, || {
            let voxel_set = get_voxel_set(
                memory_allocator.clone(),
                command_buffer_allocator.clone(),
                descriptor_set_allocator.clone(),
                &pipelines.render,
                queue,
                1,
                world,
                memory.bricks,
            );
            // Bound in place of the fine detail when there isn't any
            let empty_near_set = get_voxel_set(
                memory_allocator.clone(),
                command_buffer_allocator.clone(),
                descriptor_set_allocator.clone(),
                &pipelines.render,
                queue,
                2,
                &World::from_packed_occupancy([8; 3], &[0; 4], worldgen::material::AIR),
                0,
            )
            .set;
            (voxel_set, empty_near_set)
        });
        if progress.cancelled() {
            return None;
        }

        note_vram(&gpu.device, "the world sent over");
        let water = progress.run(Step::Water, || {
            let mut springs = river_springs.clone();
            springs.extend(pieces.iter().flat_map(|p| p.springs.iter().copied()));
            let mut water = Water::new(memory_allocator, command_buffer_allocator, descriptor_set_allocator, queue, &pipelines.water, world, &springs);
            water.active &= flag("water", true);
            water
        });

        note_vram(&gpu.device, "the water made");
        let sun_map = get_history_image_view(
            memory_allocator.clone(),
            [SUN_MAP_SIZE, SUN_MAP_SIZE],
            Format::R32_SFLOAT,
        );

        Some(Loaded { gpu, pipelines, sun_map, placed, voxel_set, empty_near_set, water })
    }
}

impl App {
    /// Puts the game together from what was loaded, with what can't be made on another thread and
    /// sent over: the game itself, with the weapons the player starts with, and the sound. The
    /// loading screen's window is handed over after, see attach().
    fn new(loaded: Loaded) -> Self {
        let Loaded { gpu, pipelines, sun_map, placed, voxel_set, empty_near_set, water } = loaded;
        // The instance was only needed to make the window's surface, which the loading screen did
        let Gpu { instance: _, device, queue, memory_allocator, descriptor_set_allocator, command_buffer_allocator } = gpu;
        let Pipelines { render: render_pipeline, post: post_pipeline, resample: resample_pipeline, upscale: upscale_pipeline, water: water_pipeline, shafts: shafts_pipeline, skips: skips_pipeline } = pipelines;
        let Placed { scene, terrain, rivers, mut world, pieces, river_springs, mut camera, indoors, memory } = placed;
        let stream = scene.is_terrain().then(|| stream::Stream::new(stream::STRIP_M * world.voxels_per_meter, pieces, river_springs, memory_allocator.clone()));
        // The water has what it needs of it now
        world.forget_water_surface();
        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        // The preset asked for, chosen before, or that suits the graphics card, the same the world
        // was built for, see Placed::place()
        let quality = quality::chosen();
        let preset = quality.settings();
        println!("Quality: {} (sun shafts {})", quality.name(), preset.sun_shafts.name());
        water::set_flowing_area((setting("flowing_water_m").and_then(|v| v.parse().ok()).unwrap_or(preset.flowing_water_m) * world.voxels_per_meter as f32) as u32);

        let input = WinitInputHelper::new();
        let flag = |key: &str, default: bool| setting(key).map_or(default, |v| v == "true" || v == "1");

        // Start on foot, except for screenshot runs which need the camera to stay put
        let capture = Capture::from_args();
        let water_level = |x: f64, y: f64| water.surface_at(x, y);

        let walk = scene.is_terrain() && flag("walk", capture.is_none());
        // In a dungeon or a cave, right where the camera is rather than up on the ground over it
        let eye = camera_to_voxels(camera.position, world.size);
        let player = walk.then(|| if indoors { Player::new(eye, &world) } else { Player::on_dry_ground(eye, &world, &water_level) });

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
        // `--set ammo=fire_bolts` gives a few bundles of that special ammo and loads it
        if let Some(id) = setting("ammo") {
            game.give_ammo(id);
        }
        // `--set special=meteors` puts a weapon with that special attack in hand, charged, and
        // `special_family=handcrossbow` one of that family, or `weapon=handcrossbow` without it
        if let Some(name) = setting("special")
            && let Ok(special) = serde_json::from_str::<weapons::Special>(&format!("\"{name}\""))
        {
            game.find_special(special, setting("special_family").or(setting("weapon")));
            println!("special: {}", game.describe_held());
        }
        // `--set gallery=pitted_blade,leafsteel_blade` lays a weapon made with each of those parts on
        // the ground ahead, in rows of four, to look at them, and `gallery_size=4` makes them all
        // that size, see weapons::SIZES
        if let Some(ids) = setting("gallery") {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            let ids: Vec<&str> = ids.split(',').collect();
            // `gallery_hold=1` puts the first of them in hand too. The camera's brought down to look
            // at them from just behind and over them.
            let (middle, longest) = game.lay_out(&world, &ids, setting("gallery_size").and_then(|v| v.parse().ok()), start, ahead, flag("gallery_hold", false));
            let vpm = world.voxels_per_meter as f64;
            let flat = Vector3::new(ahead.x, ahead.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
            if !flag("gallery_hold", false) {
                // Close over one or two, as close as the longest fits in view whichever way it's
                // turned, further back over more
                let span = longest.max(0.8) + 0.5;
                let (back, up) = if ids.len() <= 2 { (0.42 * span, 0.75 * span) } else { (1.7, 2.3) };
                camera.position = voxels_to_camera(middle - flat * back * vpm + Vector3::new(0.0, 0.0, up * vpm), world.size);
                // At them, hovering a hand over the ground
                camera.look_at(voxels_to_camera(middle + Vector3::new(0.0, 0.0, 0.3 * vpm), world.size));
            }
        }
        // `--set staff_element=fire` puts a staff of fire in hand, or of another element, or `none`
        if let Some(name) = setting("staff_element") {
            let element = serde_json::from_str::<weapons::Element>(&format!("\"{name}\"")).ok();
            game.find_weapon("staff", element, 2.0);
        }
        // `--set boss=golem`, `frostwyrm`, `broodmother` and so on puts one a little way ahead, to
        // try fighting it
        let boss = setting("boss").and_then(|name| {
            let named = |l: &animals::Look| format!("{l:?}").eq_ignore_ascii_case(name) || l.name().replace(' ', "").eq_ignore_ascii_case(name);
            animals::Look::ALL.into_iter().find(|l| l.build().is_some() && named(l))
        });
        if let Some(look) = boss {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            let distance = setting("boss_distance").and_then(|v| v.parse().ok()).unwrap_or(20.0);
            game.summon(&world, &water_level, look, start, ahead, distance);
            // `--set boss_health=0.6` starts it hurt, to see what it does later in a fight
            if let Some(fraction) = setting("boss_health").and_then(|v| v.parse::<f64>().ok()) {
                game.hurt_bosses(fraction);
            }
        }
        // `--set enemy=crossbowman`, `shieldbearer`, `burrower`, `wraith`, `lurker`, `gloomwing`,
        // `tusker` or any other monster puts a pack of them a little way ahead, or `gang` a gang of
        // bandits, to try fighting them. `enemy_count=1` makes it that many, `enemy_distance=8`
        // that many meters off, `enemy_turn=90` turns them that many degrees from facing the
        // player, `enemy_calm=1` leaves them standing there not noticing the player, and
        // `enemy_pose=windup`, `strike` or `special` holds them in that pose, to look at them with
        // `--script enemy`.
        if let Some(name) = setting("enemy") {
            // Its name in full or the end of it, so `burrower` finds the dune burrower
            let wanted = name.to_lowercase();
            let named = |l: &animals::Look| [format!("{l:?}").to_lowercase(), l.name().replace(' ', "").to_lowercase()].iter().any(|n| n.ends_with(&wanted));
            let looks: Vec<animals::Look> = if name == "gang" {
                animals::gang(0.4).to_vec()
            } else {
                // A boss too, `enemy=graveknight`, to look at it held in a pose
                let monster = animals::Look::ALL.into_iter().find(|l| l.is_monster() && named(l));
                match monster.or_else(|| animals::Look::ALL.into_iter().find(|l| l.is_boss() && named(l))) {
                    Some(look) => vec![look; setting("enemy_count").and_then(|v| v.parse().ok()).unwrap_or(look.pack_size())],
                    None => {
                        println!("No monster called {name}");
                        Vec::new()
                    }
                }
            };
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            let distance = setting("enemy_distance").and_then(|v| v.parse().ok()).unwrap_or(12.0);
            let pose = match setting("enemy_pose") {
                Some("windup") => Some(animals::POSE_WINDUP),
                Some("strike") => Some(animals::POSE_STRIKE),
                Some("special") => Some(animals::POSE_SPECIAL),
                // A knight's own
                Some("coil") => Some(animals::POSE_COIL),
                Some("sweep") => Some(animals::POSE_SWEEP),
                Some("raise") => Some(animals::POSE_RAISE),
                Some("spikes") => Some(animals::POSE_SPIKES),
                _ => None,
            };
            let turn = setting("enemy_turn").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0).to_radians();
            game.summon_pack(&world, &water_level, &looks, start, ahead, distance, turn, flag("enemy_calm", false), pose);
            println!("enemy: {}", looks.iter().map(|l| l.name()).collect::<Vec<_>>().join(", "));
        }
        // `--set health=10` starts the player that hurt, to see them die
        if let Some(health) = setting("health").and_then(|v| v.parse::<f64>().ok()) {
            game.set_health(health);
        }
        // `--set merchant=6` puts a merchant that many meters ahead, and `--set shop=1` trades with
        // the nearest, to see their stall
        if let Some(distance) = setting("merchant").and_then(|v| v.parse::<f64>().ok()) {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            game.summon_merchant(&world, &water_level, start, ahead, distance);
        }
        if flag("shop", false) {
            game.open_nearest_shop(start);
        }
        // `--set villager=3` puts a villager of each kind that many meters ahead, to see them close
        if let Some(distance) = setting("villager").and_then(|v| v.parse::<f64>().ok()) {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            game.summon_villagers(&world, &water_level, start, ahead, distance);
        }
        if capture.is_some() {
            println!("{}", game.describe_weapons());
        }
        let models_version = game.models_version();
        let animal_models = model_buffer(&memory_allocator, game.model_words());
        let weapon_models = model_buffer(&memory_allocator, game.weapon_model_words());
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
            memory.light_cache_entries * LIGHT_CACHE_WORDS,
        )
        .unwrap();
        let instance_allocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                // And each frame's View
                buffer_usage: BufferUsage::STORAGE_BUFFER | BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );
        // A preview of the loading screen shows it fade into the menu, as it does in play
        let menu_open = (capture.is_none() || loading::previewing()) && flag("menu", true);
        // `--set waterfall=2` starts looking at the second waterfall
        let visit_waterfall = setting("waterfall").and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0);
        // `--set sun_shafts=off`, `low` or `high`, and `sun_shafts_strength=1.5` for thicker air
        let shafts = shafts::Shafts::new(
            shafts_pipeline,
            &memory_allocator,
            &descriptor_set_allocator,
            // As the quality preset has them, which on cards that aren't their own is Low
            setting("sun_shafts").and_then(shafts::Quality::parse).unwrap_or(preset.sun_shafts),
            setting("sun_shafts_strength").and_then(|v| v.parse().ok()).unwrap_or(1.0),
        );

        let mut app = App {
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
            skips_pipeline,

            sun_map,
            sun_map_for: None,

            voxel_set,
            player,
            world,
            stream,
            world_builds: 0,
            rivers,
            last_camera: None,
            camera_drift: Vector3::zeros(),
            lights: Vec::new(),
            // No finer voxels round the camera, so they're the same size all over. Finer ones have
            // to be built again every time a spell or a blow leaves a mark near by.
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
            // The same moving or still, so stopping doesn't cost more rays and rebuild every image
            render_scale: setting("render_scale").and_then(|v| v.parse().ok()).unwrap_or(preset.render_scale),
            still_render_scale: setting("still_render_scale").and_then(|v| v.parse().ok()).unwrap_or(preset.render_scale),
            still_frames: 0,
            last_camera_view: None,

            // Bright, clear daylight from ahead and to one side of the starting view, with little
            // haze, so the voxels read crisply. Grass is made of voxels rather than triangles.
            sun_azimuth: setting("sun_azimuth").and_then(|v| v.parse().ok()).unwrap_or(150.0),
            sun_elevation: setting("sun_elevation").and_then(|v| v.parse().ok()).unwrap_or(38.0),
            shadows: flag("shadows", preset.shadows),
            ambient_occlusion: flag("ao", preset.ambient_occlusion),
            bounce_light: flag("bounce", preset.bounce_light),
            smooth_shading: flag("smooth", preset.smooth_shading),
            wind: flag("wind", preset.wind),
            sunbeams: flag("sunbeams", false),
            denoise: flag("denoise", preset.denoise),
            color_grade: flag("grade", true),
            bloom: setting("bloom").and_then(|v| v.parse().ok()).unwrap_or(preset.bloom),
            quality,
            view_distance: setting("view_distance").and_then(|v| v.parse().ok()).unwrap_or(preset.view_distance),
            reflections: flag("reflections", preset.reflections),
            fast_upscale: flag("fast_upscale", preset.fast_upscale),
            pending_quality: None,
            haze: setting("haze").and_then(|v| v.parse().ok()).unwrap_or(0.0012),
            shafts,
            grass: flag("grass", false),

            frame: 0,
            accum_count: 0,
            last_view: None,
            reset_accum: true,
            last_world_to_window: None,

            game,
            animals_on: flag("animals", true),
            animal_models,
            weapon_models,
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
            scripted_sprint: false,
            script_controls: combat::Controls::default(),
            kicked: 0.0,
            audio,
            craters: Vec::new(),
            edited: Vec::new(),
            edited_ground: None,
            batching: false,
            stains: Vec::new(),
            step_distance: 0.0,
            walk_speed: 0.0,
            walked: 0.0,
            waterfall_index: 0,

            menu_open,
            started: !menu_open,
            // `--set settings=1` starts with them shown, to see them in screenshots with `gui=1`
            show_settings: flag("settings", false),
            // `--set inventory=1` shows the inventory, to see how it looks
            inventory_open: flag("inventory", false),
            shopping: false,
            generate_requested: false,
            waterfall_requested: false,

            screenshot_requested: false,
            max_in_flight: setting("in_flight").and_then(|v| v.parse().ok()).unwrap_or(MAX_IN_FLIGHT),
            frame_ms: 0.0,
            scales_set: (0.0, 0.0),
            scales_set_at: Instant::now(),
            vram_checked: Instant::now(),
            scale_lowered_at: Instant::now(),
            memory,
            perf: setting("perf_log").and_then(|v| v.parse().ok()).filter(|_| capture.is_some()).map(perf::PerfLog::new),
            sky_check: setting("sky_check").and_then(|v| v.parse().ok()).filter(|_| capture.is_some()).unwrap_or(0),
            sky_probe: None,
            blank_shots: 0,
            play: perf::Play::new(setting("churn").and_then(|v| v.parse().ok()), setting("shot_every").and_then(|v| v.parse().ok())),
            capture,

            input,
            focused: false,
            last_second: Instant::now(),
            frames_since_last_second: 0,
            fps: 0,

            rcx: None,
        };
        let pieces = app.stream.as_ref().map_or(Vec::new(), |s| s.pieces.clone());
        app.adopt_pieces(&pieces);
        // `--set camp_clear=1` kills every bandit of the camp nearest the start, its warlord too,
        // so its hoard turns up
        if flag("camp_clear", false) {
            let fell = app.game.clear_nearest_camp(&app.world, start);
            println!("Cleared the camp: {fell} of its band fell. {}", app.game.last_message().unwrap_or(""));
        }
        // The merchants on stalls and the princess in her throne room are about by now, and may be
        // nearer than the one wandering about
        if setting("shop").is_some_and(|v| v == "true" || v == "1") {
            app.game.open_nearest_shop(start);
        }
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
            && let Some(stream) = &app.stream
        {
            let (kind, inside) = what.strip_suffix("-inside").map_or((what, false), |k| (k, true));
            let vpm = app.world.voxels_per_meter as f64;
            let origin = Vector3::new(app.world.origin[0] as f64, app.world.origin[1] as f64, 0.0);
            let start = camera_to_voxels(app.camera.position, app.world.size);
            let to_voxels = |p: [f32; 3]| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64) * vpm - origin;
            let site = stream
                .pieces
                .iter()
                .flat_map(|p| p.sites.iter())
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
        // For checking how trees look: `--set view_tree=3` looks at the third tree nearest the start,
        // from `--set view_distance=12` meters away or far enough to see all of it
        if let Some(n) = setting("view_tree").and_then(|v| v.parse::<usize>().ok()) {
            let start = camera_to_voxels(app.camera.position, app.world.size);
            if let Some(&(foot, height, reach)) = app.trees_near(start, 30.0).get(n.saturating_sub(1)) {
                let vpm = app.world.voxels_per_meter as f64;
                let distance = setting("view_distance").and_then(|v| v.parse::<f64>().ok()).unwrap_or((height * 1.1).max(reach * 2.0) + 3.0);
                println!("looking at a tree {:.1} m high, {:.0} m away", height, (foot - start).xy().norm() / vpm);
                app.view_tree(foot, height, reach, distance);
            }
        }
        // `--set tilt=30` tips the view that many degrees further up than wherever it starts
        if let Some(tilt) = setting("tilt").and_then(|v| v.parse::<f64>().ok()) {
            app.camera.rotation.x = (app.camera.rotation.x + tilt.to_radians()).clamp(-FRAC_PI_2, FRAC_PI_2);
        }
        // `--set sun_ahead=20` turns the sun to that many degrees round from straight ahead of the
        // view, keeping its elevation, to see light coming towards the camera, as sun shafts do
        if let Some(offset) = setting("sun_ahead").and_then(|v| v.parse::<f64>().ok()) {
            let ahead = (app.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            app.sun_azimuth = (ahead.y.atan2(ahead.x).to_degrees() + offset).rem_euclid(360.0) as f32;
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

    /// In screenshot runs, says when `what`, started at `start`, held a frame up
    fn slow(&self, what: &str, start: Instant) {
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        if let Some(capture) = &self.capture
            && ms > 15.0
        {
            println!("Slow at frame {}: {what} took {ms:.0} ms", capture.frame_times.len() + 1);
        }
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
        if let Some(rcx) = &self.rcx {
            grab_cursor(&rcx.window, focused);
        }
    }

    /// Takes the lights and props of what the world's made of, `landscape`, in its voxels: every
    /// time it moves on, so what's come in turns up, see stream.rs. What's there already stays.
    fn adopt_features(&mut self, landscape: &worldgen::PieceFeatures) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let to_voxels = |p: [f32; 3]| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64) * vpm - origin;
        self.lights = landscape
            .lights
            .iter()
            .map(|l| {
                let p = to_voxels(l.position);
                [p.x as f32, p.y as f32, p.z as f32, (l.reach as f64 * vpm) as f32, l.color[0], l.color[1], l.color[2], 0.0]
            })
            .collect();
        // Bosses guard some of the ruins: golems the castles and watchtowers, drakes the desert
        // ruins and pyramids, Ironhide the shrines, colonnades and ravines. Each waits a little way
        // out in front, where there's room to fight.
        let camps = &landscape.camps;
        let camp_at = |site: &worldgen::Site| camps.iter().find(|c| c.fire == site.position);
        let lairs: Vec<game::Lair> = landscape
            .sites
            .iter()
            .filter_map(|site| {
                let key = site.position.map(|v| (v * 10.0).round() as i64);
                let pick = (key[0].wrapping_mul(73_856_093) ^ key[1].wrapping_mul(19_349_663)).rem_euclid(100);
                // Every dungeon's deepest hall has its Grave Knight, and every great cavern of a
                // cave its boss; only some ruins have one, and some bandits' camps a warlord, more
                // of the forts than the others, see worldgen\camps.rs
                if pick >= BOSS_CHANCE && !matches!(site.kind, "dungeon" | "cavern" | "bandit camp" | "fallen throne" | "undercity heart") {
                    return None;
                }
                let look = match site.kind {
                    // Raid bosses: in a third of the dungeons' deepest halls, and the odd castle and
                    // pyramid
                    "dungeon" if pick % 3 == 0 => animals::Look::CryptTitan,
                    // The dead lord of a fallen city in its temple: a titan in some, a knight in the
                    // rest, see worldgen\undercity.rs
                    "undercity heart" if pick % 4 == 0 => animals::Look::CryptTitan,
                    "undercity heart" => animals::Look::GraveKnight,
                    "castle" if pick % 5 == 0 => animals::Look::Colossus,
                    "pyramid" if pick % 5 == 0 => animals::Look::ElderWyrm,
                    // Each turns into a kind of its own for the land round it, see Look::for_ground
                    // A castle fallen to the dead has one on its throne
                    "dungeon" | "fallen throne" => animals::Look::GraveKnight,
                    "castle" if pick % 3 == 1 => animals::Look::Ironhide,
                    "castle" if pick % 3 == 2 => animals::Look::GraveKnight,
                    "castle" | "desert ruins" => animals::Look::Golem,
                    "watchtower" | "colonnade" if pick % 2 == 1 => animals::Look::Direwolf,
                    "watchtower" | "pyramid" | "flooded church" => animals::Look::Drake,
                    "colonnade" => animals::Look::Ironhide,
                    // A crevice is shut in under the ground, with no way down to it, so nothing
                    // guards it
                    "ravine" => animals::Look::Spider,
                    // In caves spiders, cave bears, and golems of the stone itself
                    "cavern" if pick % 3 == 2 => animals::Look::Golem,
                    "cavern" if pick % 3 == 1 => animals::Look::Ironhide,
                    "cavern" => animals::Look::Spider,
                    "shrine" => animals::Look::Treant,
                    "bandit camp" if camp_at(site).is_some_and(|c| c.warlord) => animals::Look::Warlord,
                    _ => return None,
                };
                let [fx, fy] = site.facing;
                let length = (fx * fx + fy * fy).sqrt().max(1e-3);
                let out = if matches!(site.kind, "ravine" | "dungeon" | "cavern" | "bandit camp" | "fallen throne" | "undercity heart") { 0.0 } else { 16.0 };
                let at = [site.position[0] + fx / length * out, site.position[1] + fy / length * out, site.position[2]];
                Some(game::Lair { look, position: to_voxels(at), key })
            })
            .collect();
        // Monsters haunt the other halls of dungeons, cave spiders nest in caverns or gloomwings
        // roost there, bandits keep camps, as many as each camp's band, their hoard turning up
        // once they're all dead, gloomwings roost along open ravines, and burrowers dig in out in
        // front of desert ruins and pyramids
        let dens: Vec<game::Den> = landscape
            .sites
            .iter()
            .filter_map(|site| {
                let (kind, out, band) = match site.kind {
                    "dungeon hall" => (game::DenKind::Hall, 0.0, None),
                    "cave nest" => (game::DenKind::Nest, 0.0, None),
                    "bandit camp" => (
                        game::DenKind::Camp,
                        0.0,
                        camp_at(site).map(|c| game::Band {
                            keepers: std::iter::repeat_n(animals::Look::Bandit, c.bandits as usize).chain(std::iter::repeat_n(animals::Look::BanditBrute, c.brutes as usize)).collect(),
                            within: c.spread as f64,
                            hoard: game::Chest { position: to_voxels(c.chest), heading: c.heading as f64, key: c.chest.map(|v| (v * 10.0).round() as i64), luck: c.luck as f64, hoard: true },
                            hint: c.hint,
                        }),
                    ),
                    "ravine" => (game::DenKind::Roost, 0.0, None),
                    "desert ruins" | "pyramid" => (game::DenKind::Burrows, 12.0, None),
                    "fallen castle" | "fallen court" | "fallen hall" | "fallen throne" => (game::DenKind::Fallen, 0.0, None),
                    // The dead walking the streets, the square, the canal and the temple of a city
                    // fallen under the land
                    "undercity gate" | "undercity street" | "undercity canal" | "undercity plaza" | "undercity hall" => (game::DenKind::Sunken, 0.0, None),
                    _ => return None,
                };
                let [fx, fy] = site.facing;
                let length = (fx * fx + fy * fy).sqrt().max(1e-3);
                let at = [site.position[0] + fx / length * out, site.position[1] + fy / length * out, site.position[2]];
                Some(game::Den { position: to_voxels(at), key: site.position.map(|v| (v * 10.0).round() as i64), kind, band })            })
            .collect();
        // Merchants keep stalls out in front of some of the ruins no boss guards, and behind the
        // counters of the stalls on the towns' squares
        let stalls: Vec<game::Stall> = landscape
            .sites
            .iter()
            .filter_map(|site| {
                let key = site.position.map(|v| (v * 10.0).round() as i64);
                if site.kind == "town stall" {
                    return Some(game::Stall { position: to_voxels(site.position), key: [key[0], key[1], key[2] + 7], within: 0.3, royal: None });
                }
                // The princess keeps the royal armoury in a castle's throne room
                if site.kind.starts_with("royal") {
                    let heading = (site.facing[1] as f64).atan2(site.facing[0] as f64);
                    return (site.kind == "royal stall").then(|| game::Stall { position: to_voxels(site.position), key: [key[0], key[1], key[2] + 7], within: 0.3, royal: Some(heading) });
                }
                let pick = (key[0].wrapping_mul(73_856_093) ^ key[1].wrapping_mul(19_349_663)).rem_euclid(100);
                if pick < BOSS_CHANCE || pick % 3 != 0 || site.kind.starts_with("fallen") || site.kind.starts_with("undercity") || matches!(site.kind, "dungeon" | "dungeon hall" | "ravine" | "crevice" | "cavern" | "cave nest" | "bandit camp" | "town" | "cave town") {
                    return None;
                }
                let [fx, fy] = site.facing;
                let length = (fx * fx + fy * fy).sqrt().max(1e-3);
                let at = [site.position[0] + fx / length * 11.0, site.position[1] + fy / length * 11.0, site.position[2]];
                Some(game::Stall { position: to_voxels(at), key: [key[0], key[1], key[2] + 7], within: 4.0, royal: None })
            })
            .collect();
        // Folk live round the towns' squares, those down in caverns in fewer of them, and some
        // stroll round the courtyards of royal castles
        let towns: Vec<game::Town> = landscape
            .sites
            .iter()
            .filter(|site| matches!(site.kind, "town" | "cave town" | "royal court"))
            .map(|site| {
                let (folk, roam) = match site.kind {
                    "town" => (7, 18.0),
                    "royal court" => (4, 9.0),
                    _ => (5, 12.0),
                };
                game::Town { position: to_voxels(site.position), folk, roam }
            })
            .collect();
        // Royal guards at the gates of castles and by the princess's throne
        let guards: Vec<(Vector3<f64>, f64)> = landscape
            .sites
            .iter()
            .filter(|site| site.kind == "royal guard")
            .map(|site| (to_voxels(site.position), (site.facing[1] as f64).atan2(site.facing[0] as f64)))
            .collect();
        let water = &self.water;
        self.game.set_lairs(&self.world, &|x, y| water.surface_at(x, y), lairs);
        self.game.set_dens(&self.world, &|x, y| water.surface_at(x, y), dens);
        self.game.set_stalls(&self.world, &|x, y| water.surface_at(x, y), stalls);
        self.game.set_towns(&self.world, &|x, y| water.surface_at(x, y), towns);
        self.game.set_guards(&self.world, &|x, y| water.surface_at(x, y), guards);

        let (chests, props): (Vec<&worldgen::Prop>, Vec<&worldgen::Prop>) = landscape.props.iter().partition(|p| p.kind == worldgen::PropKind::Chest);
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
                    hoard: false,
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
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        self.craters.push((((center + origin) / vpm).into(), radius));
        self.carve(center, radius, true);
    }

    /// Stains the ground `radius` meters round `center`, in voxels of the world, with `material`,
    /// burning off what grows there with `burn`. Remembered like the holes.
    fn stain(&mut self, center: Vector3<f64>, radius: f64, material: u8, burn: bool) {
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        self.stains.push((((center + origin) / vpm).into(), radius, material, burn));
        self.paint(center, radius, material, burn);
    }

    /// Paints the world and the GPU's copy of it, see World::paint()
    fn paint(&mut self, center: Vector3<f64>, radius: f64, material: u8, burn: bool) {
        let vpm = self.world.voxels_per_meter as f64;
        let changed = self.world.paint(center.into(), radius * vpm, material, burn);
        self.edited.extend(changed);
        if !self.batching {
            self.flush_edits();
        }
    }

    /// Takes the voxels `radius` meters round `center` out of the world, and out of the GPU's copy
    /// of it, letting water into the hole. With `debris` bits of them fly out.
    fn carve(&mut self, center: Vector3<f64>, radius: f64, debris: bool) {
        let vpm = self.world.voxels_per_meter as f64;
        let start = Instant::now();
        let (changed, blown) = self.world.carve(center.into(), radius * vpm);
        self.slow("blowing a hole in the world", start);
        if changed.is_empty() {
            return;
        }
        if debris {
            let start = Instant::now();
            self.game.debris(center, &blown, vpm);
            self.slow("throwing out what was blown loose", start);
        }
        self.edited.extend(changed);
        let reach = (radius * vpm).ceil() as i64 + 2;
        let (x, y) = (center.x as i64, center.y as i64);
        let (lo, hi) = ([x - reach, y - reach], [x + reach + 1, y + reach + 1]);
        self.edited_ground = Some(match self.edited_ground {
            Some((a, b)) => ([a[0].min(lo[0]), a[1].min(lo[1])], [b[0].max(hi[0]), b[1].max(hi[1])]),
            None => (lo, hi),
        });
        if !self.batching {
            self.flush_edits();
        }
    }

    /// Sends what's been carved and painted since last time to the GPU, all at once: each time
    /// waits for the frame being drawn to finish, so a burst of holes and stains costs one wait
    /// rather than one each
    fn flush_edits(&mut self) {
        if self.edited.is_empty() && self.edited_ground.is_none() {
            return;
        }
        let start = Instant::now();
        let needed = self.world.brick_room();
        let grew = needed > self.voxel_set.capacity;
        let mut cells = std::mem::take(&mut self.edited);
        cells.sort_unstable();
        cells.dedup();
        // The frames still being drawn read what's about to change, so let them finish
        if let Some(rcx) = &mut self.rcx {
            wait_for_frames(rcx, &self.device);
        }
        // Bricks still to be copied over into more room for them go first, see grow_with_frame()
        self.voxel_set.settle(&self.command_buffer_allocator, &self.queue);
        if needed > self.voxel_set.capacity {
            // Out of room for more bricks: a bigger buffer, the ones there are copied over on the GPU
            self.voxel_set.grow(needed, &self.memory_allocator, &self.command_buffer_allocator, &self.descriptor_set_allocator, &self.render_pipeline, &self.queue);
        }
        if !cells.is_empty() {
            self.upload_bricks(&cells);
        }
        if let Some((lo, hi)) = self.edited_ground.take() {
            self.water.sync_ground(&self.world, lo, hi, &self.memory_allocator, &self.command_buffer_allocator, &self.queue);
        }
        // Screenshot runs say when sending what changed held a frame up
        if self.capture.is_some() && (grew || start.elapsed().as_secs_f64() > 0.004) {
            println!("Edits sent in {:.1} ms: {} bricks{}", start.elapsed().as_secs_f64() * 1000.0, cells.len(), if grew { ", after making more room" } else { "" });
        }
    }

    /// Sends the brick grid `cells` of the main world to the GPU, along with their bricks
    fn upload_bricks(&mut self, cells: &[usize]) {
        let grid = self.voxel_set.grid.clone();
        let mut data: Vec<u32> = cells.iter().map(|&c| self.world.brick_grid[c]).collect();
        let grid_regions: Vec<BufferImageCopy> = cells
            .iter()
            .enumerate()
            .map(|(k, &c)| BufferImageCopy {
                buffer_offset: (k * 4) as u64,
                image_subresource: grid.subresource_layers(),
                // Where it's kept in the room there is, see World::cell()
                image_offset: self.world.cell_at(c),
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
            speed: self.walk_speed,
            stride: self.walked,
        }
    }

    /// Once the GPU's done with the last look at how much of the view is sky, see `sky_check`, counts
    /// it, and says so when the world's missing from it while the camera looks down at where it
    /// should be, saving a shot of the next frame
    fn check_sky(&mut self) {
        let Some((probe, pitch)) = &self.sky_probe else {
            return;
        };
        let pitch = *pitch;
        let sky = match probe.read() {
            Ok(depths) => depths.iter().filter(|&&d| d < 0.0).count() as f32 / depths.len().max(1) as f32,
            // Still being drawn
            Err(_) => return,
        };
        self.sky_probe = None;
        if let Some(perf) = &mut self.perf {
            perf.sky(sky);
        }
        // Looking down a little, at least a third of the view is the ground: nearly all sky is a
        // world gone missing, even with something held up in front of the camera. Not while the
        // screen's dark round the world being built afresh, see stream.rs, as nothing's seen.
        let dark = self.stream.as_ref().and_then(|s| s.teleport.as_ref()).is_some_and(|t| t.dark > 0.5);
        if sky < 0.8 || pitch > -0.15 || dark {
            return;
        }
        let vpm = self.world.voxels_per_meter as f64;
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let [sx, sy, sz] = self.world.size.map(f64::from);
        let edge = eye.x.min(eye.y).min(sx - eye.x).min(sy - eye.y) / vpm;
        let Some(capture) = &mut self.capture else {
            return;
        };
        let frame = capture.frame_times.len() + 1;
        println!(
            "BLANK at frame {frame}, {}: {:.0}% sky looking {:.0} degrees down, eye at {:.0}, {:.0}, {:.0} m in a world {:.0} x {:.0} x {:.0} m, edge {edge:.0} m off, a strip coming in: {}",
            perf::clock(capture.started.elapsed().as_secs_f64()),
            sky * 100.0,
            -pitch.to_degrees(),
            eye.x / vpm,
            eye.y / vpm,
            eye.z / vpm,
            sx / vpm,
            sy / vpm,
            sz / vpm,
            self.stream.as_ref().is_some_and(|s| s.busy()),
        );
        if let Some(perf) = &mut self.perf {
            perf.blank += 1;
        }
        if self.blank_shots < 30 {
            self.blank_shots += 1;
            capture.shots.push(frame + 1);
        }
    }

    /// How much of the GPU's memory the world takes: its bricks, the map of them, the grass and the
    /// water. There's only ever the one, which moves on a strip at a time, see stream.rs.
    fn piece_bytes(&self) -> u64 {
        let water = if self.water.active { self.water.bytes() } else { 0 };
        self.voxel_set.bytes() + water
    }

    /// How many bytes of the GPU's memory everything sized to the render resolution may take,
    /// `in_use` of which it takes now: what Windows gives the game before it starts moving some
    /// of it out to the computer's memory, which makes every frame slow, less all else the game
    /// has there and a margin, see VRAM_MARGIN. None where the driver can't say.
    fn render_room(&self, in_use: u64) -> Option<u64> {
        let vram = perf::vram(&self.device)?;
        // At least what's known to be there: what Windows says is used lags behind at the start
        let held = self.piece_bytes() + self.light_cache.size();
        let others = vram.used.saturating_sub(in_use).max(held);
        let margin = ((vram.budget as f64 * VRAM_MARGIN) as u64).max(VRAM_MARGIN_MIN);
        Some(vram.budget.saturating_sub(others + margin))
    }

    /// Makes everything sized to the render resolution again when the window or the render scale
    /// has changed, or when the GPU's memory has less room for it, or more again, see VRAM_CHECK:
    /// at the render scale asked for, or as near it as there's room for, see render_room(). At
    /// 3.0 there was barely room at all, and once something else took a little, Windows moved
    /// some of the game out to the computer's memory, which made every frame slower.
    fn update_render_images(&mut self, window_extent: [u32; 2]) {
        let wanted = if self.still_frames >= 2 { self.still_render_scale } else { self.render_scale };
        let scales = (self.render_scale, self.still_render_scale);
        if scales != self.scales_set {
            self.scales_set = scales;
            self.scales_set_at = Instant::now();
        }
        let settled = self.scales_set_at.elapsed() > SCALE_SETTLE;
        let rcx = self.rcx.as_ref().unwrap();
        let changed = rcx.recreate_images || (settled && wanted != rcx.render_scale_wanted);
        let checking = self.vram_checked.elapsed() > VRAM_CHECK;
        if !changed && !checking {
            return;
        }
        if checking {
            self.vram_checked = Instant::now();
        }
        let in_use = rcx.images.bytes() + rcx.other_images.as_ref().map_or(0, |(_, images)| images.bytes());
        let (scale_in_use, scale_wanted, kept_other) = (rcx.render_scale_in_use, rcx.render_scale_wanted, rcx.other_images.is_some());

        let room = self.render_room(in_use);
        let scale = room.map_or(wanted, |room| fitting_scale(wanted, window_extent, room));
        // Less room than what's in use takes, as when something else has taken some, or at the
        // start, before Windows counts all the game has, comes down; or up again once there's
        // room for more of what's wanted, a while after. Not while the world's moved on all at
        // once, which takes more for a moment.
        let settling = self.stream.as_ref().is_some_and(|s| s.teleport.is_some());
        let down = !settling && scale < scale_in_use * 0.98;
        let up = !settling
            && self.scale_lowered_at.elapsed() > RAISE_AFTER
            && scale_in_use < scale_wanted.min(wanted) * 0.98
            && scale >= (scale_in_use * 1.15).min(wanted);
        if down && kept_other && !changed {
            // Those kept for the other render scale go first
            self.rcx.as_mut().unwrap().other_images = None;
        } else if changed || down || up {
            if down {
                self.scale_lowered_at = Instant::now();
            }
            self.make_render_images(window_extent, wanted, scale, room);
        }
    }

    /// Makes everything sized to the render resolution again, at render scale `scale` for the
    /// `wanted` one, with `room` bytes of the GPU's memory for them, see update_render_images().
    /// Those made for the other of the render scales while moving and when still are used again if
    /// they're the size, and the ones in use kept for it if there's room for both. If there isn't
    /// room for the old ones and the new at once, the old are let go of first, once the frames
    /// drawn with them are done.
    fn make_render_images(&mut self, window_extent: [u32; 2], wanted: f32, scale: f32, room: Option<u64>) {
        let extent = render_extent(window_extent, scale);
        let device = self.device.clone();
        let other_scale = if wanted == self.render_scale { self.still_render_scale } else { self.render_scale };
        let rcx = self.rcx.as_mut().unwrap();
        let remade = rcx.recreate_images;
        rcx.recreate_images = false;
        let old_wanted = rcx.render_scale_wanted;
        rcx.render_scale_wanted = wanted;
        rcx.render_scale_in_use = scale;
        if !remade && rcx.images.fits(extent, window_extent) {
            return;
        }
        if let Some(capture) = &self.capture {
            println!(
                "Render images made again at frame {}: {}x{} at scale {scale:.2}{}",
                capture.frame_times.len() + 1,
                extent[0],
                extent[1],
                if scale < wanted { format!(", {wanted:.2} wanted but the GPU's memory has room for only {:.0} MB", room.unwrap_or(0) as f64 / perf::MB) } else { String::new() }
            );
        }
        let new_bytes = perf::render_bytes(extent, window_extent);
        let room = room.unwrap_or(u64::MAX);
        let kept = rcx.images.histories.clone();
        let images = match rcx.other_images.take_if(|(_, images)| !remade && images.fits(extent, window_extent)) {
            Some((_, images)) => images,
            None => {
                // Only one other lot is kept
                let other = rcx.other_images.take();
                if new_bytes + rcx.images.bytes() + other.as_ref().map_or(0, |(_, images)| images.bytes()) > room {
                    // No room for both at once: the old ones are let go of first, once the frames
                    // drawn with them are done, with tiny ones in their place meanwhile, and the
                    // sun shafts' with them, which hold on to one of them
                    wait_for_frames(rcx, &device);
                    drop(other);
                    self.shafts.let_go_of_targets();
                    let tiny = get_render_images(
                        self.memory_allocator.clone(),
                        self.descriptor_set_allocator.clone(),
                        &self.render_pipeline,
                        &self.post_pipeline,
                        &self.resample_pipeline,
                        &self.upscale_pipeline,
                        &self.sun_map,
                        [1, 1],
                        [1, 1],
                        None,
                    );
                    drop(std::mem::replace(&mut rcx.images, tiny));
                }
                get_render_images(
                    self.memory_allocator.clone(),
                    self.descriptor_set_allocator.clone(),
                    &self.render_pipeline,
                    &self.post_pipeline,
                    &self.resample_pipeline,
                    &self.upscale_pipeline,
                    &self.sun_map,
                    extent,
                    window_extent,
                    Some(kept.clone()),
                )
            }
        };
        let old = std::mem::replace(&mut rcx.images, images);
        // Kept for the other render scale, while moving or when still, if that's what they were
        // made for and there's room for both
        if !remade && old_wanted == other_scale && other_scale != wanted && old.bytes() + new_bytes <= room && old.resample_image.extent()[..2] == window_extent[..] {
            rcx.other_images = Some((old_wanted, old));
        }
        // New ones have nothing in them yet
        if !Arc::ptr_eq(&rcx.images.histories[0], &kept[0]) {
            self.last_world_to_window = None;
        }
        self.reset_accum = true;
        if let Some(perf) = &mut self.perf {
            perf.recreated += 1;
        }
    }

    /// Says how the frames went since the last time, how much of the GPU's memory is used, and
    /// what's piled up, see perf.rs
    fn report_perf(&mut self, render_extent: [u32; 3], window_extent: [u32; 3]) {
        let vram = perf::vram(&self.device);
        let bytes = |b: u64| b as f64 / perf::MB;
        let world = self.voxel_set.bytes();
        let water = if self.water.active { self.water.bytes() } else { 0 };
        let render = perf::render_bytes([render_extent[0], render_extent[1]], [window_extent[0], window_extent[1]]);
        // What's kept on the CPU: the world's bricks, their map, grass and water maps, and the
        // water's own copies
        let cpu_world = {
            let w = &self.world;
            w.brick_bytes() as u64
                + (w.brick_grid.capacity() + w.grass.as_ref().map_or(0, |g| g.capacity())) as u64 * 4
                + w.water.as_ref().map_or(0, |m| (m.ground.capacity() + m.surface.capacity()) as u64 * 4)
        };
        // How the strips have been coming in since last time, see stream.rs
        let strips = match &mut self.stream {
            Some(stream) => {
                let s = &mut stream.stats;
                let most = |v: &[f32]| v.iter().copied().fold(0.0f32, f32::max);
                let mean = |v: &[f32]| if v.is_empty() { 0.0 } else { v.iter().sum::<f32>() / v.len() as f32 };
                let line = format!(
                    " strips={} reused={} discarded={} teleports={} strip_build_ms={:.0}/{:.0} strip_send_ms={:.0}/{:.0} strip_total_ms={:.0}/{:.0} strip_frame_ms={:.1} bricks={} pages={}",
                    s.strips,
                    s.reused,
                    s.discarded,
                    s.teleports,
                    mean(&s.built_ms),
                    most(&s.built_ms),
                    mean(&s.sent_ms),
                    most(&s.sent_ms),
                    mean(&s.total_ms),
                    most(&s.total_ms),
                    s.frame_ms_max,
                    self.world.brick_count(),
                    self.world.page_pool().map_or(0, |p| p.lock().unwrap_or_else(|e| e.into_inner()).count()),
                );
                *s = stream::StreamStats::default();
                line
            }
            None => String::new(),
        };
        let (heap, heap_peak) = perf::heap();
        let more = format!(
            "rt={}x{} win={}x{} scale={:.2}/{:.2} vram={:.0}/{:.0} render_mb={:.0} world_mb={:.0} water_mb={:.0} world_m={} models_mb={:.0} light_mb={:.0} heap_mb={:.0} heap_peak_mb={:.0} cpu_world_mb={:.0} cpu_water_mb={:.0} allocs={} craters={} stains={} lights={}{strips}",
            render_extent[0],
            render_extent[1],
            window_extent[0],
            window_extent[1],
            self.render_scale,
            self.still_render_scale,
            vram.map_or(-1.0, |v| bytes(v.used)),
            vram.map_or(-1.0, |v| bytes(v.budget)),
            bytes(render),
            bytes(world),
            bytes(water),
            self.world.size[0] / self.world.voxels_per_meter.max(1),
            bytes(self.animal_models.size() + self.weapon_models.size()),
            bytes(self.light_cache.size()),
            heap as f64 / perf::MB,
            heap_peak as f64 / perf::MB,
            bytes(cpu_world),
            bytes(self.water.cpu_bytes()),
            self.device.allocation_count(),
            self.craters.len(),
            self.stains.len(),
            self.lights.len(),
        );
        let Some(perf) = &mut self.perf else {
            return;
        };
        println!("{}", perf.report(&more));
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        println!("PERFGAME {}", self.game.report(eye));
    }

    /// Builds the world picked in the settings, in place of the current one
    fn generate_world(&mut self) {
        // A new landscape has rivers of its own
        let rivers = Arc::new(RiverCache::default());
        let (world, landscape) = build_world(self.scene, &self.terrain, self.future_voxel_resolution, [0, 0], &rivers);
        self.craters.clear();
        self.stains.clear();

        let max_bytes = limits::get().buffer_bytes() as usize;
        if world.brick_room() * worldgen::BRICK_WORDS * 4 > max_bytes {
            self.world_error = Some(format!(
                "That world needs {} MB of bricks but your GPU allows {} MB. Try fewer voxels per metre or a smaller world.",
                world.brick_room() * worldgen::BRICK_WORDS * 4 / (1024 * 1024),
                max_bytes / (1024 * 1024)
            ));
            return;
        }
        // A landscape moves on a strip at a time from where it's built
        let (world, stream) = match &landscape {
            Some(landscape) => {
                let (world, pieces, river_springs) = into_window(world, landscape);
                let stream = stream::Stream::new(stream::STRIP_M * world.voxels_per_meter, pieces, river_springs, self.memory_allocator.clone());
                (world, Some(stream))
            }
            None => (world, None),
        };
        drop(landscape);
        // What was there before isn't drawn with any more
        if let Some(rcx) = &mut self.rcx {
            wait_for_frames(rcx, &self.device);
        }
        self.voxel_set = get_voxel_set(
            self.memory_allocator.clone(),
            self.command_buffer_allocator.clone(),
            self.descriptor_set_allocator.clone(),
            &self.render_pipeline,
            &self.queue,
            1,
            &world,
            self.memory.bricks,
        );
        let springs = stream.as_ref().map_or(Vec::new(), |s| s.springs());
        self.water = Water::new(&self.memory_allocator, &self.command_buffer_allocator, &self.descriptor_set_allocator, &self.queue, &self.water_pipeline, &world, &springs);
        let mut world = world;
        world.forget_water_surface();
        self.world = world;
        self.stream = stream;
        self.world_builds += 1;
        self.rivers = rivers;
        self.light_cache_for = None;
        self.near = None;
        self.near_build = None;
        self.world_error = None;
        self.sun_map_for = None;
        let pieces = self.stream.as_ref().map_or(Vec::new(), |s| s.pieces.clone());
        self.adopt_pieces(&pieces);

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
        let Some(stream) = &self.stream else {
            return;
        };
        // Those in the world where it is now
        let vpm = self.world.voxels_per_meter as f32;
        let (lo, hi) = ([self.world.origin[0] as f32 / vpm, self.world.origin[1] as f32 / vpm], [(self.world.origin[0] + self.world.size[0] as i64) as f32 / vpm, (self.world.origin[1] + self.world.size[1] as i64) as f32 / vpm]);
        let mut falls: Vec<([f32; 3], [f32; 2], f32)> = stream.pieces.iter().flat_map(|p| p.waterfalls.iter().copied()).filter(|(lip, ..)| lip[0] >= lo[0] && lip[1] >= lo[1] && lip[0] < hi[0] && lip[1] < hi[1]).collect();
        falls.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]).then(a.0[1].total_cmp(&b.0[1])));
        if falls.is_empty() {
            return;
        }
        let (top, flow, _) = falls[self.waterfall_index % falls.len()];
        self.waterfall_index += 1;
        let vpm = self.world.voxels_per_meter as f64;
        let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
        let target = Vector3::new(top[0] as f64, top[1] as f64, top[2] as f64 - 1.5) * vpm - origin;
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

    /// The trees within `radius` meters of `from`, nearest first: the foot of each one's trunk in
    /// voxels, and how high and how far out from it it reaches in meters
    fn trees_near(&self, from: Vector3<f64>, radius: f64) -> Vec<(Vector3<f64>, f64, f64)> {
        use worldgen::material::*;
        let vpm = self.world.voxels_per_meter as f64;
        let step = (vpm / 4.0).max(1.0) as i64;
        let reach = (radius * vpm) as i64;
        let trunk = |m: u8| matches!(m, WOOD | BARK_DARK | BIRCH_BARK | PALM_WOOD | DEAD_WOOD);
        let ground = |m: u8| !worldgen::is_passable(m) && !trunk(m);
        let mut found: Vec<(Vector3<f64>, f64, f64)> = Vec::new();
        let (cx, cy) = (from.x as i64, from.y as i64);
        for y in (cy - reach..=cy + reach).step_by(step as usize) {
            for x in (cx - reach..=cx + reach).step_by(step as usize) {
                let Some(floor) = (0..self.world.size[2] as i64).rev().find(|&z| ground(self.world.material_at(x, y, z))) else {
                    continue;
                };
                if !trunk(self.world.material_at(x, y, floor + vpm as i64)) || !trunk(self.world.material_at(x, y, floor + 2 * vpm as i64)) {
                    continue;
                }
                let foot = Vector3::new(x as f64, y as f64, floor as f64);
                if found.iter().any(|(f, ..)| (f - foot).xy().norm() < 2.5 * vpm) {
                    continue;
                }
                // How high and wide it is, from the leaves and wood round it
                let (mut top, mut wide) = (floor, 0.0f64);
                for dy in (-8 * vpm as i64..=8 * vpm as i64).step_by(4) {
                    for dx in (-8 * vpm as i64..=8 * vpm as i64).step_by(4) {
                        let t = (floor + 2..floor + 30 * vpm as i64).rev().find(|&z| self.world.material_at(x + dx, y + dy, z) != AIR).unwrap_or(floor);
                        let m = self.world.material_at(x + dx, y + dy, t);
                        if t > floor + 2 * vpm as i64 && (worldgen::sways(m) || trunk(m)) {
                            top = top.max(t);
                            wide = wide.max(((dx * dx + dy * dy) as f64).sqrt() / vpm);
                        }
                    }
                }
                found.push((foot, (top - floor) as f64 / vpm, wide));
            }
        }
        found.sort_by(|a, b| (a.0 - from).xy().norm().total_cmp(&(b.0 - from).xy().norm()));
        found
    }

    /// Looks at the whole of a tree standing at `foot`, `height` meters high and `reach` meters
    /// across, from about `distance` meters away, the side with least in the way, the sun behind
    fn view_tree(&mut self, foot: Vector3<f64>, height: f64, reach: f64, distance: f64) {
        let vpm = self.world.voxels_per_meter as f64;
        let target = foot + Vector3::new(0.0, 0.0, height * 0.5 * vpm);
        let sun = (self.sun_azimuth as f64).to_radians();
        let mut best = (usize::MAX, target);
        for turn in 0..12 {
            // Starting from the side the sun's on, so it's lit
            let angle = sun + ((turn + 1) / 2) as f64 / 12.0 * TAU * if turn % 2 == 0 { 1.0 } else { -1.0 };
            let eye = target + Vector3::new(angle.cos() * distance, angle.sin() * distance, distance * 0.15) * vpm;
            let steps = (distance * 4.0) as usize;
            let blocked = (0..steps)
                .filter(|&i| {
                    let p = eye + (target - eye) * (i as f64 / steps as f64);
                    (p - target).xy().norm() > (reach + 0.5) * vpm && self.world.material_at(p.x as i64, p.y as i64, p.z as i64) != worldgen::material::AIR
                })
                .count();
            if blocked < best.0 {
                best = (blocked, eye);
            }
        }
        self.look_from(best.1, target);
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
            if self.game.shopping() {
                self.game.close_shop();
                if !self.inventory_open {
                    self.set_focus(true);
                }
            } else if self.inventory_open {
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
        if playing && !self.game.shopping() && (self.input.key_pressed(KeyCode::KeyI) || self.input.key_pressed(KeyCode::Tab)) {
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
        if playing && !self.focused && !self.inventory_open && !self.game.shopping() && self.input.mouse_pressed(MouseButton::Left) {
            self.set_focus(true);
        } else if playing && self.focused && !self.game.dead() {
            let bucket = self.game.holding_bucket() && !self.game.has_target();
            let armed = self.game.holding_weapon();
            if armed {
                controls = combat::Controls {
                    attack: self.input.mouse_pressed(MouseButton::Left),
                    holding: self.input.mouse_held(MouseButton::Left),
                    heavy: self.input.mouse_pressed(MouseButton::Right),
                    aiming: self.input.mouse_held(MouseButton::Right),
                    special: self.input.key_pressed(KeyCode::KeyR) || self.input.mouse_pressed(MouseButton::Middle),
                };
            } else if self.input.mouse_pressed(MouseButton::Left) {
                if bucket {
                    self.scoop_water(eye, look);
                } else {
                    self.game.primary(eye);
                }
            }
            if self.input.key_pressed(KeyCode::KeyT) {
                self.game.switch_ammo();
            }
            if self.input.key_pressed(KeyCode::KeyE) {
                self.game.interact(eye);
                // Trading with a merchant frees the mouse for their stall
                if self.game.shopping() {
                    self.set_focus(false);
                }
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
            let start = Instant::now();
            self.game.update(&self.world, &|x, y| water.surface_at(x, y), feet, &view, controls, delta_time.min(0.1));
            self.slow("the fighting, animals and such", start);
            // A shot kicking the view up, springing back down
            let kick = self.game.kick();
            self.camera.rotation.x += kick - self.kicked;
            self.kicked = kick;
            let cues = self.game.take_cues();
            if let Some(sound) = &mut self.audio {
                for cue in cues {
                    sound.play(cue, &view);
                }
            }
            // Explosions blowing holes in the world, and splashing stains on it, sent to the GPU
            // all together
            self.batching = true;
            for (center, radius) in self.game.take_craters() {
                self.blast(center, radius);
            }
            let start = Instant::now();
            for (center, radius, material, burn) in self.game.take_stains() {
                self.stain(center, radius, material, burn);
            }
            self.slow("splashing stains", start);
            self.batching = false;
            let start = Instant::now();
            self.flush_edits();
            self.slow("sending what changed to the GPU", start);
            // Thrown by blows, and waking up somewhere safe after being beaten
            let push = self.game.take_knockback();
            if let Some(player) = &mut self.player {
                player.push(push);
            }
            // Slain, the player lies there until they rise again, at the last place they stood
            // safe. Screenshot runs rise by themselves.
            if self.game.take_died()
                && let Some(capture) = &self.capture
            {
                println!("DIED at frame {}: {}", capture.frame_times.len(), self.game.last_message().unwrap_or(""));
            }
            let rise = (interactive && self.focused && (self.input.key_pressed(KeyCode::Space) || self.input.mouse_pressed(MouseButton::Left)))
                || (self.capture.is_some() && self.game.dead_for() > 4.0);
            if self.game.can_rise() && rise {
                let spot = self.game.rise();
                if let Some(feet) = spot {
                    let player = Player::new(feet + Vector3::new(0.0, 0.0, 1.6 * self.world.voxels_per_meter as f64), &self.world);
                    self.camera.position = voxels_to_camera(player.eye(&self.world), self.world.size);
                    self.player = Some(player);
                }
                if let Some(capture) = &self.capture {
                    println!("ROSE at frame {} {}", capture.frame_times.len(), if spot.is_some() { "where it was safe" } else { "where they fell" });
                }
            }
            // `--set leap_at=N` takes the player 150 m on at frame N of a screenshot run, further
            // than strips can keep up with, as rising far off can: the world's built afresh round
            // them, see stream.rs
            static LEAP_AT: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
            let leap_at = *LEAP_AT.get_or_init(|| settings_from_args().into_iter().find(|(k, _)| k == "leap_at").and_then(|(_, v)| v.parse().ok()));
            if let Some(capture) = &self.capture
                && leap_at == Some(capture.frame_times.len())
                && let Some(player) = &mut self.player
            {
                player.position.x += 150.0 * self.world.voxels_per_meter as f64;
                self.camera.position = voxels_to_camera(player.eye(&self.world), self.world.size);
                println!("LEAPT at frame {} 150 m on", capture.frame_times.len());
            }
        }
        self.ambience(delta_time);

        if playing && self.input.key_pressed(KeyCode::KeyF) {
            self.player = match self.player {
                Some(_) => None,
                None => Some(Player::new(camera_to_voxels(self.camera.position, self.world.size), &self.world)),
            };
        }

        // The dead don't walk
        let alive = !self.game.dead();
        let t = |k: KeyCode| (self.focused && interactive && alive && self.input.key_held(k)) as u8 as f64;
        let mut footstep = None;
        // Nor does anyone while the world's built afresh round them, see stream.rs
        let held = self.held_still();
        if let Some(player) = &mut self.player
            && !held
        {
            // Walk where the camera faces, ignoring whether it looks up or down
            let yaw = self.camera.rotation.z;
            let forward = Vector2::new(-yaw.sin(), yaw.cos());
            let right = Vector2::new(yaw.cos(), yaw.sin());
            let mut wish = forward * (t(KeyCode::KeyW) - t(KeyCode::KeyS)) + right * (t(KeyCode::KeyD) - t(KeyCode::KeyA));
            if self.scripted_walk && alive {
                wish = forward;
            }
            if wish.norm() > 1.0 {
                wish.normalize_mut();
            }
            // Slowed by frost
            wish *= self.game.pace();
            let jump = t(KeyCode::Space) > 0.0;
            let sprint = alive && (self.focused && self.input.held_shift() || self.scripted_sprint);
            // Long hitches, like generating a world, shouldn't fling the player through the floor
            let water = &self.water;
            let before = player.position;
            player.update(&self.world, &|x, y| water.surface_at(x, y), wish, jump, sprint, delta_time.min(0.05));
            // Fallen dead, the view sinks to the ground
            let fallen = Vector3::new(0.0, 0.0, 1.0 * self.game.fallen() * self.world.voxels_per_meter as f64);
            self.camera.position = voxels_to_camera(player.eye(&self.world) - fallen, self.world.size);

            // Wading through water pushes it along
            let vpm = self.world.voxels_per_meter as f64;
            let feet = player.position;
            let moved = self.last_feet.map_or(Vector3::zeros(), |last| (feet - last) / delta_time.max(1e-3));
            // In screenshot runs, any sudden jump of the player's, to catch what throws them
            if let (Some(capture), Some(last)) = (&self.capture, self.last_feet)
                && (feet - last).norm() > 1.5 * vpm
            {
                println!("JUMP of {:.1} m at frame {}: {}", (feet - last).norm() / vpm, capture.frame_times.len(), self.game.report(feet));
            }
            self.last_feet = Some(feet);
            let wading = self.water.depth_at(feet.x, feet.y) > 0.1 * vpm;
            self.wake = (wading && !self.menu_open).then_some(([feet.x, feet.y], [moved.x, moved.y]));

            // How fast they're going over the ground and how far they've walked, to bob what's in hand
            let over_ground = if player.on_ground() { (feet - before).xy().norm() / vpm } else { 0.0 };
            let speed = over_ground / delta_time.min(0.05).max(1e-3);
            self.walk_speed += (speed - self.walk_speed) * (1.0 - (-10.0 * delta_time).exp());
            self.walked += over_ground;

            // A footstep every stride, sounding like whatever's underfoot
            if player.on_ground() {
                self.step_distance += (feet - before).xy().norm() / vpm;
                if self.step_distance > if sprint { 0.95 } else { 0.7 } {
                    self.step_distance = 0.0;
                    let under = self.world.material_at(feet.x.floor() as i64, feet.y.floor() as i64, feet.z.floor() as i64 - 1);
                    footstep = Some(if self.water.depth_at(feet.x, feet.y) > 0.05 * vpm { audio::Ground::Water } else { ground_of(under) });
                }
            }
        } else if self.player.is_none() {
            self.last_feet = None;
            self.wake = None;
            self.walk_speed = 0.0;
            if self.focused && interactive {
                let v = Vector3::new(KeyCode::KeyD, KeyCode::KeyW, KeyCode::KeyQ).map(t)
                    - Vector3::new(KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyE).map(t);

                let sprint = if self.input.held_shift() { 4.0 } else { 1.0 };
                let step = (self.camera.rotation_matrix() * v.push(0.0) * delta_time * self.move_speed * sprint).xyz();
                // No faster out to the edge of the world than it moves on, see stream.rs
                let scale = self.world.size.iter().copied().max().unwrap_or(1) as f64;
                self.camera.position += self.keep_up(step * scale) / scale;
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

    /// Sets everything a quality preset does, as when one's picked in the settings
    fn apply_quality(&mut self, preset: quality::Preset) {
        let s = preset.settings();
        self.quality = preset;
        self.render_scale = s.render_scale;
        self.still_render_scale = s.render_scale;
        self.shadows = s.shadows;
        self.ambient_occlusion = s.ambient_occlusion;
        self.bounce_light = s.bounce_light;
        self.smooth_shading = s.smooth_shading;
        self.wind = s.wind;
        self.denoise = s.denoise;
        self.bloom = s.bloom;
        self.view_distance = s.view_distance;
        self.reflections = s.reflections;
        self.fast_upscale = s.fast_upscale;
        self.shafts.quality = s.sun_shafts;
        water::set_flowing_area((s.flowing_water_m * self.world.voxels_per_meter as f32) as u32);
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
        let run_time = capture.started.elapsed().as_secs_f64();
        let vpm = self.world.voxels_per_meter as f64;
        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        let before = self.game.last_message().map(str::to_owned);
        match (script.as_str(), frame) {
            ("play", f) => self.play_step(f, run_time, eye),
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
            // Runs straight on and on, saying now and then how far the edge of the world is ahead,
            // to see the next piece of the landscape is ready before the edge is reached
            ("trek", f) => {
                if f == 1 && self.player.is_none() {
                    self.player = Some(Player::new(eye, &self.world));
                }
                self.scripted_walk = true;
                self.scripted_sprint = true;
                if f % 30 == 0 {
                    let [sx, sy, _] = self.world.size.map(|n| n as f64);
                    let edge = eye.x.min(eye.y).min(sx - eye.x).min(sy - eye.y) / vpm;
                    let at = [self.world.origin[0] as f64 / vpm + eye.x / vpm, self.world.origin[1] as f64 / vpm + eye.y / vpm];
                    println!("frame {frame}: at {:.0}, {:.0}, edge {edge:.0} m off, a strip coming in: {}", at[0], at[1], self.stream.as_ref().is_some_and(|s| s.busy()));
                }
            }
            // Flies over the land `--set fly_speed=27` meters a second, as flying with shift held is,
            // turning sharply round every `--set fly_turn=8` seconds or so, to see the world keeps up
            // as it moves on a strip at a time, see stream.rs
            ("fly", f) => {
                let setting = |key: &str, default: f64| settings_from_args().into_iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(default);
                if f == 1 {
                    self.player = None;
                    self.play.heading = self.camera.rotation.z;
                    self.play.leg_end = run_time + setting("fly_turn", 8.0);
                }
                let dt = (run_time - self.play.fly_last).clamp(0.0, 0.1);
                self.play.fly_last = run_time;
                if run_time >= self.play.leg_end {
                    self.play.leg += 1;
                    self.play.heading += std::f64::consts::PI + (self.play.random() - 0.5) * 1.2;
                    self.play.leg_end = run_time + setting("fly_turn", 8.0) * (0.5 + self.play.random());
                    println!("frame {frame}: turning round");
                }
                // `--set shot_every=0.5`, a shot of the view every half a second
                if let Some(every) = self.play.shot_every
                    && run_time >= self.play.next_shot
                    && let Some(capture) = &mut self.capture
                {
                    self.play.next_shot = run_time + every;
                    capture.shots.push(frame + 2);
                }
                self.camera.rotation.z = self.play.heading;
                self.camera.rotation.x = -0.25;
                let forward = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
                let flat = Vector3::new(forward.x, forward.y, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
                // As far as flying gets, no faster than the world moves on, see keep_up()
                let step = self.keep_up(flat * setting("fly_speed", 27.0) * vpm * dt);
                self.play.flown += step.norm() / vpm;
                let mut at = eye + step;
                let ground = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64;
                at.z = (ground + 12.0 * vpm).min(self.world.size[2] as f64 - 2.0 * vpm);
                self.camera.position = voxels_to_camera(at, self.world.size);
                if f % 30 == 0 {
                    let [sx, sy, _] = self.world.size.map(|n| n as f64);
                    let exit = |e: f64, s: f64, d: f64| if d > 1e-9 { (s - e) / d } else if d < -1e-9 { -e / d } else { f64::INFINITY };
                    let ahead = exit(at.x, sx, flat.x).min(exit(at.y, sy, flat.y)) / vpm;
                    let world = [self.world.origin[0] as f64 / vpm + at.x / vpm, self.world.origin[1] as f64 / vpm + at.y / vpm];
                    println!(
                        "frame {frame}: at {:.0}, {:.0}, edge {ahead:.0} m ahead, flown {:.0} m in {run_time:.1} s, a strip coming in: {}",
                        world[0],
                        world[1],
                        self.play.flown,
                        self.stream.as_ref().is_some_and(|s| s.busy())
                    );
                }
            }
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
                let aim = |attack| combat::Controls { attack, holding: attack, aiming: true, ..Default::default() };
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
            // Unleashes the special attack of what's in hand, facing the nearest boss if there is one
            ("special", f) => {
                // `--set special_frame=150` lets it go later, as when a boss is about to strike
                let when = settings_from_args().into_iter().find(|(k, _)| k == "special_frame").and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(30);
                // `--set on_foot=1` lets it go standing on the ground, as a player would
                if f == 1 && self.player.is_none() && settings_from_args().iter().any(|(k, v)| k == "on_foot" && v == "1") {
                    self.player = Some(Player::new(eye, &self.world));
                }
                // Kept facing the boss as it comes on, till it's let go, or with `--set
                // special_face=pack` the nearest of a pack `--set enemy` put there
                let pack = settings_from_args().iter().any(|(k, v)| k == "special_face" && v == "pack");
                if f == 1 || f <= when {
                    match if pack { self.game.nearest_foe(eye) } else { self.game.nearest_boss(eye) } {
                        Some(target) => self.camera.look_at(voxels_to_camera(target, self.world.size)),
                        None if f == 1 => self.camera.rotation.x = -0.35,
                        None => {}
                    }
                }
                self.script_controls = combat::Controls { special: f == when, ..Default::default() };
                if f == 29 || f == 120 || f % 50 == 0 {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
            // Finds the nearest water and casts into it, to see it splash
            ("splash", f) => {
                if f == 1 {
                    let vpm = self.world.voxels_per_meter as f64;
                    let mut best: Option<Vector3<f64>> = None;
                    for ring in 4..40 {
                        for k in 0..ring * 6 {
                            let a = k as f64 / (ring * 6) as f64 * TAU;
                            let (x, y) = (eye.x + a.cos() * ring as f64 * vpm, eye.y + a.sin() * ring as f64 * vpm);
                            let surface = self.water.surface_at(x, y);
                            if best.is_none() && surface.is_finite() && self.water.depth_at(x, y) > 0.5 * vpm {
                                best = Some(Vector3::new(x, y, surface));
                            }
                        }
                    }
                    match best {
                        Some(at) => self.camera.look_at(voxels_to_camera(at, self.world.size)),
                        None => println!("no water near"),
                    }
                }
                let cast = (30..33).contains(&f);
                self.script_controls = combat::Controls { attack: cast, holding: cast, ..Default::default() };
            }
            // One spell cast at the ground a few meters ahead, to watch it go off and what it leaves
            ("spells", f) => {
                if f == 1 {
                    self.camera.rotation.x = -0.6;
                }
                // Once it's come up into view
                let cast = (30..33).contains(&f);
                self.script_controls = combat::Controls { attack: cast, holding: cast, ..Default::default() };
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
            // Blows holes, hops far enough over that the world's built again around the camera
            // while it goes on blowing them, then checks none have filled back in
            // Blows a hollow under the ground ahead, to see sand, gravel and snow fall into it
            ("sand", f) => {
                if f == 1 {
                    self.camera.rotation.x = -0.45;
                }
                if f == 40 || f == 80 {
                    let vpm = self.world.voxels_per_meter as f64;
                    let ahead = eye + Vector3::new(look.x, look.y, 0.0).normalize() * (5.0 + (f / 40) as f64 * 2.5) * vpm;
                    let ground = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(ahead.x as i64, ahead.y as i64, z))).unwrap_or(0) as f64;
                    let material = self.world.material_at(ahead.x as i64, ahead.y as i64, ground as i64);
                    self.blast(Vector3::new(ahead.x, ahead.y, ground - 1.6 * vpm), 1.3);
                    println!("frame {frame}: hollow under {material} ground");
                }
            }
            // Blows a hole in the ground ahead every few frames, here and there, for the worst of
            // a fight's destruction going on in sight
            ("barrage", f) => {
                if f == 1 {
                    self.camera.rotation.x = -0.35;
                }
                if f > 20 && f % 8 == 0 {
                    let vpm = self.world.voxels_per_meter as f64;
                    let mut seed = f as u32 * 2654435761;
                    let mut next = || {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        (seed % 1000) as f64 / 1000.0
                    };
                    let flat = Vector3::new(look.x, look.y, 0.0).normalize();
                    let side = Vector3::new(-flat.y, flat.x, 0.0);
                    let at = eye + flat * (5.0 + 7.0 * next()) * vpm + side * (next() - 0.5) * 10.0 * vpm;
                    let ground = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64;
                    self.blast(Vector3::new(at.x, at.y, ground), 1.2 + 1.3 * next());
                }
            }
            ("holes", f) => {
                if f == 1 {
                    self.game.select_weapon(weapons::Kind::Magic);
                    self.camera.rotation.x = -0.9;
                }
                if f == 30 {
                    let vpm = self.world.voxels_per_meter as f64;
                    self.camera.position = voxels_to_camera(eye + Vector3::new(40.0 * vpm, 0.0, 0.0), self.world.size);
                }
                self.script_controls = combat::Controls { attack: f > 5 && f < 140, holding: f > 5 && f < 140, ..Default::default() };
                if f % 40 == 0 {
                    let vpm = self.world.voxels_per_meter as f64;
                    let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
                    let filled = self
                        .craters
                        .iter()
                        .map(|&(center, _)| Vector3::from(center) * vpm - origin)
                        .filter(|p| p.x >= 0.0 && p.y >= 0.0 && p.x < self.world.size[0] as f64 && p.y < self.world.size[1] as f64)
                        .filter(|p| !worldgen::is_passable(self.world.material_at(p.x as i64, p.y as i64, p.z as i64)))
                        .count();
                    println!("frame {frame}: {} holes blown, {filled} filled back in, a strip coming in: {}", self.craters.len(), self.stream.as_ref().is_some_and(|s| s.busy()));
                }
            }
            // On foot, facing the nearest boss and attacking it with what's in hand
            ("boss", f) => {
                if f == 1 && self.player.is_none() {
                    self.player = Some(Player::new(eye, &self.world));
                }
                // `--set boss_look_down=8` looks that many meters under it, to see what it does
                // round the player's feet
                let down = settings_from_args().into_iter().find(|(k, _)| k == "boss_look_down").and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(0.0);
                // `--set boss_approach=300` stands there that many frames, then walks up to it
                let approach = settings_from_args().into_iter().find(|(k, _)| k == "boss_approach").and_then(|(_, v)| v.parse::<usize>().ok());
                if let Some(target) = self.game.nearest_boss(eye) {
                    self.camera.look_at(voxels_to_camera(target - Vector3::new(0.0, 0.0, down * vpm), self.world.size));
                    self.scripted_walk = approach.is_some_and(|after| f > after && (target - eye).xy().norm() > 3.0 * vpm);
                }
                // `--set boss_pulse=50` holds the button that many frames at a time, letting go a
                // moment between, to ring a bell or such
                let pulse = settings_from_args().into_iter().find(|(k, _)| k == "boss_pulse").and_then(|(_, v)| v.parse::<usize>().ok());
                let pressing = f > 30 && pulse.is_none_or(|n| (f - 31) % (n + 12) < n);
                self.script_controls = combat::Controls { attack: pressing, holding: pressing, ..Default::default() };
                if f % 60 == 0 {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
            // Fights the boss near by, then walks up to the nearest weapon it dropped and looks at
            // it, to see its card. `--set take=1` picks it up once there.
            ("spoils", f) => {
                if f == 1 && self.player.is_none() {
                    self.player = Some(Player::new(eye, &self.world));
                }
                let take = settings_from_args().iter().any(|(k, v)| k == "take" && v == "1");
                let boss = self.game.nearest_boss(eye).filter(|b| (b - eye).norm() < 30.0 * vpm);
                self.script_controls = combat::Controls::default();
                self.scripted_walk = false;
                if let Some(target) = boss {
                    self.camera.look_at(voxels_to_camera(target, self.world.size));
                    self.script_controls = combat::Controls { attack: f > 30, holding: f > 30, ..Default::default() };
                } else if let Some(loot) = self.game.nearest_loot(eye) {
                    self.camera.look_at(voxels_to_camera(loot, self.world.size));
                    self.scripted_walk = (loot - eye).xy().norm() > 2.0 * vpm;
                    if take && !self.scripted_walk && f % 40 == 0 {
                        self.game.interact(eye);
                    }
                }
                if f % 60 == 0 {
                    println!("frame {frame}: {} · carrying {}", self.game.report(eye), self.game.carried());
                }
            }
            // Looks at the nearest monster from where the camera is, to see what it looks like and
            // does, without fighting it: see `--set enemy`
            ("enemy", f) => {
                if let Some(target) = self.game.nearest_boss(eye) {
                    self.camera.look_at(voxels_to_camera(target, self.world.size));
                }
                if f % 60 == 0 {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
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
            // On foot, shooting what's in hand as fast as it'll go at the nearest boss, or the
            // ground a little way ahead, and switching ammo every 150 frames
            ("shoot", f) => {
                if f == 1 && self.player.is_none() {
                    self.player = Some(Player::new(eye, &self.world));
                }
                match self.game.nearest_boss(eye).filter(|b| (b - eye).norm() < 40.0 * vpm) {
                    Some(target) => self.camera.look_at(voxels_to_camera(target, self.world.size)),
                    None if f == 1 => self.camera.rotation.x = -0.15,
                    None => {}
                }
                self.script_controls = combat::Controls { attack: f > 30, holding: f > 30, ..Default::default() };
                if f % 150 == 0 {
                    self.game.switch_ammo();
                }
                if f % 60 == 0 {
                    println!("frame {frame}: {} · carrying {}", self.game.report(eye), self.game.carried());
                }
            }
            // Standing still where the camera is, facing the nearest of a pack `--set enemy` put
            // there, and shooting what's in hand at it as fast as it'll go, reloading as it needs
            // to, from frame 30 on: for seeing what special ammo does where it lands, and how many
            // holes it's blown
            ("fire", f) => {
                if f <= 30 {
                    match self.game.nearest_foe(eye) {
                        Some(target) => self.camera.look_at(voxels_to_camera(target, self.world.size)),
                        None if f == 1 => self.camera.rotation.x = -0.2,
                        None => {}
                    }
                }
                self.script_controls = combat::Controls { attack: f >= 30, holding: f >= 30, ..Default::default() };
                if f % 100 == 0 {
                    println!("frame {frame}: {} holes blown", self.craters.len());
                }
            }
            // Draws all the way and shoots at the ground a little way ahead, over and over
            ("volley", f) => {
                if f == 1 {
                    self.camera.rotation.x = -0.22;
                }
                let phase = f % 70;
                self.script_controls = combat::Controls { aiming: phase > 5 && phase <= 60, attack: phase == 60, ..Default::default() };
            }
            ("loot", 20) => {
                println!("frame {frame}: last said \"{}\", aiming at something: {}, {}", self.game.last_message().unwrap_or(""), self.game.has_target(), self.game.report(eye));
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

    /// `--script play`, a long run of play for seeing whether anything gets slower the longer it
    /// goes on: on foot, walking and running about in legs a quarter to half a minute long, turning
    /// some way between them and stopping a moment every few seconds, fighting with a different
    /// kind of weapon each minute, with a pack of monsters turning up every two minutes, so the
    /// world moves on many times. With `--set churn=90` the settings change every 90 seconds, as a
    /// player trying them out would: the render scale up and down, while moving and while still
    /// apart and together, and the window bigger and smaller. `t` is seconds into the run.
    fn play_step(&mut self, frame: usize, t: f64, eye: Vector3<f64>) {
        let vpm = self.world.voxels_per_meter as f64;
        if frame == 1 {
            if self.player.is_none() {
                self.player = Some(Player::new(eye, &self.world));
            }
            self.play.heading = self.camera.rotation.z;
        }
        let feet = self.player.as_ref().map_or(eye, |p| p.position);
        // Turned away from whatever it's walked into
        let walking = t - self.play.leg_start > 3.0;
        let mut stuck = false;
        match self.play.check {
            Some((at, when)) if t - when > 4.0 => {
                stuck = walking && (feet - at).xy().norm() < 3.0 * vpm;
                self.play.check = Some((feet, t));
            }
            None => self.play.check = Some((feet, t)),
            _ => {}
        }
        if t >= self.play.leg_end || stuck {
            self.play.leg += 1;
            let side = if self.play.random() < 0.5 { -1.0 } else { 1.0 };
            self.play.heading += side * (0.4 + 0.6 * self.play.random()) * std::f64::consts::PI;
            self.play.leg_start = t;
            self.play.leg_end = t + 15.0 + 15.0 * self.play.random();
            self.play.check = None;
        }
        // A moment stood still every eight seconds, when the view settles and what's drawn at the
        // render scale for a still view, if that's different
        let standing = (t - self.play.leg_start) % 8.0 < 1.5;
        self.camera.rotation.z = self.play.heading;
        self.camera.rotation.x = -0.22;
        self.scripted_walk = !standing;
        self.scripted_sprint = self.play.leg % 3 != 0;

        // Swinging, shooting or casting most of the time, with something else each minute
        let minute = (t / 60.0) as u64;
        if self.play.minute != Some(minute) {
            self.play.minute = Some(minute);
            let kind = [weapons::Kind::Melee, weapons::Kind::Ranged, weapons::Kind::Magic][minute as usize % 3];
            self.game.select_weapon(kind);
        }
        let swinging = t % 10.0 < 6.0;
        self.script_controls = combat::Controls { attack: swinging, holding: swinging, ..Default::default() };

        if t >= self.play.next_pack {
            self.play.next_pack = t + 120.0;
            let monsters: Vec<animals::Look> = animals::Look::ALL.into_iter().filter(|l| l.is_monster()).collect();
            if !monsters.is_empty() {
                let look = monsters[self.play.leg as usize * 7 % monsters.len()];
                let ahead = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
                let water = &self.water;
                self.game.summon_pack(&self.world, &|x, y| water.surface_at(x, y), &vec![look; look.pack_size()], feet, ahead, 14.0, 0.0, false, None);
                println!("PLAY t={}: a pack of {} {}", perf::clock(t), look.pack_size(), look.name());
            }
        }

        if let Some(every) = self.play.churn
            && t >= self.play.next_churn
        {
            self.play.next_churn = t + every;
            let step = self.play.churns % 8;
            self.play.churns += 1;
            let mut resize = None;
            let what = match step {
                0 => {
                    self.render_scale = 2.0;
                    "render scale while moving 2.0, still as it was"
                }
                1 => {
                    resize = Some([1920, 1200]);
                    "window 1920x1200, as full screen"
                }
                2 => {
                    self.still_render_scale = 2.0;
                    "render scale still 2.0 too"
                }
                3 => {
                    resize = Some([1280, 800]);
                    "window 1280x800"
                }
                4 => {
                    self.render_scale = 1.0;
                    self.still_render_scale = 1.0;
                    "render scale 1.0 moving and still"
                }
                5 => {
                    resize = Some([1920, 1200]);
                    "window 1920x1200, as full screen"
                }
                6 => {
                    self.render_scale = 0.45;
                    self.still_render_scale = 0.45;
                    "render scale back to 0.45"
                }
                _ => {
                    resize = Some([1280, 800]);
                    "window 1280x800"
                }
            };
            if let (Some([w, h]), Some(rcx)) = (resize, &self.rcx) {
                let _ = rcx.window.request_inner_size(PhysicalSize::new(w, h));
            }
            let vram = perf::vram(&self.device);
            println!("CHURN t={}: {what}, vram {:.0} of {:.0} MB", perf::clock(t), vram.map_or(-1.0, |v| v.used as f64 / perf::MB), vram.map_or(-1.0, |v| v.budget as f64 / perf::MB));
        }

        if let Some(every) = self.play.shot_every
            && t >= self.play.next_shot
        {
            self.play.next_shot = t + every;
            // This frame's number as render() counts it, once it's taken this one's time
            if let Some(capture) = &mut self.capture {
                capture.shots.push(frame + 2);
            }
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
        if self.stream.is_none() {
            self.near = None;
            return;
        }
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
            if build.world_build == self.world_builds && world.voxels_per_meter == fine_vpm {
                // Its origin is from the corner of the whole landscape, the world's isn't
                let window = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
                self.near = Some(NearDetail {
                    set,
                    origin: Vector3::from(world.origin).cast::<f64>() / scale as f64 - window,
                    scale: scale as f64,
                    size: world.size,
                    center: build.center,
                    craters: build.craters,
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
        // A hole's been blown or a stain splashed since it was built
        let scars = self.craters.len() + self.stains.len();
        let holed = self.near.as_ref().is_some_and(|near| near.craters < scars);
        if (moved || holed) && self.near_build.is_none() {
            let size = [NEAR_WIDTH_M * fine_vpm, NEAR_WIDTH_M * fine_vpm, NEAR_HEIGHT_M * fine_vpm];
            let window = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            let center = (camera + window) * scale as f64;
            // Brick aligned, and within the height of the world
            let corner = |axis: usize| ((center[axis] - size[axis] as f64 * 0.5) / 8.0).round() as i64 * 8;
            let top = self.world.size[2] as i64 * scale as i64 - size[2] as i64;
            let origin = [corner(0), corner(1), corner(2).clamp(0, top.max(0))];
            // Placed just as the world is, at its voxels, out to as far as reaches into the box
            let settings = self.terrain_settings();
            let rivers = self.rivers.clone();
            let place_at = [origin[0].div_euclid(scale as i64), origin[1].div_euclid(scale as i64)];
            let place_size = [size[0] / scale + 8, size[1] / scale + 8];
            let (memory_allocator, command_buffer_allocator, descriptor_set_allocator, queue) = (
                self.memory_allocator.clone(),
                self.command_buffer_allocator.clone(),
                self.descriptor_set_allocator.clone(),
                self.queue.clone(),
            );
            let render_pipeline = (*self.render_pipeline).clone();
            let craters = self.craters.clone();
            let stains = self.stains.clone();
            let job = std::thread::spawn(move || {
                let start = Instant::now();
                let landscape = worldgen::place_for(&settings, place_at, place_size, &rivers);
                let mut world = landscape.build(origin, size, fine_vpm);
                // With the holes blown in it and the stains splashed on it
                let vpm = fine_vpm as f64;
                let local = |center: [f64; 3], radius: f64| {
                    let p = [center[0] * vpm - origin[0] as f64, center[1] * vpm - origin[1] as f64, center[2] * vpm - origin[2] as f64];
                    let reach = radius * vpm + 2.0;
                    (0..3).all(|i| p[i] > -reach && p[i] < size[i] as f64 + reach).then_some(p)
                };
                for (center, radius) in craters {
                    if let Some(p) = local(center, radius) {
                        world.carve(p, radius * vpm);
                    }
                }
                for (center, radius, material, burn) in stains {
                    if let Some(p) = local(center, radius) {
                        world.paint(p, radius * vpm, material, burn);
                    }
                }
                // Uploaded here too, so the frames carry on smoothly meanwhile
                let set = get_voxel_set(memory_allocator, command_buffer_allocator, descriptor_set_allocator, &render_pipeline, &queue, 2, &world, 0).set;
                println!("Built fine detail: {} bricks ({:?})", world.brick_count(), start.elapsed());
                (world, set)
            });
            self.near_build = Some(NearBuild {
                job,
                world_build: self.world_builds,
                center: camera,
                craters: scars,
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
        self.skips_pipeline.maybe_reload();
        if let Some(preset) = self.pending_quality.take() {
            self.apply_quality(preset);
            quality::keep(preset);
        }
        let start = Instant::now();
        self.update_window();
        self.slow("moving the world along", start);
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
        // No more than so many frames queued up for the GPU: the oldest is waited for
        let waiting = Instant::now();
        while self.max_in_flight > 0 && rcx.in_flight.len() >= self.max_in_flight {
            let _ = rcx.in_flight.pop_front().unwrap().wait(None);
        }
        // Far longer than frames take
        if let Some(capture) = &self.capture
            && waiting.elapsed().as_secs_f64() * 1000.0 > (3.0 * self.frame_ms).max(50.0)
        {
            println!("Slow at frame {}: waiting for the frames before took {:.0} ms", capture.frame_times.len() + 1, waiting.elapsed().as_secs_f64() * 1000.0);
        }

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

        // Lower resolution while the view changes, full detail once it settles, as much as the
        // GPU's memory has room for
        self.update_render_images(window_size.into());
        let rcx = self.rcx.as_mut().unwrap();

        let acquiring = Instant::now();
        let (image_index, suboptimal, acquire_future) =
            match acquire_next_image(rcx.swapchain.clone(), None).map_err(Validated::unwrap) {
                Ok(r) => r,
                Err(VulkanError::OutOfDate) => {
                    rcx.recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("failed to acquire next image: {e}"),
            };
        rcx.presented.acquired(image_index);
        if let Some(capture) = &self.capture
            && acquiring.elapsed().as_secs_f64() > 0.015
        {
            println!("Slow at frame {}: waiting for an image to draw on took {:.0} ms", capture.frame_times.len() + 1, acquiring.elapsed().as_secs_f64() * 1000.0);
        }

        if suboptimal {
            rcx.recreate_swapchain = true;
        }

        // What the menu and inventory ask for, done once the GUI is drawn
        let mut focus_change = None;
        let mut quit = false;
        // Under the render scale sliders: what that comes to
        let render_note = self.show_settings.then(|| {
            let [w, h, _] = rcx.images.render_image.extent();
            let (in_use, wanted) = (rcx.render_scale_in_use, rcx.render_scale_wanted);
            let mut note = format!(
                "Rendering {w}x{h}, {:.0} MB of the GPU's memory, about {:.1}x the work of {DEFAULT_RENDER_SCALE}",
                rcx.images.bytes() as f64 / perf::MB,
                (in_use / DEFAULT_RENDER_SCALE).powi(2)
            );
            if in_use < wanted * 0.98 {
                note += &format!("\nRender scale {in_use:.2} rather than {wanted:.2}: as much as the GPU's memory has room for");
            }
            if let Some(vram) = perf::vram(&self.device) {
                note += &format!("\nGPU memory: {:.1} of {:.1} GB in use", vram.used as f64 / perf::MB / 1024.0, vram.budget as f64 / perf::MB / 1024.0);
            }
            if let Some(stream) = &self.stream {
                let vpm = self.world.voxels_per_meter as f64;
                let half = self.world.size[0].min(self.world.size[1]) as f64 * 0.5 / vpm;
                let sees = fade_distance(self.world.size, stream.strip, vpm, 1.0) / vpm;
                note += &format!("\nSees {sees:.0} m: the piece of the landscape around you is {:.0} m across, and moves on {} m at a time", 2.0 * half, stream::STRIP_M);
            }
            note
        });

        rcx.gui.immediate_ui(|gui| {
            let ctx = gui.context();

            // Dark while the world's built afresh round the camera, gone too fast for the strips to
            // keep up, see stream.rs
            if let Some(dark) = self.stream.as_ref().and_then(|s| s.teleport.as_ref()).map(|t| t.dark)
                && dark > 0.0
            {
                ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("moving on")))
                    .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha((dark.clamp(0.0, 1.0) * 255.0) as u8));
            }

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
                                             Crossbow, gun: click to shoot, right click reloads · T ammo\n\
                                             Right click heavy swing, or eat · E pick up, open, trade, pet\n\
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
                // A merchant's stall closed, or walked away from: back to playing
                let shopping = self.game.shopping() && self.game.shop_window(&ctx);
                if self.shopping && !shopping && !self.inventory_open && self.capture.is_none() {
                    focus_change = Some(true);
                }
                self.shopping = shopping;
                if !shopping && self.inventory_open {
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
                // Sets the render scale and the lighting below, and is kept for next time
                egui::ComboBox::from_label("Quality").selected_text(self.quality.name()).show_ui(ui, |ui| {
                    for preset in quality::Preset::ALL {
                        if ui.selectable_label(self.quality == preset, preset.name()).clicked() {
                            self.pending_quality = Some(preset);
                        }
                    }
                });
                ui.add(egui::Slider::new(&mut self.camera.fov, 0.0..=180.0).text("FOV"));
                ui.add(egui::Slider::new(&mut self.render_scale, 0.125..=2.0).text("Render Scale While Moving"));
                ui.add(egui::Slider::new(&mut self.still_render_scale, 0.125..=2.0).text("Render Scale When Still"));
                if let Some(note) = &render_note {
                    ui.label(RichText::new(note).size(12.0).color(Color32::from_gray(190)));
                }
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
                        self.shafts.settings_ui(ui);
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
                            // Only ever the one piece of it, moving on a strip at a time, see memory.rs
                            let piece = memory::piece_bytes(self.terrain.width_m, self.terrain.height_m, self.terrain.voxels_per_meter);
                            let gb = |b: u64| b as f64 / (1u64 << 30) as f64;
                            match self.memory.budget {
                                Some(budget) if piece > budget / 10 * 8 => {
                                    ui.colored_label(Color32::LIGHT_RED, format!("About {:.1} GB of the GPU's memory: more than its {:.1} GB, so it'd run slowly and the world could be seen to end", gb(piece), gb(budget)));
                                }
                                _ => {
                                    ui.label(format!("About {:.1} GB of the GPU's memory", gb(piece)));
                                }
                            }
                            let max_bricks = limits::get().image_3d * 8;
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

        // Shaken by blows and explosions, and tipped over on falling dead: only what's drawn, not
        // where the player's looking
        let shake = self.game.shake();
        let fallen = self.game.fallen();
        let pixel_to_ray = if (shake > 0.01 || fallen > 0.0) && self.loaded_scene.is_terrain() {
            let t = self.start_time.elapsed().as_secs_f64();
            let amount = shake * shake * 0.012;
            let rotation = self.camera.rotation;
            self.camera.rotation.x += amount * ((t * 41.0).sin() + 0.5 * (t * 67.0).sin()) - 0.25 * fallen;
            self.camera.rotation.y += 1.2 * fallen;
            self.camera.rotation.z += amount * ((t * 37.0 + 1.0).sin() + 0.5 * (t * 59.0).sin());
            let shaken = self.camera.pixel_to_ray_matrix();
            self.camera.rotation = rotation;
            shaken
        } else {
            self.camera.pixel_to_ray_matrix()
        };

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
        // Sun shafts on High do the sunbeams' job better
        let sunbeams = self.sunbeams && !self.shafts.in_the_haze();
        if sunbeams {
            flags |= FLAG_SUNBEAMS;
        }
        if self.shafts.in_the_haze() {
            flags |= FLAG_SHAFTS;
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
        if self.reflections {
            flags |= FLAG_REFLECTIONS;
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
            let start = Instant::now();
            self.models_version = self.game.models_version();
            self.weapon_models = model_buffer(&self.memory_allocator, self.game.weapon_model_words());
            if self.capture.is_some() {
                println!("Models sent in {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
            }
        }

        // The weapon in hand, and the animals, food and things flying near the camera, for the
        // shader to trace
        let playing = self.loaded_scene.is_terrain() && self.animals_on;
        let instances: Vec<AnimalInstance> = if playing { self.game.instances(&self.world, &view) } else { Vec::new() };
        if let Some(perf) = &mut self.perf {
            perf.instances = perf.instances.max(instances.len());
        }
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

        // The view, for traverse.comp and shafts.comp
        let previous_world_to_pixel = self
            .last_view
            .as_ref()
            .map_or(Matrix4::identity(), |last| world_to_pixel(&last.pixel_to_ray.cast()));
        let view_buffer = self.instance_allocator.allocate_sized::<View>().unwrap();
        *view_buffer.write().unwrap() = View { pixel_to_ray: pixel_to_ray.cast(), previous_world_to_pixel: previous_world_to_pixel.cast() };

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
                WriteDescriptorSet::buffer(7, self.weapon_models.clone()),
                WriteDescriptorSet::buffer(8, view_buffer.clone()),
            ],
            [],
        )
        .unwrap();
        // Things fade into the sky before the edge of the world, which is never nearer than this
        let fade = match &self.stream {
            Some(stream) => fade_distance(self.world.size, stream.strip, self.world.voxels_per_meter as f64, self.view_distance) as f32,
            None => 0.0,
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

        // The view's in view_buffer, see View
        #[derive(BufferContents, Clone, Copy)]
        #[repr(C)]
        struct PushConstants {
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
            world_size: [sx, sy, sz, self.world.voxels_per_meter],
            sun_direction: view.sun_direction,
            render_mode: view.render_mode,
            flags: view.flags,
            frame: self.frame,
            history_limit,
            near_origin,
            near_size,
            haze: [view.haze, self.start_time.elapsed().as_secs_f32(), lights.len() as f32, fade],
            // Where its first column is kept in the room there is, which they go round, see
            // World::wrap
            window: [self.world.origin[0] as i32, self.world.origin[1] as i32, self.world.wrap[0] as i32, self.world.wrap[1] as i32],
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
        // Screenshot runs time each part of the frame on the GPU
        let device = self.device.clone();
        let mut timer = self.capture.as_mut().filter(|c| c.time_gpu).and_then(|c| {
            // Not where the GPU can't time its work
            let timer = c.gpu.take().or_else(|| gpu_timing::GpuTimer::new(&device));
            c.time_gpu = timer.is_some();
            timer
        });
        if let Some(timer) = &mut timer {
            timer.begin(&mut builder);
        }

        builder
            .clear_color_image(ClearColorImageInfo::image(rcx.images.render_image.clone()))
            .unwrap();
        if clear_light_cache {
            builder.fill_buffer(self.light_cache.clone(), 0).unwrap();
        }

        // The water flows on, except while the menu is open
        let now = Instant::now();
        let frame_time = if self.menu_open { 0.0 } else { (now - self.last_frame).as_secs_f64() };
        // What's sent to the GPU meanwhile, like the world as it's first sent over, goes over at a
        // pace that suits how long frames take, see staging.rs
        let took = ((now - self.last_frame).as_secs_f64() * 1000.0).min(500.0);
        self.frame_ms += (took - self.frame_ms) * 0.1;
        staging::frame_time(self.frame_ms);
        self.last_frame = now;
        let camera = camera_to_voxels(self.camera.position, self.world.size);
        // What's come of strips coming into the world goes over with the frame, see stream.rs, after
        // the bricks there were if there's more room for them now
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "clear");
        }
        self.voxel_set.copy_over(&mut builder);
        if let Some(stream) = &mut self.stream {
            stream.record_uploads(&mut builder, &self.voxel_set, &self.water, self.frame, self.frame_ms);
            stream.skips.record(&mut builder, &self.skips_pipeline, &self.voxel_set.views[0].1, &self.descriptor_set_allocator);
        }
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "strips");
        }
        self.water.record(&mut builder, &self.water_pipeline, frame_time, [camera.x, camera.y], self.wake, |b, n| {
            if let Some(timer) = &mut timer {
                timer.mark(b, n);
            }
        });
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "water");
        }

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
        if shaded && sunbeams && self.haze > 0.0 && self.sun_map_for != Some(sun_direction) {
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
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "trace");
        }

        if shaded {
            let images = &rcx.images;
            let frame = shafts::Frame {
                pixel_to_ray: pixel_to_ray.cast(),
                view: &view_buffer,
                sun_direction,
                haze: self.haze,
                world_size: self.world.size,
                voxels_per_meter: self.world.voxels_per_meter,
                world_origin: [self.world.origin[0], self.world.origin[1]],
                world_wrap: self.world.wrap,
                seconds: push_constants.haze[1],
                frame: push_constants.frame,
                still: history_limit == STILL_HISTORY || history_limit == ANIMATED_HISTORY,
                grid: &self.voxel_set.views[0].1,
                bricks: &self.voxel_set.bricks,
                depths: &images.depths,
                guide: &images.guide,
                hdr: &images.hdr,
            };
            self.shafts.record(&mut builder, &frame, timer.as_mut());
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
            // Before the bloom, so shafts towards the sun glow too
            self.shafts.composite(&mut builder);
            if self.bloom > 0.0 {
                dispatch_post(&mut builder, &self.post_pipeline, &images.bloom_extract_set, constants(PostStage::BloomExtract, 0), images.bloom_extent);
                dispatch_post(&mut builder, &self.post_pipeline, &images.bloom_blur_sets[0], constants(PostStage::BloomBlurX, 0), images.bloom_extent);
                dispatch_post(&mut builder, &self.post_pipeline, &images.bloom_blur_sets[1], constants(PostStage::BloomBlurY, 0), images.bloom_extent);
            }
            dispatch_post(&mut builder, &self.post_pipeline, &images.tonemap_set, constants(PostStage::Tonemap, 0), render_extent);
        }
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "post");
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
                pixel_to_ray: top_rows(&pixel_to_ray),
                previous_world_to_pixel: top_rows(&previous.unwrap_or(world_to_window)),
                frame: push_constants.frame,
                flags: if previous.is_some() { UPSCALE_HISTORY } else { 0 } | if self.fast_upscale { UPSCALE_FAST } else { 0 },
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
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "upscale");
        }

        // How far away what a grid of pixels shows is, to see how much of the view is sky once
        // it's back, see perf.rs
        if shaded && self.sky_check > 0 && self.sky_probe.is_none() && self.frame % self.sky_check == 0 {
            let depth = rcx.images.depths[parity].image().clone();
            let [w, h, _] = depth.extent();
            let (across, down) = (64u32, 36u32);
            let regions: Vec<BufferImageCopy> = (0..across * down)
                .map(|k| {
                    let (i, j) = (k % across, k / across);
                    BufferImageCopy {
                        buffer_offset: k as u64 * 4,
                        image_subresource: depth.subresource_layers(),
                        image_offset: [((i as f32 + 0.5) / across as f32 * w as f32) as u32, ((j as f32 + 0.5) / down as f32 * h as f32) as u32, 0],
                        image_extent: [1, 1, 1],
                        ..Default::default()
                    }
                })
                .collect();
            let probe = Buffer::new_slice::<f32>(
                self.memory_allocator.clone(),
                BufferCreateInfo { usage: BufferUsage::TRANSFER_DST, ..Default::default() },
                AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS, ..Default::default() },
                (across * down) as u64,
            )
            .unwrap();
            builder
                .copy_image_to_buffer(CopyImageToBufferInfo { regions: regions.into(), ..CopyImageToBufferInfo::image_buffer(depth, probe.clone()) })
                .unwrap();
            self.sky_probe = Some((probe, self.camera.rotation.x));
        }

        let mut capture_done = false;
        let mut capture_shot = None;
        if let Some(capture) = &mut self.capture {
            let now = Instant::now();
            if let Some(last) = capture.last_frame {
                let took = now - last;
                capture.frame_times.push(took);
                if let Some(perf) = &mut self.perf {
                    perf.frame(took.as_secs_f32() * 1000.0);
                    // How near the edge of the world the camera's come, and how long it was nearer
                    // than where things have faded well into the sky, so the world could be seen to end
                    if self.stream.is_some() {
                        let vpm = self.world.voxels_per_meter as f64;
                        let eye = camera_to_voxels(self.camera.position, self.world.size);
                        let [sx, sy, _] = self.world.size.map(f64::from);
                        let edge = eye.x.min(eye.y).min(sx - eye.x).min(sy - eye.y) / vpm;
                        perf.edge_min = perf.edge_min.min(edge);
                        if edge < 0.7 * fade as f64 / vpm {
                            perf.cut += took.as_secs_f64();
                        }
                        // And straight ahead, where it'd be seen
                        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
                        let flat = Vector2::new(look.x, look.y);
                        if flat.norm() > 1e-6 {
                            let d = flat / flat.norm();
                            let exit = |e: f64, s: f64, d: f64| if d > 1e-9 { (s - e) / d } else if d < -1e-9 { -e / d } else { f64::INFINITY };
                            let ahead = exit(eye.x, sx, d.x).min(exit(eye.y, sy, d.y)) / vpm;
                            perf.ahead_min = perf.ahead_min.min(ahead);
                            if ahead < 0.7 * fade as f64 / vpm {
                                perf.cut_ahead += took.as_secs_f64();
                            }
                            // A few shots of the world seen to end near ahead, this frame
                            let frame = capture.frame_times.len() + 1;
                            if ahead < 0.35 * fade as f64 / vpm && perf.edge_shots < 8 && frame > perf.last_edge_shot + 600 {
                                perf.edge_shots += 1;
                                perf.last_edge_shot = frame;
                                capture.shots.push(frame);
                                println!("EDGE at frame {frame}, {}: the world ends {ahead:.0} m ahead", perf::clock(capture.started.elapsed().as_secs_f64()));
                            }
                        }
                    }
                }
            }
            capture.last_frame = Some(now);
            let frame = capture.frame_times.len() + 1;
            capture_done = frame >= capture.frames as usize || capture.seconds.is_some_and(|s| capture.started.elapsed().as_secs_f64() >= s);
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

        // Held up when another thread's making room on the GPU or letting go of it
        let building = Instant::now();
        let command_buffer = builder.build().unwrap();
        if let Some(capture) = &self.capture
            && building.elapsed().as_secs_f64() > 0.010
        {
            println!("Slow at frame {}: building the frame's commands took {:.0} ms", capture.frame_times.len() + 1, building.elapsed().as_secs_f64() * 1000.0);
        }

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
        // With `--set gui=1`, shots are of the whole screen once the GUI's drawn over the game
        let screen = rcx.image_views[image_index as usize].image().clone();
        let screen_buffer = (capture_shot.is_some() && self.capture.as_ref().is_some_and(|c| c.gui)).then(|| {
            Buffer::new_slice::<u8>(
                self.memory_allocator.clone(),
                BufferCreateInfo { usage: BufferUsage::TRANSFER_DST, ..Default::default() },
                AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS, ..Default::default() },
                screen.extent()[0] as u64 * screen.extent()[1] as u64 * 4,
            )
            .unwrap()
        });
        let gui_future = match &screen_buffer {
            Some(buffer) => {
                let mut copy = AutoCommandBufferBuilder::primary(self.command_buffer_allocator.clone(), self.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
                copy.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(screen.clone(), buffer.clone())).unwrap();
                gui_future.then_execute(self.queue.clone(), copy.build().unwrap()).unwrap().boxed()
            }
            None => gui_future,
        };

        let submitting = Instant::now();
        // Sent to the GPU now, before the semaphore it's drawn behind and the present: left to the
        // present, now and then it went wrong inside vulkano, which let go of the frame's hold on
        // its images without having taken it and stopped the game ("Image was not locked for GPU
        // write"), on AVI as it was too, a few hundred frames into a run
        if let Err(e) = gui_future.flush() {
            println!("Sending the frame to the GPU went wrong: {e}");
        }
        // Shown once it's drawn, see loading::Drawn
        let drawn = loading::Drawn::after(gui_future);
        let semaphore = drawn.semaphore();
        let frame_end = drawn
            .boxed()
            .then_swapchain_present(
                self.queue.clone(),
                SwapchainPresentInfo::swapchain_image_index(rcx.swapchain.clone(), image_index),
            )
            .then_signal_fence_and_flush();
        if let Some(capture) = &self.capture
            && submitting.elapsed().as_secs_f64() > 0.010
        {
            println!("Slow at frame {}: submitting took {:.0} ms", capture.frame_times.len() + 1, submitting.elapsed().as_secs_f64() * 1000.0);
        }
        match frame_end.map_err(Validated::unwrap) {
            Ok(frame_end) => {
                // Screenshots read the frame back, so it has to be finished first. Screenshot runs
                // also wait for every frame, so their timings are the GPU's, unless `nowait`.
                if screenshot_buffer.is_some() || self.capture.as_ref().is_some_and(|c| !c.nowait) {
                    let waiting = Instant::now();
                    frame_end.wait(None).unwrap();
                    if let Some(capture) = &self.capture
                        && waiting.elapsed().as_secs_f64() > 0.015
                    {
                        println!("Slow at frame {}: the GPU took {:.0} ms", capture.frame_times.len() + 1, waiting.elapsed().as_secs_f64() * 1000.0);
                    }
                    if let Some(timer) = &mut timer {
                        timer.finish(waiting.elapsed());
                    }
                }
                if self.max_in_flight > 0 {
                    let frame_end = Arc::new(frame_end);
                    rcx.in_flight.push_back(frame_end.clone());
                    rcx.previous_frame_end = Some(frame_end.boxed());
                } else {
                    rcx.previous_frame_end = Some(frame_end.boxed());
                }
                // Kept till its image comes round again, see loading::Presented
                rcx.presented.showing(image_index, semaphore);
            }
            Err(VulkanError::OutOfDate) => {
                rcx.recreate_swapchain = true;
                rcx.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => panic!("failed to flush frame: {e}"),
        }
        if let Some(capture) = &mut self.capture
            && timer.is_some()
        {
            capture.gpu = timer;
        }
        if let Some(capture) = &self.capture
            && matches!(capture.frame_times.len(), 0 | 1 | 2 | 30 | 300)
        {
            note_vram(&self.device, &format!("{} frames drawn", capture.frame_times.len() + 1));
        }
        self.check_sky();
        if self.perf.as_ref().is_some_and(perf::PerfLog::due) {
            self.report_perf(render_extent, resample_extent);
        }

        if let Some(buffer) = screenshot_buffer {
            let encoded = post_flags & POST_ENCODE_SRGB != 0;
            if capture_done && let Some(mut capture) = self.capture.take() {
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
                    // Hitches, leaving out the first frames while the world settles in
                    let mut later: Vec<f64> = capture.frame_times.iter().skip(10).map(|t| t.as_secs_f64() * 1000.0).collect();
                    later.sort_by(f64::total_cmp);
                    if !later.is_empty() {
                        let worst = later.len() - 1;
                        println!(
                            "after the first 10 frames: 99th percentile {:.1} ms, slowest {:.1} ms, {} over 25 ms, {} over 20 ms",
                            later[worst * 99 / 100],
                            later[worst],
                            later.iter().filter(|&&t| t > 25.0).count(),
                            later.iter().filter(|&&t| t > 20.0).count()
                        );
                        let long: Vec<String> = capture.frame_times.iter().enumerate().skip(10).filter(|(_, t)| t.as_secs_f64() > 0.025).map(|(k, t)| format!("{}: {:.0} ms", k + 1, t.as_secs_f64() * 1000.0)).collect();
                        println!("long frames: {}", long.join(", "));
                    }
                }
                if let Some(timer) = &mut capture.gpu {
                    println!("{}", timer.report());
                }
                event_loop.exit();
            } else if let Some(path) = capture_shot {
                match &screen_buffer {
                    Some(screen_buffer) => save_screen(screen_buffer, screen.extent(), screen.format(), &path),
                    None => save_screenshot(&buffer, resample_extent, encoded, &path),
                }
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

/// How the window's made. It's never seen empty: it's shown once the loading screen's drawn in it,
/// see loading.rs, and handed over to the game once it's loaded, see App::attach().
fn window_attributes(out_of_sight: bool) -> winit::window::WindowAttributes {
    // `--set window=1920x1080` opens it that size, to see how it runs maximized on a screen like that
    let size = settings_from_args()
        .into_iter()
        .find(|(k, _)| k == "window")
        .and_then(|(_, v)| v.split_once('x').and_then(|(w, h)| Some(PhysicalSize::new(w.parse().ok()?, h.parse().ok()?))))
        .unwrap_or(INITIAL_WINDOW_RESOLUTION);
    let mut attributes = Window::default_attributes()
        .with_inner_size(size)
        .with_window_icon(Some(load_icon(include_bytes!("../assets/icon.png"))))
        .with_title("Innert")
        .with_visible(false);
    // Screenshot runs happen out of sight: never shown at all, so they can't take focus from
    // whatever's on the screen, like a game in fullscreen, which would minimize it. Windows
    // can show a window the first time in whatever way it was started instead of the way it
    // asks, so not taking focus isn't enough on its own.
    if out_of_sight {
        attributes = attributes.with_active(false).with_position(winit::dpi::PhysicalPosition::new(-30000, -30000));
        #[cfg(windows)]
        {
            use winit::platform::windows::WindowAttributesExtWindows;
            attributes = attributes.with_skip_taskbar(true);
        }
    }
    attributes
}

impl App {
    /// Takes over the window the loading screen was drawn in, see loading.rs
    fn attach(&mut self, screen: loading::Screen, event_loop: &ActiveEventLoop) {
        let loading::Screen { window, swapchain, image_views, gui, previous_frame_end, recreate_swapchain, presented } = screen;
        // A window that's never shown is never asked to redraw, so nothing comes along to wake the
        // loop for the next frame in screenshot runs
        event_loop.set_control_flow(if self.capture.is_some() { winit::event_loop::ControlFlow::Poll } else { winit::event_loop::ControlFlow::Wait });
        // The window may have been given the focus while loading, and the game's input only takes
        // any once it's heard it has it
        if window.has_focus() {
            self.input.process_window_event(&WindowEvent::Focused(true));
        }

        // Never nothing, should the window have been minimized while loading: they're all made
        // again once it's back
        let window_extent: [u32; 2] = <[u32; 2]>::from(window.inner_size()).map(|n| n.max(1));
        // As much as the GPU's memory has room for
        let room = self.render_room(0);
        let scale = room.map_or(self.render_scale, |room| fitting_scale(self.render_scale, window_extent, room));
        let render_extent = render_extent(window_extent, scale);
        if self.capture.is_some() {
            println!("Render images made at {}x{}, scale {scale:.2} of {:.2}, with room for {:.0} MB", render_extent[0], render_extent[1], self.render_scale, room.unwrap_or(0) as f64 / perf::MB);
        }
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

        self.rcx = Some(RenderContext {
            window,
            swapchain,
            image_views,

            images,

            gui,

            recreate_swapchain,
            recreate_images: false,
            render_scale_in_use: scale,
            render_scale_wanted: self.render_scale,
            other_images: None,

            previous_frame_end,
            in_flight: VecDeque::new(),
            presented,
        });
    }
}

impl ApplicationHandler for App {
    // The window's made for the loading screen, and handed over once the game's loaded, see attach()
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

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
        let start = Instant::now();
        self.update(event_loop);
        self.slow("updating the game", start);
        // A window that's never shown is never asked to redraw, so screenshot runs draw straight away
        if self.capture.is_some() {
            self.render(event_loop);
        } else {
            self.rcx.as_mut().unwrap().window.request_redraw();
        }
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
    // Alone on the screen, as when there was no window yet for it to go over
    loading::put_away();
    show_error(text);
    background_printing::finish();
    std::process::exit(1)
}

/// Screenshot runs print what they find as they go, to a file. Writing to it can hold a frame up
/// for a long while when the disk's busy, which isn't the game's doing and never happens in play,
/// where there's nowhere to print to. So what they print goes through a pipe to a thread that
/// writes it on, and the frames carry on meanwhile.
#[cfg(windows)]
mod background_printing {
    use std::ffi::c_void;
    use std::sync::Mutex;
    use std::thread::JoinHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
        fn CreatePipe(read: *mut *mut c_void, write: *mut *mut c_void, attributes: *mut c_void, size: u32) -> i32;
        fn ReadFile(file: *mut c_void, buffer: *mut u8, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
        fn WriteFile(file: *mut c_void, buffer: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn PeekNamedPipe(pipe: *mut c_void, buffer: *mut c_void, len: u32, read: *mut u32, available: *mut u32, left: *mut u32) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    /// Room in the pipe for what's printed while the file can't keep up
    const PIPE_BYTES: u32 = 8 << 20;

    /// The file's handle and the pipe's ends, as numbers so they can be shared, and the thread
    /// writing to the file
    struct Printing {
        file: usize,
        write: usize,
        read: usize,
        writer: JoinHandle<()>,
    }
    static STATE: Mutex<Option<Printing>> = Mutex::new(None);

    /// From here on, what's printed goes to the file through the pipe
    pub fn start() {
        let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if state.is_some() {
            return;
        }
        let file = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        // Nowhere to print to, as in play
        if file.is_null() || file as isize == -1 {
            return;
        }
        let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
        if unsafe { CreatePipe(&mut read, &mut write, std::ptr::null_mut(), PIPE_BYTES) } == 0 {
            return;
        }
        let (file, read) = (file as usize, read as usize);
        let printing = std::io::stdout().lock();
        let writer = std::thread::spawn(move || {
            let mut buffer = vec![0u8; 1 << 16];
            loop {
                let mut got = 0u32;
                if unsafe { ReadFile(read as *mut c_void, buffer.as_mut_ptr(), buffer.len() as u32, &mut got, std::ptr::null_mut()) } == 0 || got == 0 {
                    break;
                }
                let mut sent = 0;
                while sent < got {
                    let mut wrote = 0u32;
                    if unsafe { WriteFile(file as *mut c_void, buffer[sent as usize..].as_ptr(), got - sent, &mut wrote, std::ptr::null_mut()) } == 0 {
                        break;
                    }
                    sent += wrote;
                }
            }
            unsafe { CloseHandle(read as *mut c_void) };
        });
        unsafe { SetStdHandle(STD_OUTPUT_HANDLE, write) };
        drop(printing);
        *state = Some(Printing { file, write: write as usize, read, writer });
    }

    /// Puts printing back the way it was and waits for everything printed to reach the file
    pub fn finish() {
        use std::io::Write;
        // Holding on to stdout, so nothing's being printed to the pipe as it's closed
        let stdout = std::io::stdout();
        let mut printing = stdout.lock();
        let _ = printing.flush();
        let Some(Printing { file, write, writer, .. }) = STATE.lock().unwrap_or_else(|e| e.into_inner()).take() else {
            return;
        };
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, file as *mut c_void);
            CloseHandle(write as *mut c_void);
        }
        drop(printing);
        let _ = writer.join();
    }

    /// Gives what's been printed a moment to reach the file, without taking hold of stdout, which
    /// whatever panicked may have been in the middle of printing to
    pub fn drain() {
        let Some(read) = STATE.try_lock().ok().and_then(|state| state.as_ref().map(|p| p.read)) else {
            return;
        };
        for _ in 0..100 {
            let mut waiting = 0u32;
            let peeked = unsafe { PeekNamedPipe(read as *mut c_void, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut waiting, std::ptr::null_mut()) };
            if peeked == 0 || waiting == 0 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // The last of it on its way to the file
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(not(windows))]
mod background_printing {
    pub fn start() {}
    pub fn finish() {}
    pub fn drain() {}
}

/// What most often fixes it when the graphics card can't be used
const DRIVER_HINT: &str = "Innert needs a graphics card that supports Vulkan 1.3, which most made in the last several \
    years do. Installing the latest driver from NVIDIA, AMD or Intel usually sorts it out.";

/// Counts how much is allocated, for what's printed about it in screenshot runs, see perf.rs
#[global_allocator]
static ALLOCATOR: perf::Counting = perf::Counting;

fn main() {
    if std::env::args().any(|a| a == "--screenshot") {
        perf::count_heap();
    }
    startup::init();
    // `--set vram_budget=2048` takes the graphics card to have no more memory for the game than
    // that, in MB, as a smaller card would
    if let Some(megabytes) = settings_from_args().iter().find(|(k, _)| k == "vram_budget").and_then(|(_, v)| v.parse().ok()) {
        perf::cap_vram(megabytes);
    }
    // Decoding the sounds on a thread of their own, done long before anything's heard
    audio::preload();
    std::panic::set_hook(Box::new(|info| {
        let place = info.location().map_or(String::new(), |l| format!(" ({}:{})", l.file(), l.line()));
        let what = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        // Which thread, and with RUST_BACKTRACE set where it was, for screenshot runs' logs
        let thread = std::thread::current().name().map_or(String::new(), |n| format!(" on the {n} thread"));
        let trace = std::backtrace::Backtrace::capture();
        let trace = if trace.status() == std::backtrace::BacktraceStatus::Captured { format!("\n\n{trace}") } else { String::new() };
        show_error(&format!("Innert ran into a problem and has to close.\n\n{what}{place}{thread}{trace}"));
        background_printing::drain();
    }));
    if std::env::args().any(|a| a == "--screenshot") {
        background_printing::start();
    }
    startup::moment("Started running");
    let event_loop = EventLoop::new().unwrap_or_else(|e| fail(&format!("Couldn't open a window: {e}")));
    // The window's up with the loading screen in it straight away, while the game's made on
    // other threads, see loading.rs
    let mut launcher = loading::Launcher::start(&event_loop);
    event_loop.run_app(&mut launcher).unwrap();
    let loaded = launcher.finish();
    background_printing::finish();
    // What went wrong has been said already
    if !loaded {
        std::process::exit(101);
    }
}
