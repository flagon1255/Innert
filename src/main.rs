//! Innert: the game, src\bin\innert.rs, and its model editor, src\bin\innert-editor.rs, are both
//! this, started by main() or editor_main().

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
mod anim;
mod assets;
mod audio;
mod avians;
mod baked;
mod bases;
mod bindings;
mod building;
mod bosses;
mod camera;
mod character;
mod combat;
mod compass;
mod crash;
mod daynight;
mod demis;
mod dragons;
mod editor;
mod felling;
mod effects;
mod game;
mod gargoyles;
mod gates;
mod gather;
mod gpu_timing;
mod hearths;
mod hot_reload;
mod jewels;
mod leaves;
mod levels;
mod serpent_lairs;
mod serpent_trials;
mod serpents;
mod limits;
mod loading;
mod materials;
mod memory;
mod merchants;
mod night;
mod perf;
mod pieces;
mod player;
mod quality;
mod raids;
mod realms;
mod rig;
mod rope;
mod save;
mod session;
mod skills;
mod shafts;
mod skips;
mod staging;
mod startup;
mod stream;
mod theme;
mod tools;
mod voxelize;
mod vox;
mod water;
mod weapon_parts;
mod weapons;
mod wings;
mod worldgen;
mod guardians;
/// Fish and sharks as the app has them, see seas.rs
mod seas;

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
// Frames a pixel keeps averaging as the sun moves on a step with the day, see daynight.rs: the light
// changes only a little, so what was there is let go of over a few frames
const SUN_STEP_HISTORY: u32 = 6;

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
    /// The model editor's: a plain floor and nothing else, see editor.rs
    Studio,
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
            Scene::Studio => "Studio".to_string(),
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

/// The flames and crystals to light the world with this frame, of `lights`, in voxels as App::lights
/// has them, for the eye at `eye`: those whose light reaches nearest it, the most STILL_LIGHTS, each
/// fading out as it comes to be the next left out, or to LIGHT_MARGIN_M past where its light
/// reaches, so walking along a cave lit by many torches they come and go smoothly
fn still_lights(lights: impl Iterator<Item = [f32; 8]>, eye: Vector3<f32>, vpm: f32) -> Vec<[f32; 8]> {
    let (margin, fade) = (LIGHT_MARGIN_M * vpm, LIGHT_FADE_M * vpm);
    let mut near: Vec<(f32, [f32; 8])> = lights.map(|l| ((Vector3::new(l[0], l[1], l[2]) - eye).norm() - l[3], l)).filter(|(r, _)| *r < margin).collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Where the first left out is, which moves smoothly as the eye does
    let cutoff = near.get(STILL_LIGHTS).map_or(margin, |(r, _)| r.min(margin));
    near.truncate(STILL_LIGHTS);
    near.into_iter()
        .filter_map(|(r, mut l)| {
            let t = ((r - (cutoff - fade)) / fade).clamp(0.0, 1.0);
            let k = 1.0 - t * t * (3.0 - 2.0 * t);
            for c in &mut l[4..7] {
                *c *= k;
            }
            (k > 0.0).then_some(l)
        })
        .collect()
}

/// Light `light`, as App::lights has it, flickering and swaying a little at `seconds` if it's a
/// flame, going by its warm color: a torch's or a fire's most, a candle's less and a lantern's,
/// yellower behind its glass, least. Crystals and glows shine steady.
fn flicker(light: [f32; 8], seconds: f32, vpm: f32) -> [f32; 8] {
    let [r, g, b] = [light[4], light[5], light[6]];
    if r <= 0.0 || r < g || g <= b {
        return light;
    }
    let amount = ((0.78 - g / r) * 3.5).clamp(0.0, 1.0);
    if amount <= 0.0 {
        return light;
    }
    // Each its own way, so they don't all flicker together
    let s = (light[0] * 0.37 + light[1] * 0.71 + light[2] * 0.13).fract() * 40.0;
    let t = seconds;
    let k = 1.0 + amount * (0.09 * (t * 7.3 + s).sin() + 0.06 * (t * 13.1 + 2.3 * s).sin() + 0.04 * (t * 23.7 + 1.7 * s).sin());
    let sway = amount * 0.03 * vpm;
    let mut out = light;
    out[0] += sway * (t * 1.9 + s).sin();
    out[1] += sway * (t * 2.3 + 0.5 * s).cos();
    for c in &mut out[4..7] {
        *c *= k;
    }
    out
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
        Scene::Studio => (World::studio(editor::STUDIO_SIZE, worldgen::material::STONE, worldgen::material::GRAVEL, terrain.voxels_per_meter), None),
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
/// The most flames and crystals lighting things up at once, nearest the camera first, see
/// still_lights(): a share of their own, so the fighting never takes one of theirs away and makes
/// it go out
const STILL_LIGHTS: usize = 14;
/// And the most lights from the fighting, see Game::lights(), the nearest and brightest first
const FIGHTING_LIGHTS: usize = 10;
/// How far past where its light reaches a flame or crystal is still sent, in meters, so a pool
/// of light down a cave or a street shows from a way off; and over how many meters before that,
/// or before where the farthest of STILL_LIGHTS sent is, each fades out, so none pops in or out
const LIGHT_MARGIN_M: f32 = 12.0;
const LIGHT_FADE_M: f32 = 4.0;
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
    /// The day going round, see daynight::Sky: towards the sun, its light and how much the stars
    /// show, towards the moon and how bright it is, the light the world's lit by, and what the light
    /// cache keeps light relative to. Only traverse.comp reads these.
    sun: [f32; 4],
    sun_light: [f32; 4],
    moon: [f32; 4],
    key_light: [f32; 4],
    cache_level: [f32; 4],
    /// The look of the realm the world is, see App::realm_look(), and the colours its sky's tinted
    /// towards, straight up and round the horizon, with how far, see sky_tint()
    realm: [f32; 4],
    realm_sky: [[f32; 4]; 2],
}

impl View {
    fn new(pixel_to_ray: Matrix4<f32>, previous_world_to_pixel: Matrix4<f32>, sky: &daynight::Sky, realm: [f32; 4]) -> Self {
        let v = |v: Vector3<f64>, w: f64| [v.x as f32, v.y as f32, v.z as f32, w as f32];
        let c = |c: [f64; 3], w: f64| [c[0] as f32, c[1] as f32, c[2] as f32, w as f32];
        View {
            pixel_to_ray,
            previous_world_to_pixel,
            sun: v(sky.sun, 0.0),
            sun_light: c(sky.sun_light, sky.stars),
            moon: v(sky.moon, sky.moon_shine),
            key_light: c(sky.key_light, sky.moon_phase),
            cache_level: c(sky.cache_level, 0.0),
            realm,
            realm_sky: sky_tint(realm),
        }
    }
}

/// The colours the sky of the realm with look `realm` is tinted towards straight up and round the
/// horizon, and how far, see App::realm_look(): the dreaming realm's lilac over a rosy horizon, deep
/// red over its bleeding half, the frozen realm's pale and cold, the sunken reach's clear, the
/// Cinder Crown's smoky over embers, and no tint for the normal world's and the sky isles'. Worked
/// out here once a frame rather than for every ray that looks at the sky in traverse.comp.
fn sky_tint(realm: [f32; 4]) -> [[f32; 4]; 2] {
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
    let with = |c: [f32; 3], w: f32| [c[0], c[1], c[2], w];
    let blood = realm[3];
    match realm[0].round() as u32 {
        2 => [with(mix([0.42, 0.3, 0.85], [0.45, 0.04, 0.05], blood), 0.85), with(mix([1.0, 0.68, 0.8], [0.95, 0.28, 0.16], blood), 0.85)],
        3 => [with([0.32, 0.5, 0.82], 0.6), with([0.82, 0.9, 1.0], 0.6)],
        // The sunken reach's a clear sea blue over a pale green-blue horizon
        4 => [with([0.06, 0.4, 0.82], 0.4), with([0.6, 0.88, 0.9], 0.45)],
        // The Cinder Crown's dark and smoky over an ember horizon
        5 => [with([0.36, 0.15, 0.1], 0.9), with([0.98, 0.5, 0.24], 0.85)],
        _ => [[0.0; 4]; 2],
    }
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
    /// How much brighter it's shown, and how much of the color goes at night, see daynight::Sky
    exposure: f32,
    night: f32,
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
        STONE | ROCK | GRAVEL | CLAY | CLAY_BANK | TEMPLE_STONE | TEMPLE_DARK | TEMPLE_RED | TEMPLE_TEAL | BRICK_STONE | MORTAR | LIMESTONE | WOOD | DEAD_WOOD | CLAY_POT => audio::Ground::Stone,
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
    // What else the set binds, to bind again when there's more room for bricks, see
    // grow_with_frame()
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

    /// Makes room for at least `needed` bricks, and `spare` more: a bigger buffer, the bricks there
    /// are copied into on the GPU, which is far quicker than sending the world over again, with
    /// the next frame, see copy_over(), so no frame waits for it: drawn with the new buffer from
    /// then on, the old one goes once the frames before are done with it
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
    /// since the last, before anything else's sent into it. Nothing's ever sent into them outside
    /// a frame, see App::flush_edits().
    fn copy_over(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        if let Some(old) = self.copy_from.take() {
            let len = old.len().min(self.bricks.len());
            builder.copy_buffer(CopyBufferInfo::buffers(old.slice(0..len), self.bricks.clone().slice(0..len))).unwrap();
        }
    }
}

/// Holes blown in the world and stains splashed on it, as they're to go over to the GPU with the
/// next frame, see App::flush_edits() and App::record_edits(): host memory of their own, held by
/// that frame's commands till it's done, so nothing waits and nothing's written while it's read
struct EditUpload {
    /// The cells of the brick grid that changed, then their bricks
    words: Subbuffer<[u32]>,
    grid: Vec<BufferImageCopy>,
    bricks: Vec<BufferCopy>,
    /// The ground the water lies on where it changed, and where the water was laid on it, a piece
    /// of each for each part of the room it's in
    #[allow(clippy::type_complexity)]
    ground: Vec<((Subbuffer<[u32]>, BufferImageCopy), (Subbuffer<[u32]>, BufferImageCopy))>,
}

/// Host memory for `words`, to be copied from with a frame
fn host_words(memory_allocator: &Arc<StandardMemoryAllocator>, words: Vec<u32>) -> Subbuffer<[u32]> {
    Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo { usage: BufferUsage::TRANSFER_SRC, ..Default::default() },
        AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..Default::default() },
        words,
    )
    .unwrap()
}

/// Seconds a frame spends blowing holes in the world at the most, past the first, see
/// App::pending_craters: a burst of them, as a cluster bomb's, goes over the next few frames
const CARVE_SECONDS: f64 = 0.004;

/// How much water something knocks down going into it, in cubic meters, for a splash of size 1,
/// see App::ripple(), and how long till it's let back, wider, so the dip rolls on out as a wave
const RIPPLE_VOLUME: f64 = 0.45;
const RIPPLE_REFILL_SECONDS: f64 = 0.15;

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
            // Copied from when there's to be more room for bricks, see GpuWorld::grow_with_frame()
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
    /// Makes empty bricks round what's just been built skip no further than it, see building.rs
    builds_pipeline: HotReloadComputePipeline,
    /// What's been built and dug, and what's carried to build with, see bases.rs, and the building
    /// and gathering going on, see building.rs
    bases: bases::Bases,
    building: building::Building,
    /// The night's raid, see night.rs
    raid: night::Raid,

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
    /// The day going round, see daynight.rs, unless the sun's held where `sun_azimuth` and
    /// `sun_elevation` put it, as `--set sun_azimuth=` and the like do and the settings can
    cycle: daynight::DayNight,
    fixed_sun: bool,
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
    /// Model files in assets\models read again as they're saved, see assets.rs
    model_files: assets::Reloader<Subbuffer<[u32]>>,
    /// The model editor, in its own app, see editor.rs
    editor: editor::Editor,
    weapon_models: Subbuffer<[u32]>,
    // Which of Game::models_version() the weapons' are
    models_version: u32,
    // Where the animals are this frame gets uploaded from here
    instance_allocator: SubbufferAllocator,
    // Light bounced onto each voxel face, gathered over frames, see LightCache in traverse.comp
    light_cache: Subbuffer<[u32]>,
    // What the light in it was gathered for. When that changes it's thrown away.
    light_cache_for: Option<(u64, u32)>,
    /// Which sun the frames were last drawn under, see sky_epoch in render(): a step of the sun
    /// with the day keeps it
    sky_epoch: u64,
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
    /// What flush_edits() made of them, to go over with the next frame, see record_edits()
    edit_uploads: Vec<EditUpload>,
    /// Holes still to be blown, a few a frame, see CARVE_SECONDS: where, in voxels of the whole
    /// landscape, and how big, in meters
    pending_craters: VecDeque<(Vector3<f64>, f64)>,
    /// Water still to be knocked down or let back where things went into it, see ripple(): in how
    /// many seconds, where in voxels of the whole landscape, how far round in voxels, and how many
    /// cubic voxels, taken out if negative. One a frame, as the water takes one brush a frame.
    pending_ripples: Vec<(f64, Vector3<f64>, f64, f64)>,
    /// Stains still to be splashed, once the holes before them are blown, the same way
    pending_stains: Vec<(Vector3<f64>, f64, u8, bool)>,
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
    /// The land the loading screen showed, behind the menu until a game's started, answering the
    /// mouse, see loading::Backdrop
    backdrop: Option<loading::Backdrop>,
    /// Whether the world's the empty one Innert starts on, with nothing built in it yet, see
    /// empty_world(): not drawn behind the menu, and built afresh when a game's begun
    bare: bool,
    /// The mode being played, Free Roam or Realms, and its save, see session.rs
    session: session::Session,
    /// The seed a new Free Roam game's landscape is to have, picked in the menu
    new_seed: u32,
    /// A grappling hook thrown, and the winds bringing the player back after a fall below the sky
    /// isles, see rope.rs
    hook: Option<rope::Hook>,
    fall_back: Option<rope::FallBack>,
    /// The map on M, see compass.rs
    map: compass::Map,
    /// How the wings worn are moving, see wings.rs
    wings: wings::Flight,
    /// The wards round raid bosses breaking, see guardians.rs
    wards: guardians::Wards,
    show_settings: bool,
    /// The keys every action's on, and their Controls window, see bindings.rs
    keys: bindings::Keys,
    inventory_open: bool,
    /// The skill web's window is open, over everything, the world held still behind it, see skills.rs
    skills_open: bool,
    /// Its window closed from within, for the mouse to be taken back next frame
    skills_closed: bool,
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
    /// The frame times said at the end are those from this frame on: from when the world's there
    /// to play in, with `--set time_from_ready=1`, rather than while it's still being built
    stats_from: usize,
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
            stats_from: 0,
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
    builds: HotReloadComputePipeline,
}

impl Pipelines {
    // Read from where they are while working on them, so they reload when changed. Copies are
    // built in for running elsewhere.
    const SHADERS: [(&str, &str); 8] = [
        ("traverse.comp", include_str!("../shaders/traverse.comp")),
        ("post.comp", include_str!("../shaders/post.comp")),
        ("resample.comp", include_str!("../shaders/resample.comp")),
        ("upscale.comp", include_str!("../shaders/upscale.comp")),
        ("water.comp", include_str!("../shaders/water.comp")),
        ("shafts.comp", include_str!("../shaders/shafts.comp")),
        ("skips.comp", include_str!("../shaders/skips.comp")),
        ("builds.comp", include_str!("../shaders/builds.comp")),
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
        let [render, post, resample, upscale, water, shafts, skips, builds] = making;
        Some(Pipelines {
            render: render.join().ok()?,
            post: post.join().ok()?,
            resample: resample.join().ok()?,
            upscale: upscale.join().ok()?,
            water: water.join().ok()?,
            shafts: shafts.join().ok()?,
            skips: skips.join().ok()?,
            builds: builds.join().ok()?,
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
    /// Whether it's the empty world Innert starts its menu on, see empty_world()
    bare: bool,
}

impl Placed {
    fn place(progress: &Arc<loading::Progress>) -> Self {
        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());

        // The model editor's own app opens on a plain floor rather than the landscape
        let scene = if editor::is_app() { Scene::Studio } else { Scene::Terrain };
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
        // `--set dragon_lair=1` starts at the Worldpyre Ancient's far lair, see dragons.rs, looking
        // in at it from out over its rim
        let dragon_lair = setting("dragon_lair").and_then(|_| worldgen::far_lair(&terrain));
        // `--set dragon_lairs=1` builds the three great dragons' lairs round where the game starts,
        // as the Dragon realm has them, for `--set spawn=dragon:0` and so on to wait in
        if setting("dragon_lairs").is_some() {
            let [x, y] = worldgen::FREE_ROAM_START;
            let lairs: Vec<(worldgen::LairKind, [f32; 2])> = [(worldgen::LairKind::Peak, [x - 55.0, y + 45.0]), (worldgen::LairKind::Caldera, [x + 62.0, y + 30.0]), (worldgen::LairKind::Hoard, [x + 5.0, y - 64.0])]
                .into_iter()
                .map(|(kind, at)| (kind, worldgen::dry_land_near(&terrain, at, 80.0, 10.0, &|_| true).unwrap_or(at)))
                .collect();
            worldgen::set_lairs(terrain.seed, lairs);
        }
        // `--set crevice=edge` starts on the rim of the nearest open ravine beside the hollow at its
        // bottom, looking down into it; `crevice=bottom` down on its floor looking across the
        // hollow at what's there, `crevice=ramp` at the top of the way down looking down it,
        // `crevice=air` high over it, `crevice=demi` and `crevice=fight` on the floor of the nearest
        // with a demi-boss in it, the latter near enough for it to come for the player, and
        // `crevice=built` high over the nearest thing built near the edge of one. With
        // `--set hold=bones`, `crystals`, `fungus`, `nest` or `cache`, the nearest with that down in it.
        let crevice_view = setting("crevice");
        // `--set demi=archer`, `brute`, `ghoul` or `spider`: every demi-boss that kind, see demis.rs
        if let Some(kind) = setting("demi")
            && !demis::force(kind)
        {
            println!("No demi-boss grows from a {kind}");
        }
        // Near where a new game sets out, so with a mode too the world needn't move on from it
        let near_start = {
            let widest = memory::WIDTHS_M[0] * terrain.voxels_per_meter;
            let at = camera_to_voxels(scene.initial_camera().position, [widest, widest, terrain.size()[2]]) / terrain.voxels_per_meter as f64;
            [at.x as f32, at.y as f32]
        };
        let hold = setting("hold").and_then(|h| worldgen::Hold::ALL.into_iter().find(|k| format!("{k:?}").eq_ignore_ascii_case(h)));
        let chasm_built = crevice_view.filter(|v| *v == "built").and_then(|_| worldgen::built_by_chasm(&terrain, near_start, 14.0, &rivers));
        let chasm = match crevice_view {
            Some("built") => chasm_built.map(|(_, _, c)| c),
            Some("demi" | "fight") => worldgen::nearest_chasm(&terrain, near_start, Some(worldgen::Hold::Lurker)),
            Some(_) => worldgen::nearest_chasm(&terrain, near_start, hold),
            None => None,
        };
        if let Some(c) = &chasm {
            println!("Crevice: {:?} down in it, its floor at {:.0}, {:.0}, {:.1} m under its rim", c.hold, c.floor[0], c.floor[1], c.rim[2] - c.floor[2]);
        }
        if let Some((kind, at, _)) = chasm_built {
            println!("Built by a crevice: a {kind} at {:.0}, {:.0}", at[0], at[1]);
        }
        let chasm_spot = chasm.map(|c| match chasm_built {
            Some((_, at, _)) => [(at[0] + c.rim[0]) * 0.5, (at[1] + c.rim[1]) * 0.5],
            // Between it and where a new game sets out, if they're near enough for both to be in
            // the world, so a game started with `--set mode=` needn't move it on
            None if (c.floor[0] - near_start[0]).hypot(c.floor[1] - near_start[1]) < 110.0 => [(c.floor[0] + near_start[0]) * 0.5, (c.floor[1] + near_start[1]) * 0.5],
            None => [c.floor[0], c.floor[1]],
        });
        // For trying out each kind of land: `--set biome=jungle` starts in the nearest jungle
        let start_in = dragon_lair
            .or(chasm_spot)
            .or(camp_at)
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
            // `--set at=x,y` starts over that spot of the landscape, in meters
            .or_else(|| {
                let at: Vec<f32> = setting("at")?.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                (at.len() == 2).then(|| [at[0], at[1]])
            })
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
        // The land round there's shown rising on the loading screen as it's built
        if scene.is_terrain() {
            let vpm = terrain.voxels_per_meter as f32;
            progress.show_land(terrain, start_in.unwrap_or([start_at.x as f32 / vpm, start_at.y as f32 / vpm]));
        }
        // Innert starts on its menu with no world built, only the map of the one about to be
        // entered beside it: the world's built once a game's begun, behind the loading screen, see
        // App::begin(). Screenshot runs go straight into the world as they always have, as does
        // one that starts a game straight away with `--set mode=`, and `--set world_at_start=1`
        // builds it as before behind the menu.
        let bare = scene.is_terrain()
            && setting("menu").map_or(Capture::from_args().is_none() || loading::previewing(), |v| v == "1" || v == "true")
            && setting("mode").is_none()
            && setting("world_at_start").is_none_or(|v| v != "1" && v != "true");
        let (world, landscape) = if bare {
            progress.world_built();
            (empty_world(&terrain, origin, &rivers), None)
        } else {
            progress.building_world(|| build_world(scene, &terrain, INITIAL_VOXEL_RESOLUTION, origin, &rivers))
        };
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
            // `--set cave=passage` stands a little way down the tunnel from a torch on its wall, the
            // nearest more than 15 m out from the cavern, looking back along the tunnel at it
            let torch = landscape.as_ref().and_then(|l| {
                let off = |p: [f32; 3]| (p[0] - floor[0]).hypot(p[1] - floor[1]);
                l.lights()
                    .iter()
                    .filter(|t| t.color == worldgen::TORCH_LIGHT && off(t.position) > 15.0 && t.position[2] < floor[2] + 12.0)
                    .min_by(|a, b| off(a.position).total_cmp(&off(b.position)))
                    .map(|t| Vector3::new(t.position[0] as f64 * vpm - origin[0] as f64, t.position[1] as f64 * vpm - origin[1] as f64, t.position[2] as f64 * vpm))
            });
            if let (Some("passage"), Some(torch)) = (setting("cave"), torch) {
                // Which way the tunnel runs from it: the way with the most room, at head height
                let low = torch - Vector3::new(0.0, 0.0, 0.8 * vpm);
                let room = |d: Vector3<f64>| (1..=(12.0 * vpm) as i64).take_while(|&k| worldgen::is_passable(world.material_at((low.x + d.x * k as f64) as i64, (low.y + d.y * k as f64) as i64, low.z as i64))).count();
                let way = (0..32).map(|k| k as f64 / 32.0 * std::f64::consts::TAU).map(|a| Vector3::new(a.cos(), a.sin(), 0.0)).max_by_key(|&d| room(d)).unwrap_or(Vector3::x());
                let eye = low + way * (room(way) as f64).min(7.0 * vpm) * 0.85;
                camera.position = voxels_to_camera(eye, world.size);
                let target = voxels_to_camera(torch - way * 3.0 * vpm - Vector3::new(0.0, 0.0, 0.6 * vpm), world.size);
                camera.look_at(target);
                // The same view for a build without this, as `--set camera=` and `look_at=`
                let whole = voxels_to_camera(eye + Vector3::new(origin[0] as f64, origin[1] as f64, 0.0), [widest, widest, terrain.size()[2]]);
                println!("Passage view: --set camera={},{},{} --set look_at={},{},{}", whole.x, whole.y, whole.z, target.x, target.y, target.z);
            }
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
            // `town=bowyer` and the like in front of that specialist's stall, see merchants.rs
            let wanted = town_view.and_then(|v| merchants::Trade::by_id(v)).map_or("town stall", |t| t.stall_site());
            let specialist = town_view == Some("specialist");
            if let Some(l) = &landscape {
                let stalls: Vec<String> = l.sites().iter().filter(|s| s.kind.ends_with("stall")).map(|s| format!("{} at {:.0}, {:.0}", s.kind, s.position[0], s.position[1])).collect();
                println!("Stalls about: {}", stalls.join("; "));
            }
            let stall = landscape.as_ref().and_then(|l| {
                let sites = l.sites().iter().filter(|s| s.kind == wanted || specialist && merchants::Trade::of_site(s.kind).is_some());
                sites.map(|s| (at(s.position), Vector3::new(s.facing[0] as f64, s.facing[1] as f64, 0.0))).min_by(|a, b| (a.0 - square).norm().total_cmp(&(b.0 - square).norm()))
            });
            let (eye, target) = match (town_view, stall) {
                (Some("far"), _) => (way_in + up(5.0), square + up(1.5)),
                (Some("tunnel"), _) => (way_in + out * 7.0 * vpm + up(1.7), way_in + up(1.4)),
                // Where a customer would stand, the merchant behind the counter
                (Some("stall"), Some((spot, facing))) => (spot + facing * 3.2 * vpm + up(1.6), spot + up(1.2)),
                (Some(v), Some((spot, facing))) if v == "specialist" || merchants::Trade::by_id(v).is_some() => (spot + facing * 4.6 * vpm + up(1.9), spot + up(1.5)),
                // A little way along a street, looking down it across the square
                _ => (square + out * 10.0 * vpm + up(1.7), square - out * 8.0 * vpm + up(1.2)),
            };
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(target, world.size));
        } else if let Some(lair) = dragon_lair {
            let vpm = world.voxels_per_meter as f64;
            let middle = Vector3::new(lair[0] as f64 * vpm - origin[0] as f64, lair[1] as f64 * vpm - origin[1] as f64, 0.0);
            let ground = |p: Vector3<f64>| (0..world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(world.material_at(p.x as i64, p.y as i64, z))).unwrap_or(0) as f64;
            // From out past the way into its arena
            let way = worldgen::dragon_arena_way(&terrain, lair) as f64;
            let back = Vector3::new(way.cos(), way.sin(), 0.0);
            let out = setting("dragon_lair_out").and_then(|v| v.parse::<f64>().ok()).unwrap_or(48.0);
            let eye = middle + back * out * vpm;
            let eye = Vector3::new(eye.x, eye.y, ground(eye) + setting("rise").and_then(|v| v.parse::<f64>().ok()).unwrap_or(16.0) * vpm);
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(Vector3::new(middle.x, middle.y, ground(middle) + 3.0 * vpm), world.size));
        } else if let Some(c) = &chasm {
            let vpm = world.voxels_per_meter as f64;
            let at = |p: [f32; 3]| Vector3::new(p[0] as f64 * vpm - origin[0] as f64, p[1] as f64 * vpm - origin[1] as f64, p[2] as f64 * vpm);
            let up = |m: f64| Vector3::new(0.0, 0.0, m * vpm);
            let flat = |d: [f32; 2]| Vector3::new(d[0] as f64, d[1] as f64, 0.0);
            let (across, along) = (flat(c.across), flat(c.along));
            let (eye, target) = match crevice_view {
                // Over the middle of it a little above its rim, looking down along it to the floor
                Some("edge") => {
                    let eye = at(c.floor) - along * 12.0 * vpm;
                    (Vector3::new(eye.x, eye.y, at(c.rim).z + 4.0 * vpm), at(c.floor) + along * 3.0 * vpm)
                }
                Some("ramp") => (at(c.top) + up(1.7), at(c.foot) + up(1.2)),
                Some("air") => (at(c.rim) - across * 14.0 * vpm + up(26.0), at(c.floor)),
                Some("built") => {
                    let built = chasm_built.map_or(c.rim, |(_, b, _)| b);
                    let middle = (at(built) + at(c.rim)) * 0.5;
                    let side = Vector3::new(-(at(built) - at(c.rim)).y, (at(built) - at(c.rim)).x, 0.0).try_normalize(1e-6).unwrap_or(Vector3::x());
                    (middle + side * 34.0 * vpm + up(30.0), middle)
                }
                // Back along the floor from the hollow, looking across it
                Some("demi") => (at(c.floor) - along * 11.0 * vpm + up(1.7), at(c.floor) + up(1.0)),
                Some("fight") => (at(c.floor) - along * 6.0 * vpm + up(1.7), at(c.floor) + up(1.0)),
                _ => (at(c.floor) - along * 6.5 * vpm + up(1.7), at(c.floor) + along * 2.0 * vpm + up(0.4)),
            };
            camera.position = voxels_to_camera(eye, world.size);
            camera.look_at(voxels_to_camera(target, world.size));
            // The same view for a build without this, as `--set camera=` and `look_at=`, in the world
            // that starts round that camera
            let shift = Vector3::new(origin[0] as f64, origin[1] as f64, 0.0);
            let e = voxels_to_camera(eye + shift, [widest, widest, terrain.size()[2]]);
            let there = [0, 1].map(|i| (((eye + shift)[i]) as i64 - terrain.size()[i] as i64 / 2 + strip / 2).div_euclid(strip) * strip);
            let t = voxels_to_camera(target + shift - Vector3::new(there[0] as f64, there[1] as f64, 0.0), world.size);
            println!("Crevice view: --set camera={},{},{} --set look_at={},{},{}", e.x, e.y, e.z, t.x, t.y, t.z);
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
        let down_in_chasm = chasm.is_some() && matches!(crevice_view, Some("bottom" | "demi" | "fight"));
        let indoors = dungeon.is_some() || cavern.is_some() || town.is_some() && underground || royal.is_some() && inside_castle || sunken.is_some() && in_city || down_in_chasm;
        // Into the world that moves on from here a strip at a time
        let (world, pieces, river_springs) = match &landscape {
            Some(landscape) => into_window(world, landscape),
            None => (world, Vec::new(), Vec::new()),
        };
        Placed { scene, terrain, rivers, world, pieces, river_springs, camera, indoors, memory, bare }
    }
}

/// A world with nothing in it yet, the size and at the place the landscape's would be, `origin` in
/// voxels from the corner of the whole landscape, for Innert to start on with no world built, see
/// Placed::place(): built afresh once a game's begun, at the level of the sea it would have
fn empty_world(terrain: &TerrainSettings, origin: [i64; 2], rivers: &RiverCache) -> World {
    // The sea's level as a world built there has it, from a few columns of one
    let (_, few) = worldgen::generate_area(terrain, origin, [8, 8], rivers);
    let strip = stream::STRIP_M * terrain.voxels_per_meter;
    World::window(terrain.size(), strip, origin, terrain.voxels_per_meter, few.sea_level)
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
        // The creatures are made meanwhile, on every core now the world's built, rather than
        // once the game's put together, which held the loading screen still for a second or more
        let creatures = std::thread::Builder::new().name("creatures".into()).spawn(animals::make_ahead).ok();

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
            if flag("water_report", false) {
                water::keep_report();
            }
            let mut water = Water::new(memory_allocator, command_buffer_allocator, descriptor_set_allocator, queue, &pipelines.water, world, &springs);
            water.active &= flag("water", true);
            water
        });

        note_vram(&gpu.device, "the water made");
        progress.run(Step::Creatures, || creatures.map(|c| c.join()));
        if progress.cancelled() {
            return None;
        }
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
        let Pipelines { render: render_pipeline, post: post_pipeline, resample: resample_pipeline, upscale: upscale_pipeline, water: water_pipeline, shafts: shafts_pipeline, skips: skips_pipeline, builds: builds_pipeline } = pipelines;
        let Placed { scene, terrain, rivers, mut world, pieces, river_springs, mut camera, indoors, memory, bare } = placed;
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
        if let Some(times) = setting("water_speed").and_then(|v| v.parse().ok()) {
            water::set_speed(times);
        }

        let input = WinitInputHelper::new();
        let flag = |key: &str, default: bool| setting(key).map_or(default, |v| v == "true" || v == "1");

        // Start on foot, except for screenshot runs which need the camera to stay put
        let capture = Capture::from_args();
        let water_level = |x: f64, y: f64| water.surface_at(x, y);

        // No one's stood in the empty world Innert starts on till a game's begun
        let walk = scene.is_terrain() && flag("walk", capture.is_none()) && !bare;
        // In a dungeon or a cave, right where the camera is rather than up on the ground over it
        let eye = camera_to_voxels(camera.position, world.size);
        let player = walk.then(|| if indoors { Player::new(eye, &world) } else { Player::on_dry_ground(eye, &world, &water_level) });

        let start = player.as_ref().map_or(camera_to_voxels(camera.position, world.size), |p| p.position);
        let mut game = Game::new(&world, &water_level, terrain.seed, start);
        // For trying out levels: `--set player_level=12` starts the player at that level, before
        // any monster's given its own, `level_up=1` has them go up one straight away, and
        // `level_up_hold=1` holds the moment of it on the HUD for a screenshot, see levels.rs
        if let Some(level) = setting("player_level").and_then(|v| v.parse().ok()) {
            game.set_level(level);
        }
        if let Some(share) = setting("player_xp").and_then(|v| v.parse::<f64>().ok()) {
            game.gain_experience(crate::levels::needed(game.level().0) * share.clamp(0.0, 0.99), None);
        }
        if flag("level_up", false) {
            game.level_up_now();
        }
        // `--set skill_points=30` takes that many nodes of the skill web, going up levels for them
        // if there aren't the points, and `--set skills=1` opens its window, `skill_zoom=0.4` that
        // close, `skill_hover=melee_n3` with the pointer over that node, for its card
        if let Some(n) = setting("skill_points").and_then(|v| v.parse::<usize>().ok()) {
            while (game.skill_points() as usize) < n && game.level().0 < crate::levels::LEVEL_CAP {
                game.level_up_now();
            }
            game.take_skills(n);
        }
        if let Some(zoom) = setting("skill_zoom").and_then(|v| v.parse::<f32>().ok()) {
            game.skill_view.zoom = zoom;
        }
        if let Some(id) = setting("skill_hover") {
            game.skill_view.hover = Some(id.to_owned());
        }
        // `--set inventory=1 inspect=2` opens the inventory with the card of its third slot showing
        if let Some(slot) = setting("inspect").and_then(|v| v.parse().ok()) {
            game.inspect(slot);
        }
        // And `card_hover=1` with the card of its second slot showing as it does with the pointer
        // over it, in the inventory or at a merchant's
        game.card_hover = setting("card_hover").and_then(|v| v.parse().ok());
        game::HOLD_LEVEL_UP.store(flag("level_up_hold", false), std::sync::atomic::Ordering::Relaxed);
        // `--set class=priest` and the rest without a mode: who's playing is that character, armed
        // as their class starts, their body there for its shadow, to try them out in screenshot
        // runs with `enemy=` and the rest, see character.rs. A mode started sets its own.
        if setting("class").is_some() && setting("mode").is_none() {
            game.set_character(&character::Character::from_settings());
            game.new_character(terrain.seed);
        }
        // For trying out weapons: `--set weapon_seed=7 weapon_luck=3` starts with other ones, rarer
        // the luckier, and `--set weapon=bow` with the bow in hand
        if let Some(seed) = setting("weapon_seed").and_then(|v| v.parse().ok()) {
            game.arm(seed, setting("weapon_luck").and_then(|v| v.parse().ok()).unwrap_or(1.0));
        }
        if let Some(family) = setting("weapon") {
            game.select_family(family);
        }
        // `--set blast_size=0.5` has every gun's shell burst that big next to the usual, for a
        // shot of a small, an ordinary or a big one, see combat::shell_size()
        if let Some(size) = setting("blast_size").and_then(|v| v.parse::<f64>().ok()) {
            combat::force_blast_size(size);
        }
        // `--set jewels=melee_rare:3,magic_common:1` gives those jewels, see jewels.rs, and
        // `socket=1` sets one of each into the first weapon carried it fits
        if let Some(list) = setting("jewels") {
            for part in list.split(',') {
                let (id, n) = part.split_once(':').map_or((part, 1), |(id, n)| (id, n.parse().unwrap_or(1)));
                game.give_jewels(id, n);
            }
        }
        if flag("socket", false) {
            game.socket_some();
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
        // Avians and gargoyles made from here on making only `--set boss_move=snatch` or another of
        // their own moves, in a screenshot run, see avians.rs
        avians::force(setting("boss_move").filter(|_| capture.is_some()).and_then(avians::Move::named));
        // `--set live_npcs=0` draws townsfolk, merchants, monsters and animals in their baked frames
        // however near, rather than posed afresh from their pieces, to compare, and `live_npcs=25`
        // poses only those within 25 meters
        baked::set_live(setting("live_npcs").and_then(|v| v.parse().ok()).unwrap_or(preset.live_npcs));
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
            // `--set boss_move=throw` has it make only that move, as soon as it's ready, wherever
            // the player is, for shots of it in a screenshot run
            if let Some(m) = setting("boss_move").filter(|_| capture.is_some()).and_then(bosses::Move::named) {
                game.force_boss_move(m);
            }
            // `--set boss_pit=3` blows a pit that many meters round under it, to see it bound out
            if let Some(radius) = setting("boss_pit").filter(|_| capture.is_some()).and_then(|v| v.parse::<f64>().ok()) {
                game.pit_bosses(radius);
            }
        }
        // `--set aerie_show=1` has avians and gargoyles fight with nobody about as if someone were,
        // and `aerie_trace=1` says what each is doing twice a second, see avians.rs
        avians::SHOW.store(flag("aerie_show", false), std::sync::atomic::Ordering::Relaxed);
        avians::TRACE.store(flag("aerie_trace", false), std::sync::atomic::Ordering::Relaxed);
        // Dragons, see dragons.rs: `--set spawn=dragon:0`, `1` or `2` puts the Dragon realm's raid
        // boss of that lair a little way ahead, `spawn=dragon:ashwing` or another drakeling a pair
        // of them, and `--set dragon_ultra=1` the Worldpyre Ancient, its finale
        if let Some(which) = setting("spawn").and_then(|v| v.strip_prefix("dragon:").map(str::to_owned)) {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            match which.parse::<usize>() {
                Ok(n) => {
                    let look = dragons::RAID_DRAGONS[n.min(2)];
                    // In its lair, if the lairs are built, see `--set dragon_lairs`
                    let kind = dragons::lair_kind(look);
                    match worldgen::lairs_set(terrain.seed).into_iter().find(|(k, _)| Some(*k) == kind) {
                        Some((_, lair)) => {
                            let vpm = world.voxels_per_meter as f64;
                            let at = Vector3::new(lair[0] as f64 * vpm - world.origin[0] as f64, lair[1] as f64 * vpm - world.origin[1] as f64, start.z);
                            game.summon(&world, &water_level, look, at, ahead, 0.0);
                        }
                        None => game.summon(&world, &water_level, look, start, ahead, setting("boss_distance").and_then(|v| v.parse().ok()).unwrap_or(32.0)),
                    }
                    println!("dragon: {}", look.name());
                }
                Err(_) => {
                    let look = dragons::DRAKELINGS.into_iter().find(|l| l.name().eq_ignore_ascii_case(&which)).unwrap_or(animals::Look::Ashwing);
                    let count = setting("enemy_count").and_then(|v| v.parse().ok()).unwrap_or(2);
                    game.summon_pack(&world, &water_level, &vec![look; count], start, ahead, setting("enemy_distance").and_then(|v| v.parse().ok()).unwrap_or(18.0), 0.0, flag("enemy_calm", false), None);
                    println!("dragons: {count} {}", look.name());
                }
            }
            if let Some(fraction) = setting("boss_health").and_then(|v| v.parse::<f64>().ok()) {
                game.hurt_bosses(fraction);
            }
        }
        // `--set dragon_show=1`: with nobody about, as with the camera flying, they fight as if
        // someone stood where they were put, to be seen at it, see the "dragon" script
        dragons::SHOW.store(flag("dragon_show", false), std::sync::atomic::Ordering::Relaxed);
        // `--set dragon_trace=1` says what each dragon's doing, twice a second
        dragons::TRACE.store(flag("dragon_trace", false), std::sync::atomic::Ordering::Relaxed);
        // `--set dragon_move=breath` has every dragon make only that move, as soon as it's ready,
        // wherever the player is, for shots of it in a screenshot run
        let only = setting("dragon_move").filter(|_| capture.is_some()).and_then(dragons::Move::named);
        dragons::FORCED.store(only.map_or(0, |m| m as usize + 1), std::sync::atomic::Ordering::Relaxed);
        if flag("dragon_ultra", false) {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            game.summon(&world, &water_level, dragons::ULTRA, start, ahead, setting("boss_distance").and_then(|v| v.parse().ok()).unwrap_or(55.0));
            game.promote_ultra();
            if let Some(fraction) = setting("boss_health").and_then(|v| v.parse::<f64>().ok()) {
                game.hurt_bosses(fraction);
            }
            println!("dragon: {}", dragons::ULTRA.name());
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
            // `enemy_demi=1` makes them demi-bosses of the kind each grows into, see src\demis.rs, out
            // in the open to be seen, and `boss_move=storm` has them make only that move
            if flag("enemy_demi", false) {
                game.promote_demis();
                if let Some(m) = setting("boss_move").filter(|_| capture.is_some()).and_then(bosses::Move::named) {
                    game.force_boss_move(m);
                }
            }
            // `enemy_health=0.6` starts them hurt, their bars and levels showing
            if let Some(fraction) = setting("enemy_health").and_then(|v| v.parse::<f64>().ok()) {
                game.hurt_monsters(fraction);
            }
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
        // `--set specialist=bowyer` puts that specialist `specialist_distance=3` meters ahead, see
        // merchants.rs, and `specialist=all` every one of them side by side
        if let Some(wanted) = setting("specialist") {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            let side = Vector3::new(-ahead.y, ahead.x, 0.0).try_normalize(1e-6).unwrap_or(Vector3::y());
            let distance = setting("specialist_distance").and_then(|v| v.parse().ok()).unwrap_or(3.0);
            let trades: Vec<merchants::Trade> = if wanted == "all" { merchants::Trade::ALL.to_vec() } else { merchants::Trade::by_id(wanted).into_iter().collect() };
            let vpm = world.voxels_per_meter as f64;
            for (k, &trade) in trades.iter().enumerate() {
                let by = (k as f64 - (trades.len() as f64 - 1.0) / 2.0) * 1.1 * vpm;
                game.summon_specialist(&world, &water_level, start + side * by, ahead, distance, trade);
            }
        }
        // `--set villager=3` puts a villager of each kind that many meters ahead, to see them close
        if let Some(distance) = setting("villager").and_then(|v| v.parse::<f64>().ok()) {
            let ahead = (camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            game.summon_villagers(&world, &water_level, start, ahead, distance);
            // And `villager_walk=1` sets them off strolling across in front, to see them walk
            if flag("villager_walk", false) {
                game.walk_villagers(ahead.x.atan2(-ahead.y), 30.0);
            }
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
        // A preview of the loading screen shows it fade into the menu, as it does in play, and
        // `--set menu=1` shows it in a screenshot run, to see it with `gui=1`
        let menu_open = flag("menu", capture.is_none() || loading::previewing()) && !editor::is_app();
        // Saves go where they're kept, except in screenshot runs, which save only with `--set
        // saves=<folder>`, so trying things out never touches the player's own
        let session = session::Session::new(setting("saves").map(PathBuf::from), capture.is_none() && !editor::is_app());
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
        // The keys, kept in %LOCALAPPDATA%\Innert\keys.txt; screenshot runs play on the ones they
        // had to begin with and keep them only with `--set keys=<file>`. `--set controls=1` opens
        // their window, `rebind=jump:G` puts that action on that key as the window does, and
        // `rebinding=jump` has it waiting for a key for one, see bindings.rs
        let keys = {
            let mut keys = bindings::Keys::new(setting("keys").map(PathBuf::from).or_else(|| capture.is_none().then(bindings::kept_file)));
            keys.open = flag("controls", false);
            // `--set controls_scroll=900` that far down its list
            keys.scroll_to = setting("controls_scroll").and_then(|v| v.parse().ok());
            if let Some((action, key)) = setting("rebind").and_then(|v| v.split_once(':'))
                && let (Some(action), Some(key)) = (bindings::Action::by_id(action), bindings::Input::parse(key))
            {
                keys.bind(action, 0, key);
            }
            if let Some(action) = setting("rebinding").and_then(bindings::Action::by_id) {
                keys.change(action, 0);
            }
            keys
        };

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
            builds_pipeline,
            bases: bases::Bases::new(),
            building: building::Building::default(),
            raid: night::Raid::default(),

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
            // `--set time=21.5` starts at that hour, `--set day_cycle=0` holds the sun still where
            // it is then, and setting where the sun is holds it there
            cycle: {
                let mut cycle = daynight::DayNight::new(daynight::Clock::default());
                // `--set day=3` for the fourth day, and its moon
                if let Some(day) = setting("day").and_then(|v| v.parse::<u32>().ok()) {
                    cycle.clock.day = day;
                }
                if let Some(hour) = setting("time").and_then(|v| v.parse::<f64>().ok()) {
                    cycle.set_hour(hour);
                }
                cycle.clock.running = flag("day_cycle", true);
                cycle
            },
            fixed_sun: ["sun_azimuth", "sun_elevation", "sun_ahead"].iter().any(|k| setting(k).is_some()) || editor::is_app(),
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
            model_files: assets::Reloader::new(),
            editor: Default::default(),
            weapon_models,
            models_version,
            instance_allocator,
            light_cache,
            light_cache_for: None,
            sky_epoch: 0,
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
            edit_uploads: Vec::new(),
            pending_craters: VecDeque::new(),
            pending_ripples: Vec::new(),
            pending_stains: Vec::new(),
            stains: Vec::new(),
            step_distance: 0.0,
            walk_speed: 0.0,
            walked: 0.0,
            waterfall_index: 0,

            menu_open,
            started: !menu_open,
            backdrop: None,
            bare,
            session,
            // `--set new_seed=42` for a new Free Roam game on that seed
            new_seed: setting("new_seed").and_then(|v| v.parse().ok()).unwrap_or(terrain.seed),
            hook: None,
            fall_back: None,
            map: compass::Map::default(),
            wings: wings::Flight::default(),
            wards: guardians::Wards::default(),
            // `--set settings=1` starts with them shown, to see them in screenshots with `gui=1`
            show_settings: flag("settings", false),
            keys,
            // `--set inventory=1` shows the inventory, to see how it looks
            inventory_open: flag("inventory", false),
            skills_open: flag("skills", false),
            skills_closed: false,
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
        // `--set mode=freeroam` or `realms` starts playing that mode straight away, going on from
        // its save if there is one, or afresh with `--set new=1`, as picking it from the menu
        // does. Then `health=37`, `gold=900` and `weapon_seed=7` change the player, to see it all
        // come back from the save next time.
        if let Some(mode) = setting("mode").and_then(save::Mode::parse) {
            let saved = if flag("new", false) { None } else { app.session.saves.saved(mode).cloned() };
            let view = (app.camera.position, app.camera.rotation);
            app.begin(mode, saved);
            // `--set crevice=fight` and the like with a mode too: the player stands where the view
            // of the ravine is, near where the game sets out, see Placed::place()
            if setting("crevice").is_some() {
                let eye = camera_to_voxels(view.0, app.world.size);
                let mut player = Player::new(eye, &app.world);
                player.position = eye - Vector3::new(0.0, 0.0, 1.6 * app.world.voxels_per_meter as f64);
                app.player = Some(player);
                (app.camera.position, app.camera.rotation) = view;
            }
            if let Some(health) = setting("health").and_then(|v| v.parse::<f64>().ok()) {
                app.game.set_health(health);
            }
            if let Some(gold) = setting("gold").and_then(|v| v.parse().ok()) {
                app.game.set_gold(gold);
            }
            if let Some(level) = setting("player_level").and_then(|v| v.parse().ok()) {
                app.game.set_level(level);
            }
            if let Some(seed) = setting("weapon_seed").and_then(|v| v.parse().ok()) {
                app.game.arm(seed, setting("weapon_luck").and_then(|v| v.parse().ok()).unwrap_or(1.0));
            }
            // And `realm=1`, `blocks=3` and the like to try out the journey through the realms
            app.try_journey(&settings);
            // `weapon=handcannon` in hand in the mode started too, not only in the world before it
            if let Some(family) = setting("weapon") {
                app.game.select_family(family);
            }
            println!("STATE at the start: {}", app.state_line());
        }
        // `--set confirm=freeroam` asks whether to start a new game in its place, as New does
        app.session.menu.confirm = setting("confirm").and_then(save::Mode::parse);
        // `--set map_of=new:realms` or `continue:freeroam` has the map beside the menu show that
        // game, as pointing at its button does
        app.session.menu.looking = setting("map_of").and_then(|v| {
            let (which, mode) = v.split_once(':')?;
            let mode = save::Mode::parse(mode)?;
            match which {
                "new" => Some(session::MenuAction::New(mode)),
                "continue" => Some(session::MenuAction::Continue(mode)),
                _ => None,
            }
        });
        // `--set creator=freeroam` opens the window a new character's made in, as New does, with
        // what `--set class=priest`, `name=`, `skin=` and the rest say chosen, turned `creator_turn`
        // radians round, to see it in a screenshot with `menu=1 gui=1`
        if let Some(mode) = setting("creator").and_then(save::Mode::parse) {
            let turn = setting("creator_turn").and_then(|v| v.parse().ok()).unwrap_or(0.5);
            let mut creator = character::creator::Creator::with(mode, character::Character::from_settings(), turn);
            // And `creator_scroll=600` scrolled that far down what's chosen, to see the bottom of it
            creator.scroll_to = setting("creator_scroll").and_then(|v| v.parse().ok());
            // And `paint_view=face` painting, looking at the face, `paint_hover=0.4,0.5` the mouse
            // taken to be that far across and down the picture, see character\studio.rs
            if let Some(focus) = setting("paint_view").and_then(character::studio::Focus::parse) {
                creator.paint(focus);
                if let (Some(studio), Some(at)) = (&mut creator.studio, setting("paint_hover")) {
                    let v: Vec<f32> = at.split(',').filter_map(|v| v.trim().parse().ok()).collect();
                    studio.hover_at = (v.len() == 2).then(|| [v[0], v[1]]);
                }
            }
            app.session.creator = Some(creator);
        }
        // `--set shot_at=20,60` takes shots at those frames too, with `gui=1` of the whole screen,
        // as for the menu or the window a new character's made in
        if let (Some(frames), Some(capture)) = (setting("shot_at"), &mut app.capture) {
            capture.shots.extend(frames.split(',').filter_map(|f| f.trim().parse::<usize>().ok()));
        }
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
        // `--set talk=1` talks to the specialist nearest the start, see merchants.rs, and
        // `talk_do=invite`, `land` or `trade` says that, with `talk_gold=500` to pay with and
        // `talk_home=1` a fireplace at the base to come to
        if flag("talk", false) {
            app.game.stage_talk(start, setting("talk_do"), setting("talk_gold").and_then(|v| v.parse().ok()), flag("talk_home", false));
        }
        // `--set map=1` opens the map, to see what's marked on it
        if flag("map", false) {
            app.map.open = true;
        }
        // `--set build=1` starts building, the pieces to pick from showing, and `--set craft=1`
        // shows the inventory's Craft tab when it's open, to see them with `gui=1`
        if flag("build", false) {
            app.building.on = true;
        }
        if flag("craft", false) {
            app.building.craft_open = true;
        }
        // `--set shots_at=80,140` takes shots at those frames too, with the GUI with `gui=1`, as of
        // the window of a specialist being talked to
        if let Some(capture) = &mut app.capture {
            capture.shots.extend(setting("shots_at").map_or(Vec::new(), |v| v.split(',').filter_map(|n| n.trim().parse::<usize>().ok()).collect()));
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

    /// Whether one of `action`'s keys went down this frame, as the Controls window has them, see
    /// bindings.rs
    fn pressed(&self, action: bindings::Action) -> bool {
        self.keys.map.pressed(&self.input, action)
    }

    /// Whether one of `action`'s keys is held down
    fn held(&self, action: bindings::Action) -> bool {
        self.keys.map.held(&self.input, action)
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
        // ruins and pyramids, Ironhide the shrines and colonnades. Each waits a little way out in
        // front, where there's room to fight. Down in the ravines it's demi-boss's instead, below.
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
                if pick >= BOSS_CHANCE && !matches!(site.kind, "dungeon" | "cavern" | "bandit camp" | "fallen throne" | "undercity heart" | "dragon lair") {
                    return None;
                }
                let look = match site.kind {
                    // The Worldpyre Ancient in its far lair, in Free Roam: in Realms it's the Cinder
                    // Crown's finale instead, see dragons.rs
                    "dragon lair" if self.session.mode != Some(save::Mode::Realms) => dragons::ULTRA,
                    "dragon lair" => return None,
                    // Raid bosses: in a third of the dungeons' deepest halls, and the odd castle and
                    // pyramid
                    "dungeon" if pick % 3 == 0 => animals::Look::CryptTitan,
                    // The dead lord of a fallen city in its temple: a titan in some, a knight in the
                    // rest, see worldgen\undercity.rs
                    "undercity heart" if pick % 4 == 0 => animals::Look::CryptTitan,
                    "undercity heart" => animals::Look::GraveKnight,
                    "castle" if pick % 5 == 0 => animals::Look::Colossus,
                    "pyramid" if pick % 5 == 0 => animals::Look::ElderWyrm,
                    // A pharaoh sunk in the desert's ruins and pyramids; an ogre in the old castles,
                    // towers and caverns, a jotun where it's cold and a thunderborn giant on the high
                    // stone; an elderhorn at the woods' shrines, a mire crone at the jungle's, see
                    // Look::for_ground and src\rig.rs
                    // The Gilded Sphinx, a raid boss, in some of the desert's ruins, and the
                    // Cinderback Tortoise in others, see src\bosses\roster.rs
                    "desert ruins" if pick % 5 == 0 => animals::Look::GildedSphinx,
                    // A Dune Wyrm under the sand out in front of others, see src\bosses\wyrms.rs
                    "desert ruins" | "pyramid" if pick % 5 == 3 => animals::Look::DuneWyrm,
                    "pyramid" | "desert ruins" if pick % 3 == 1 => animals::Look::SandPharaoh,
                    "desert ruins" if pick % 3 == 0 => animals::Look::Cinderback,
                    // The Lantern Widow in half the flooded churches, the Fenhulk in a quarter, a drake
                    // in the rest, see src\bosses\roster\wardens.rs
                    "flooded church" if pick % 2 == 0 => animals::Look::LanternWidow,
                    "flooded church" if pick % 4 == 1 => animals::Look::Fenhulk,
                    // The Gloamdelver digging in a fifth of the great caverns
                    "cavern" if pick % 5 == 1 => animals::Look::Gloamdelver,
                    // The Skymane on the high towers, see src\dragons\skymane.rs, and the Grotto Squid
                    // in the pools of the waterfall caves, see src\bosses\kraken.rs
                    "watchtower" if pick % 6 == 2 => animals::Look::Skymane,
                    "waterfall cave" => animals::Look::GrottoSquid,
                    "castle" | "watchtower" if pick % 4 == 3 => animals::Look::Ogre,
                    "cavern" if pick % 4 == 3 => animals::Look::Ogre,
                    "shrine" if pick % 2 == 0 => animals::Look::Elderhorn,
                    // Each turns into a kind of its own for the land round it, see Look::for_ground
                    // A castle fallen to the dead has one on its throne
                    "dungeon" | "fallen throne" => animals::Look::GraveKnight,
                    "castle" if pick % 3 == 1 => animals::Look::Ironhide,
                    "castle" if pick % 3 == 2 => animals::Look::GraveKnight,
                    // The great carving over a ruined castle's gate, that the gargoyles on its walls
                    // answer to, see gargoyles.rs
                    "castle" if pick % 2 == 0 => animals::Look::Grotesque,
                    "castle" | "desert ruins" => animals::Look::Golem,
                    "watchtower" | "colonnade" if pick % 2 == 1 => animals::Look::Direwolf,
                    "watchtower" | "pyramid" | "flooded church" => animals::Look::Drake,
                    "colonnade" => animals::Look::Ironhide,
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
                let out = if matches!(site.kind, "dungeon" | "cavern" | "bandit camp" | "fallen throne" | "undercity heart" | "dragon lair") { 0.0 } else { 16.0 };
                let at = [site.position[0] + fx / length * out, site.position[1] + fy / length * out, site.position[2]];
                // As hard to beat there as a realm's finale, see realms::FINALE_PROMOTION
                let key = if look == dragons::ULTRA { realms::finale_key_at([at[0] as f64, at[1] as f64]) } else { key };
                Some(game::Lair { look, position: to_voxels(at), key, raid: None })
            })
            .collect();
        // In Realms, the realm's own three raid bosses too, and the Innert Gate or its portal home,
        // see session.rs
        let (raids, shrines) = self.realm_sites();
        let lairs: Vec<game::Lair> = lairs.into_iter().chain(raids).chain(self.serpent_sites()).collect();
        // Monsters haunt the other halls of dungeons, cave spiders nest in caverns or gloomwings
        // roost there, bandits keep camps, as many as each camp's band, their hoard turning up
        // once they're all dead, and burrowers dig in out in front of desert ruins and pyramids.
        // Down at the bottoms of open ravines the dead walk among the bones, gloomwings roost
        // among the crystals and spiders nest, see worldgen\ravines.rs.
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
                    worldgen::ROOST_SITE => (game::DenKind::Roost, 0.0, None),
                    worldgen::NEST_SITE => (game::DenKind::Nest, 0.0, None),
                    worldgen::BONES_SITE => (game::DenKind::Hall, 0.0, None),
                    "desert ruins" | "pyramid" => (game::DenKind::Burrows, 12.0, None),
                    "fallen castle" | "fallen court" | "fallen hall" | "fallen throne" => (game::DenKind::Fallen, 0.0, None),
                    // The dead walking the streets, the square, the canal and the temple of a city
                    // fallen under the land
                    "undercity gate" | "undercity street" | "undercity canal" | "undercity plaza" | "undercity hall" => (game::DenKind::Sunken, 0.0, None),
                    // Gargoyles asleep in stone on a ruin's tower, over a fallen castle's gate and
                    // before a fallen city's temple, see gargoyles.rs
                    "gargoyle perch" => (game::DenKind::Perch, 0.0, None),
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
                    return Some(game::Stall { position: to_voxels(site.position), key: [key[0], key[1], key[2] + 7], within: 0.3, royal: None, trade: None });
                }
                // A specialist's stall beside it, see merchants.rs
                if let Some(trade) = merchants::Trade::of_site(site.kind) {
                    let heading = (site.facing[1] as f64).atan2(site.facing[0] as f64);
                    return Some(game::Stall { position: to_voxels(site.position), key: [key[0], key[1], key[2] + 7], within: 0.0, royal: None, trade: Some((trade, heading)) });
                }
                // The princess keeps the royal armoury in a castle's throne room
                if site.kind.starts_with("royal") {
                    let heading = (site.facing[1] as f64).atan2(site.facing[0] as f64);
                    return (site.kind == "royal stall").then(|| game::Stall { position: to_voxels(site.position), key: [key[0], key[1], key[2] + 7], within: 0.3, royal: Some(heading), trade: None });
                }
                let pick = (key[0].wrapping_mul(73_856_093) ^ key[1].wrapping_mul(19_349_663)).rem_euclid(100);
                if pick < BOSS_CHANCE || pick % 3 != 0 || site.kind.starts_with("fallen") || site.kind.starts_with("undercity") || site.kind.starts_with("ravine") || site.kind == worldgen::LURKER_SITE || matches!(site.kind, "dungeon" | "dungeon hall" | "ravine" | "crevice" | "cavern" | "cave nest" | "bandit camp" | "town" | "cave town" | "gargoyle perch") {
                    return None;
                }
                let [fx, fy] = site.facing;
                let length = (fx * fx + fy * fy).sqrt().max(1e-3);
                let at = [site.position[0] + fx / length * 11.0, site.position[1] + fy / length * 11.0, site.position[2]];
                Some(game::Stall { position: to_voxels(at), key: [key[0], key[1], key[2] + 7], within: 4.0, royal: None, trade: None })
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
        // A demi-boss down in one in five of the open ravines, the desert's, the snow's and the
        // swamps' their own kinds, and out in front of a third of the ruins no boss keeps an
        // armoured dead thing, see src\demis.rs
        let climate = self.terrain_settings();
        let ravines = landscape.sites.iter().filter(|site| site.kind == worldgen::LURKER_SITE).map(|site| {
            let c = worldgen::climate(&climate, site.position[0], site.position[1]);
            let key = demis::ravine(site.position.map(|v| (v * 10.0).round() as i64), c.snow, c.desert, c.jungle);
            game::DemiLair { position: to_voxels(site.position), heading: (site.facing[1] as f64).atan2(site.facing[0] as f64), key }
        });
        let ruins = landscape.sites.iter().filter_map(|site| {
            let key = site.position.map(|v| (v * 10.0).round() as i64);
            let pick = (key[0].wrapping_mul(73_856_093) ^ key[1].wrapping_mul(19_349_663)).rem_euclid(100);
            if pick < BOSS_CHANCE || pick % 3 != 0 || !matches!(site.kind, "castle" | "watchtower" | "colonnade" | "shrine" | "desert ruins" | "pyramid") {
                return None;
            }
            let [fx, fy] = site.facing;
            let length = (fx * fx + fy * fy).sqrt().max(1e-3);
            let at = [site.position[0] + fx / length * 12.0, site.position[1] + fy / length * 12.0, site.position[2]];
            Some(game::DemiLair { position: to_voxels(at), heading: (fy as f64).atan2(fx as f64), key: demis::keyed(key, demis::TOMB) })
        });
        let demis: Vec<game::DemiLair> = ravines.chain(ruins).collect();
        self.game.set_demis(&self.world, &|x, y| water.surface_at(x, y), demis);
        self.game.set_shrines(&self.world, shrines);
        self.place_omens();

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
        // The wing shrines on the peaks, a pair of wings in each one's nest, see worldgen\aeries.rs
        let shrines = landscape.sites.iter().filter(|s| s.kind == worldgen::SHRINE);
        let key = |p: [f32; 3]| [(p[0] * 10.0).round() as i64, (p[1] * 10.0).round() as i64, worldgen::SHRINE_KEY];
        self.game.set_aeries(shrines.map(|s| (key(s.position), to_voxels(s.position))).collect(), self.world.voxels_per_meter as f64);
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

    /// Makes ready what's been carved and painted since last time to go over to the GPU with the
    /// next frame, all at once, see record_edits(). Nothing waits for the frames being drawn: what
    /// goes over is copied from host memory of its own, and the frame's copies come after
    /// everything sent to the GPU before, as each command buffer's first use of the world comes
    /// after all that, so no frame still being drawn sees a brick half written.
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
        if grew {
            // Out of room for more bricks: a bigger buffer, the ones there copied over with the
            // next frame before what's changed goes in, see grow_with_frame()
            self.voxel_set.grow_with_frame(needed, spare_bricks(), &self.memory_allocator, &self.descriptor_set_allocator, &self.render_pipeline);
        }
        // The cells of the grid that changed, then their bricks, where they're kept in the room
        // there is, see World::cell()
        let mut words: Vec<u32> = cells.iter().map(|&c| self.world.brick_grid[c]).collect();
        let grid = cells
            .iter()
            .enumerate()
            .map(|(k, &c)| BufferImageCopy { buffer_offset: (k * 4) as u64, image_offset: self.world.cell_at(c), image_extent: [1, 1, 1], ..Default::default() })
            .collect();
        let mut bricks = Vec::new();
        for &c in &cells {
            if let Some((index, brick)) = self.world.brick_words(c) {
                bricks.push(BufferCopy {
                    src_offset: (words.len() * 4) as u64,
                    dst_offset: (index * worldgen::BRICK_WORDS * 4) as u64,
                    size: (worldgen::BRICK_WORDS * 4) as u64,
                    ..Default::default()
                });
                words.extend_from_slice(brick);
            }
        }
        let ground = match self.edited_ground.take() {
            // The ground, and where the water was laid on it, for each part of the room
            Some((lo, hi)) => self
                .water
                .take_ground(&self.world, lo, hi)
                .into_iter()
                .map(|p| {
                    let region = |at: [u32; 2], extent: [u32; 2]| BufferImageCopy { image_offset: [at[0], at[1], 0], image_extent: [extent[0], extent[1], 1], ..Default::default() };
                    (
                        (host_words(&self.memory_allocator, p.ground), region(p.at, p.extent)),
                        (host_words(&self.memory_allocator, p.rest), region(p.rest_at, p.rest_extent)),
                    )
                })
                .collect(),
            None => Vec::new(),
        };
        if words.is_empty() && ground.is_empty() {
            return;
        }
        // An empty buffer can't be made, so a word for nothing when only the ground changed
        let words = if words.is_empty() { vec![0] } else { words };
        self.edit_uploads.push(EditUpload { words: host_words(&self.memory_allocator, words), grid, bricks, ground });
        // Screenshot runs say when making them ready took a while
        if self.capture.is_some() && (grew || start.elapsed().as_secs_f64() > 0.004) {
            println!("Edits made ready in {:.1} ms: {} bricks{}", start.elapsed().as_secs_f64() * 1000.0, cells.len(), if grew { ", after making more room" } else { "" });
        }
    }

    /// Copies `edits`, what flush_edits() made ready, over to the GPU with the frame being recorded
    /// into the world `voxel_set` and the ground of `water`, after the bricks there were if there's
    /// more room for them now and before anything's drawn
    fn record_edits(edits: &mut Vec<EditUpload>, voxel_set: &GpuWorld, water: &Water, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        if edits.is_empty() {
            return;
        }
        let grid = voxel_set.grid.clone();
        let images = water.images();
        for edit in std::mem::take(edits) {
            if !edit.grid.is_empty() {
                let regions: Vec<BufferImageCopy> = edit.grid.into_iter().map(|r| BufferImageCopy { image_subresource: grid.subresource_layers(), ..r }).collect();
                builder.copy_buffer_to_image(CopyBufferToImageInfo { regions: regions.into(), ..CopyBufferToImageInfo::buffer_image(edit.words.clone(), grid.clone()) }).unwrap();
            }
            if !edit.bricks.is_empty() {
                builder.copy_buffer(CopyBufferInfo { regions: edit.bricks.into(), ..CopyBufferInfo::buffers(edit.words, voxel_set.bricks.clone()) }).unwrap();
            }
            for pair in edit.ground {
                for ((words, region), image) in [(pair.0, &images.ground), (pair.1, &images.rest)] {
                    let region = BufferImageCopy { image_subresource: image.subresource_layers(), ..region };
                    builder.copy_buffer_to_image(CopyBufferToImageInfo { regions: [region].into(), ..CopyBufferToImageInfo::buffer_image(words, image.clone()) }).unwrap();
                }
            }
        }
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
        let birds = outdoors && self.sky().sun.z > 0.09 && self.game.leaves_falling() > 4;
        let quiet = if outdoors { 1.0 } else { 0.35 };
        if let Some(sound) = &mut self.audio {
            sound.ambience(wind as f32 * quiet, ((wet / all) as f32 * 1.6).min(1.0) * quiet, birds, &view, dt);
        }
    }

    /// The sky as it's drawn now: going round with the day, or held where the settings put the sun
    fn sky(&self) -> daynight::Sky {
        if self.fixed_sun { daynight::DayNight::fixed_sky(self.sun_azimuth as f64, self.sun_elevation as f64) } else { self.cycle.sky() }
    }

    /// The day's turned to `part`: the dusk's warned of with a bell, as the monsters will come
    /// once it's dark, see raids.rs
    fn day_turned(&mut self, part: daynight::Part) {
        let message = match part {
            daynight::Part::Dusk => "The sun's going down. Monsters come out at night: get behind walls.",
            daynight::Part::Night => "Night has fallen.",
            daynight::Part::Dawn => "Dawn breaks.",
            daynight::Part::Day => return,
        };
        self.game.say(message);
        if matches!(part, daynight::Part::Dusk | daynight::Part::Dawn) {
            let view = self.view();
            if let Some(sound) = &mut self.audio {
                sound.play(audio::Cue::here(audio::Sound::Toll, if part == daynight::Part::Dusk { 0.55 } else { 0.35 }).pitched(if part == daynight::Part::Dusk { 0.85 } else { 1.1 }), &view);
            }
        }
        if let Some(capture) = &self.capture {
            println!("DAY TURNED to {part:?} at frame {}, {:.2} o'clock", capture.frame_times.len(), self.cycle.hour());
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
        // What was there before isn't drawn with any more, nor what was still to go over of it
        if let Some(rcx) = &mut self.rcx {
            wait_for_frames(rcx, &self.device);
        }
        self.edited.clear();
        self.edited_ground = None;
        self.edit_uploads.clear();
        self.pending_craters.clear();
        self.pending_ripples.clear();
        self.pending_stains.clear();
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

    /// Something going into the water at `at`, splashing `size`, 1 for a spell's, see
    /// Combat::waves: the water knocked down there at once, a dip that rolls out as a ring of
    /// wave, and let back a moment later, spread wider, so the wave goes on out and none of it's
    /// lost. As much as is there to knock down in shallows, so it never takes the bottom dry.
    fn ripple(&mut self, at: Vector3<f64>, size: f64, origin: Vector3<f64>) {
        let vpm = self.world.voxels_per_meter as f64;
        let radius = (0.7 + 0.45 * size) * vpm;
        let there = self.water.volume_near(at.x, at.y, radius);
        let volume = (RIPPLE_VOLUME * size.powf(1.5) * vpm.powi(3)).min(0.6 * there);
        if volume < 0.05 * vpm.powi(3) {
            return;
        }
        self.pending_ripples.push((0.0, at + origin, radius, -volume));
        self.pending_ripples.push((RIPPLE_REFILL_SECONDS, at + origin, radius * 1.9, volume));
        // At most a few dozen waiting, the oldest let go, as a hail of shots into the sea needs
        // only so many waves
        if self.pending_ripples.len() > 48 {
            self.pending_ripples.drain(..self.pending_ripples.len() - 48);
        }
    }

    /// The first of what's waiting in pending_ripples that's due, if any, into the water
    fn let_ripples_out(&mut self, dt: f64, origin: Vector3<f64>) {
        for r in &mut self.pending_ripples {
            r.0 -= dt;
        }
        if let Some(k) = self.pending_ripples.iter().position(|r| r.0 <= 0.0) {
            let (_, at, radius, volume) = self.pending_ripples.remove(k);
            let at = at - origin;
            self.water.pour(at.x, at.y, radius, volume);
        }
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
        let key = self.sky().key;
        let sun = key.y.atan2(key.x);
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
            self.save_game(true);
            event_loop.exit();
            return;
        }
        // What was picked from the menu last frame, and saving as the game goes on
        if let Some(action) = self.session.action.take() {
            self.menu_picked(action, event_loop);
        }
        if let Some(pick) = self.session.creator_pick.take() {
            self.creator_picked(pick);
        }
        // `--set pick=new:freeroam@120` or `continue:realms@120` in a screenshot run starts that
        // game at that frame, as its button would, a new one with the character the settings make,
        // and says how long it took till its world was there to play in
        static PICK: std::sync::OnceLock<Option<(bool, save::Mode, usize)>> = std::sync::OnceLock::new();
        static PICKED: std::sync::Mutex<Option<Instant>> = std::sync::Mutex::new(None);
        let pick = *PICK.get_or_init(|| {
            let v = settings_from_args().into_iter().find(|(k, _)| k == "pick")?.1;
            let (what, at) = v.split_once('@')?;
            let (which, mode) = what.split_once(':')?;
            Some((which == "new", save::Mode::parse(mode)?, at.parse().ok()?))
        });
        if let (Some((new, mode, at)), Some(frame)) = (pick, self.capture.as_ref().map(|c| c.frame_times.len()))
            && frame == at
        {
            *PICKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
            let save = if new { None } else { self.session.saves.saved(mode).cloned() };
            self.begin(mode, save);
        }
        self.update_session(delta_time.min(0.25));
        let mut picked = PICKED.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(when) = *picked
            && !self.session.arriving
            && !self.session.find_ground
        {
            println!("PICKED: the world there to play in, the loading screen gone, {:.0} ms after the game was started", when.elapsed().as_secs_f64() * 1000.0);
            *picked = None;
        }
        drop(picked);
        // `--set save_at=300` saves at that frame of a screenshot run, saying what it saved
        static SAVE_AT: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
        let save_at = *SAVE_AT.get_or_init(|| settings_from_args().into_iter().find(|(k, _)| k == "save_at").and_then(|(_, v)| v.parse().ok()));
        if let Some(frame) = self.capture.as_ref().map(|c| c.frame_times.len())
            && save_at == Some(frame)
        {
            self.save_game(true);
            println!("STATE saved at frame {frame}: {}", self.state_line());
        }

        // Screenshot runs from the command line ignore input, so they always show the same view
        let interactive = self.capture.is_none();

        // A key being changed in the Controls window takes what's pressed, Esc too, and nothing
        // else does meanwhile, see bindings.rs
        let rebinding = interactive && self.keys.take(&self.input);
        if interactive && !rebinding && self.input.key_pressed(KeyCode::Escape) && !editor::is_app() {
            if self.keys.open {
                self.keys.close();
                if !self.menu_open && !self.inventory_open {
                    self.set_focus(true);
                }
            } else if self.session.gate_open {
                self.session.gate_action = Some(session::GateAction::Close);
            } else if self.skills_open {
                self.skills_open = false;
                self.set_focus(true);
            } else if self.map.open {
                self.toggle_map();
            } else if self.game.shopping() {
                self.game.close_shop();
                if !self.inventory_open {
                    self.set_focus(true);
                }
            } else if self.inventory_open {
                self.inventory_open = false;
                self.set_focus(true);
            } else if self.menu_open && self.session.creator.is_some() {
                // Back from painting to making a new character, or from that to the menu
                if !self.session.creator.as_mut().is_some_and(|c| c.close_paint()) {
                    self.session.creator = None;
                }
            } else if self.menu_open && self.started {
                self.menu_open = false;
                self.set_focus(true);
            } else {
                self.menu_open = true;
                self.set_focus(false);
                // Saved whenever the menu's opened, should the game be left from it
                self.save_game(false);
            }
        }
        use bindings::Action;
        // Nor while the Controls window's open, the world held still behind it
        let playing = interactive && !self.menu_open && !self.keys.open;
        if playing && !self.game.shopping() && !self.skills_open && self.pressed(Action::Inventory) {
            self.inventory_open = !self.inventory_open;
            self.set_focus(!self.inventory_open);
        }
        // The skill web, see skills.rs: over everything, the world held still behind it
        if std::mem::take(&mut self.skills_closed) {
            self.set_focus(true);
        }
        if playing && !self.game.shopping() && self.loaded_scene.is_terrain() && self.pressed(Action::Skills) {
            self.skills_open = !self.skills_open;
            self.inventory_open = false;
            self.set_focus(!self.skills_open);
        }
        // The model editor's own app is the editor, open from the start, see editor.rs
        if editor::is_app() && !self.editor.open {
            let eye = camera_to_voxels(self.camera.position, self.world.size);
            self.editor.toggle(&self.world, eye, &mut self.game);
            self.set_focus(false);
        }
        let playing = playing && !self.editor.open;
        if playing && self.pressed(Action::Settings) {
            self.show_settings = !self.show_settings;
        }
        if playing && self.pressed(Action::Map) {
            self.toggle_map();
        }

        let eye = camera_to_voxels(self.camera.position, self.world.size);
        let look = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz().normalize();
        // Scripts fight by themselves, see run_script()
        let mut controls = std::mem::take(&mut self.script_controls);
        if playing && !self.focused && !self.inventory_open && !self.game.shopping() && !self.session.gate_open && self.input.mouse_pressed(MouseButton::Left) {
            self.set_focus(true);
        } else if playing && self.focused && !self.game.dead() {
            // Gathering with G, and building with B, which takes the mouse and the keys for picking
            // what to build, see building.rs
            let building = self.loaded_scene.is_terrain() && self.update_building(delta_time);
            let bucket = self.game.holding_bucket() && !self.game.has_target();
            let armed = self.game.holding_weapon() && !building;
            if building {
            } else if armed {
                controls = combat::Controls {
                    attack: self.pressed(Action::Attack),
                    holding: self.held(Action::Attack),
                    heavy: self.pressed(Action::Heavy),
                    aiming: self.held(Action::Heavy),
                    special: self.pressed(Action::Special),
                    reload: self.pressed(Action::Reload),
                };
            } else if self.pressed(Action::Attack) {
                if bucket {
                    self.scoop_water(eye, look);
                } else if self.game.holding_hook() && !self.game.has_target() {
                    // The grappling hook thrown, see rope.rs
                    self.throw_hook();
                } else {
                    self.game.primary(eye);
                }
            }
            if self.pressed(Action::Ammo) {
                self.game.switch_ammo();
            }
            // Building, E mends what's damaged
            if self.pressed(Action::Mend) && building {
                self.mend_here();
            } else if self.pressed(Action::Interact) && !building && !self.door_here() && !self.furnace_here() && !self.hearth_here() {
                self.game.interact(eye);
                // Trading with a merchant frees the mouse for their stall
                if self.game.shopping() {
                    self.set_focus(false);
                }
            }
            if self.pressed(Action::Heavy) && !armed && !building {
                if self.game.holding_bucket() {
                    self.pour_water(eye, look);
                } else {
                    self.game.eat_selected();
                }
            }
            if let Some(slot) = self.keys.map.hotbar_pressed(&self.input).filter(|_| !building) {
                self.game.select(slot);
            }
            // On foot the wheel goes through what's carried, flying it zooms
            let wheel = self.input.scroll_diff().1;
            if self.focused && self.player.is_some() && wheel != 0.0 && !building {
                self.game.scroll(if wheel < 0.0 { 1 } else { -1 });
            }
        }

        // The world stands still while the menu is open, and while it's built afresh round the
        // player behind the dark, as on going through to another realm, so nothing that was about
        // them, a night's raiders least of all, can fall on them unseen meanwhile
        if !self.menu_open && !self.editor.open && !self.skills_open && !self.held_still() && self.animals_on && self.loaded_scene.is_terrain() {
            let feet = self.player.as_ref().map(|p| p.position);
            let view = self.view();
            let start = Instant::now();
            self.tell_sea();
            let water = &self.water;
            self.game.update(&self.world, &|x, y| water.surface_at(x, y), feet, &view, controls, delta_time.min(0.1));
            self.slow("the fighting, animals and such", start);
            // A tool's blow landing gathers what it struck, see tools.rs
            self.update_tools(delta_time.min(0.1));
            // Trees coming down, see felling.rs
            self.update_felling(delta_time.min(0.1));
            // The fires burning down, smoking and warming whoever's by them, see hearths.rs
            self.update_hearths(delta_time.min(0.1));
            // Monsters coming at night, and what's built taking their blows, see night.rs
            let start = Instant::now();
            self.update_raid(delta_time.min(0.1));
            self.slow("the night's raid", start);
            // The merchants living at the base, see merchants.rs
            self.update_merchants(delta_time.min(0.1));
            // The old foundations of the gate in a new game of Realms, see gates.rs
            self.lay_ruins();
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
            // Explosions blowing holes in the world, a few milliseconds' worth a frame and the rest
            // left for the frames after, so a burst of them doesn't hold one up, kept by where they
            // are in the whole landscape meanwhile; then splashing stains on it, which are quick,
            // once the holes before them are blown, so they're on the ground the holes leave; all
            // sent to the GPU together
            self.batching = true;
            let origin = Vector3::new(self.world.origin[0] as f64, self.world.origin[1] as f64, 0.0);
            let craters = self.game.take_craters();
            self.pending_craters.extend(craters.into_iter().map(|(center, radius)| (center + origin, radius)));
            // Water knocked down where things went into it, as they go in, before they've got
            // to the bottom
            for (at, size) in self.game.take_waves() {
                self.ripple(at, size, origin);
            }
            self.let_ripples_out(delta_time.min(0.1), origin);
            let stains = self.game.take_stains();
            self.pending_stains.extend(stains.into_iter().map(|(center, radius, material, burn)| (center + origin, radius, material, burn)));
            // What's blown and splashed this frame, for what's built to take the blows, see below
            let mut struck = Vec::new();
            // At least a quarter of those waiting, so however many come they never fall far behind
            let (start, least) = (Instant::now(), 1 + self.pending_craters.len() / 4);
            let mut blown = 0;
            while (blown < least || start.elapsed().as_secs_f64() < CARVE_SECONDS)
                && let Some((at, radius)) = self.pending_craters.pop_front()
            {
                self.blast(at - origin, radius);
                struck.push((at - origin, radius, true));
                blown += 1;
            }
            if self.pending_craters.is_empty() {
                let start = Instant::now();
                for (at, radius, material, burn) in std::mem::take(&mut self.pending_stains) {
                    self.stain(at - origin, radius, material, burn);
                    struck.push((at - origin, radius, false));
                }
                self.slow("splashing stains", start);
            }
            // What's built is damaged by blasts rather than holed, see building.rs
            self.blasted(&struck);
            self.batching = false;
            let start = Instant::now();
            self.flush_edits();
            self.slow("sending what changed to the GPU", start);
            // Thrown by blows, and waking up somewhere safe after being beaten
            let push = self.game.take_knockback();
            if let Some(player) = &mut self.player {
                player.push(push);
                // Carried off in the roc's talons, see game\aerie.rs
                if let Some(haul) = self.game.take_haul() {
                    player.haul(haul);
                }
            }
            // Slain, the player lies there until they rise again, at the last place they stood
            // safe. Screenshot runs rise by themselves.
            if self.game.take_died()
                && let Some(capture) = &self.capture
            {
                println!("DIED at frame {}: {}", capture.frame_times.len(), self.game.last_message().unwrap_or(""));
            }
            let rise = (interactive && self.focused && (self.pressed(Action::Jump) || self.pressed(Action::Attack)))
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

        // The day goes round, except while the menu holds the world still
        if !self.menu_open && self.loaded_scene.is_terrain() && !self.fixed_sun {
            // `--set day_speed=30` goes round that much faster, to see the sun moving costs nothing
            static DAY_SPEED: std::sync::OnceLock<f64> = std::sync::OnceLock::new();
            let speed = *DAY_SPEED.get_or_init(|| settings_from_args().into_iter().find(|(k, _)| k == "day_speed").and_then(|(_, v)| v.parse().ok()).unwrap_or(1.0));
            if let Some(part) = self.cycle.advance(delta_time.min(0.25) * speed) {
                self.day_turned(part);
            }
        }

        if playing && self.pressed(Action::Fly) {
            self.player = match self.player {
                Some(_) => None,
                None => Some(Player::new(camera_to_voxels(self.camera.position, self.world.size), &self.world)),
            };
        }

        // The grappling hook hauling the player along its rope, which jump lets go of, wings
        // spread while jump's held in the air, and the winds bringing the player back after a
        // fall below the sky isles, see rope.rs
        let jump_held = self.focused && interactive && !self.game.dead() && self.held(Action::Jump);
        let let_go = self.focused && interactive && self.pressed(Action::Jump);
        self.update_hook(delta_time.min(0.05), let_go);
        self.update_fall();
        let gliding = self.gliding(jump_held || self.session.trial_glides());
        if let Some(player) = &mut self.player {
            player.gliding = gliding;
        }
        // The wings worn spreading and folding, beating on a flap, and what they leave behind, see
        // wings.rs. Worn on the back, they leave the hands free for what's in them.
        // A flap is jump pressed in the air, a dash Shift, for wings that do
        let tried = self.session.trial_flaps();
        let shift = self.focused && interactive && self.pressed(Action::Run);
        self.update_wings(delta_time.min(0.05), gliding, let_go || tried, shift || tried);
        // The realm's raid bosses warded while its guardians stand, see guardians.rs
        self.update_guardians(delta_time.min(0.05));

        // The dead don't walk
        let alive = !self.game.dead();
        let t = |a: Action| (self.focused && interactive && alive && self.keys.map.held(&self.input, a)) as u8 as f64;
        // Which way they're walking, and whether they jump, as the keys have it
        let walk = if self.focused && interactive && alive { self.keys.map.walk(&self.input) } else { bindings::Walk::default() };
        let running = self.focused && self.held(Action::Run);
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
            let mut wish = forward * walk.forward + right * walk.right;
            if self.scripted_walk && alive {
                wish = forward;
            }
            if wish.norm() > 1.0 {
                wish.normalize_mut();
            }
            // Slowed by frost
            wish *= self.game.pace();
            let jump = walk.jump;
            let sprint = alive && (running || self.scripted_sprint);
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
            // And a shark's fin cutting the top of the water, see seas.rs
            if let Some(fin) = self.game.fin_wake(feet)
                && (self.wake.is_none() || seas::fin_turn())
            {
                self.wake = Some(fin);
            }

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
                let v = Vector3::new(t(Action::Right) - t(Action::Left), t(Action::Forward) - t(Action::Back), t(Action::FlyUp) - t(Action::FlyDown));

                let sprint = if running { 4.0 } else { 1.0 };
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

        // The model editor's view, turned about what it shows
        if self.editor.open && !interactive {
            self.editor.update(delta_time, &editor::Controls::default(), &mut self.game);
        } else if self.editor.open {
            let (dx, dy) = self.input.mouse_diff();
            let (left, right) = (self.input.mouse_held(MouseButton::Left), self.input.mouse_held(MouseButton::Right));
            let keys = [(KeyCode::Space, ' '), (KeyCode::KeyX, 'x'), (KeyCode::KeyY, 'y'), (KeyCode::KeyZ, 'z'), (KeyCode::KeyG, 'g'), (KeyCode::KeyQ, 'q'), (KeyCode::KeyE, 'e'), (KeyCode::KeyK, 'k'), (KeyCode::Delete, 'd')];
            let controls = editor::Controls {
                drag: if left { (dx as f64, dy as f64) } else { (0.0, 0.0) },
                turn: if right { (dx as f64, dy as f64) } else { (0.0, 0.0) },
                scroll: self.input.scroll_diff().1 as f64,
                keys: keys.iter().filter(|(k, _)| self.input.key_pressed(*k)).map(|(_, c)| *c).collect(),
            };
            self.editor.update(delta_time, &controls, &mut self.game);
        }
        if self.editor.open {
            let vpm = self.world.voxels_per_meter as f64;
            let (target, yaw, pitch, distance) = self.editor.camera(vpm);
            self.camera.rotation = Vector3::new(pitch, 0.0, yaw);
            self.camera.fov = 60.0;
            let forward = (self.camera.rotation_matrix() * Vector4::new(0.0, 1.0, 0.0, 0.0)).xyz();
            let mut eye = target - forward * distance;
            eye.z = eye.z.min(self.world.size[2] as f64 - 2.0 * vpm);
            self.camera.position = voxels_to_camera(eye, self.world.size);
        }
        let sky_key = self.sky().key;
        let view = [
            self.camera.position.x,
            self.camera.position.y,
            self.camera.position.z,
            self.camera.rotation.x,
            self.camera.rotation.y,
            self.camera.rotation.z,
            self.camera.fov,
            sky_key.x + 2.0 * sky_key.z,
            sky_key.y,
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
        baked::set_live(s.live_npcs);
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
            // Out in the water among the fish, and a shark, see seas.rs
            ("sea", f) => self.sea_step(f),
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
            // Keeps still `--set drift_after=20` seconds, then a shot, then moves `--set drift=40`
            // meters to the right and `drift_back` meters back, still looking the same way, for the
            // water that's been flowing to be seen beside the still water past it, see water.rs
            ("drift", f) => {
                let setting = |key: &str, default: f64| settings_from_args().into_iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(default);
                // The camera, not someone walking about
                if f == 1 {
                    self.player = None;
                }
                // A shot of how it is before moving off
                if self.play.flown == 0.0
                    && run_time >= setting("drift_after", 20.0) - 1.0
                    && let Some(capture) = &mut self.capture
                {
                    self.play.flown = 1.0;
                    capture.shots.push(f + 2);
                }
                if self.play.leg == 0 && run_time >= setting("drift_after", 20.0) {
                    self.play.leg = 1;
                    let right = Vector3::new(look.y, -look.x, 0.0).try_normalize(1e-9).unwrap_or(Vector3::x());
                    let back = -Vector3::new(look.x, look.y, 0.0).try_normalize(1e-9).unwrap_or(Vector3::y());
                    let eye = eye + (right * setting("drift", 40.0) + back * setting("drift_back", 0.0)) * vpm;
                    self.camera.position = voxels_to_camera(eye, self.world.size);
                    println!("frame {f}: drifted to {:.0}, {:.0}", eye.x / vpm, eye.y / vpm);
                    if let Some(capture) = &mut self.capture {
                        capture.shots.push(f + 2);
                    }
                }
            }
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
            // Goes round the views `--set tour=x,y,up,face,pitch;...`, in meters of the landscape, `up`
            // over the ground or the water there, facing `face` degrees round as `--set face=` does
            // and `pitch` degrees up, holding each `--set tour_hold=3` seconds with a shot at its
            // end. With `--set tour_speed=20` it flies there that many meters a second, so the world
            // moves on a strip at a time on the way, rather than going there at once. For looking the
            // water over from everywhere it's seen, see water.rs.
            ("tour", f) => {
                let settings = settings_from_args();
                let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
                let views: Vec<Vec<f64>> = setting("tour").unwrap_or_default().split(';').map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect::<Vec<f64>>()).filter(|v| v.len() == 5).collect();
                let hold = setting("tour_hold").and_then(|v| v.parse().ok()).unwrap_or(3.0);
                let speed: Option<f64> = setting("tour_speed").and_then(|v| v.parse().ok());
                if f <= 1 {
                    self.player = None;
                    self.play.leg = 0;
                    self.play.leg_end = f64::INFINITY;
                    self.play.fly_last = run_time;
                }
                let dt = (run_time - self.play.fly_last).clamp(0.0, 0.1);
                self.play.fly_last = run_time;
                let Some(view) = views.get(self.play.leg as usize) else {
                    return;
                };
                let origin = [self.world.origin[0] as f64, self.world.origin[1] as f64];
                let spot = Vector3::new(view[0] * vpm - origin[0], view[1] * vpm - origin[1], 0.0);
                let top = |x: f64, y: f64| {
                    let ground = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(x as i64, y as i64, z))).map_or(0.0, |z| z as f64 + 1.0);
                    ground.max(self.water.surface_at(x, y))
                };
                let there = Vector3::new(spot.x, spot.y, top(spot.x, spot.y) + view[2] * vpm);
                let to = there - eye;
                let arrived = self.play.leg_end.is_finite();
                if !arrived {
                    let step = match speed {
                        Some(s) if to.xy().norm() > s * vpm * dt => self.keep_up(Vector3::new(to.x, to.y, 0.0).normalize() * s * vpm * dt),
                        _ => Vector3::new(to.x, to.y, 0.0),
                    };
                    let mut at = eye + step;
                    // Over the land on the way, and down to the view's height once there
                    at.z = if step.xy().norm() < to.xy().norm() { (top(at.x, at.y) + view[2].max(6.0) * vpm).min(self.world.size[2] as f64 - 2.0 * vpm) } else { there.z };
                    self.camera.position = voxels_to_camera(at, self.world.size);
                    self.camera.rotation.z = view[3].to_radians() - std::f64::consts::FRAC_PI_2;
                    self.camera.rotation.x = view[4].to_radians();
                    if (at - there).xy().norm() < 0.5 && !self.stream.as_ref().is_some_and(|s| s.teleport.is_some()) {
                        self.play.leg_end = run_time + hold;
                        println!("frame {frame}: tour view {} at {:.1}, {:.1}, {:.1} m up", self.play.leg, view[0], view[1], at.z / vpm);
                    }
                } else {
                    // Kept there, as the water under it may still be moving
                    self.camera.position = voxels_to_camera(Vector3::new(there.x, there.y, eye.z), self.world.size);
                    if run_time >= self.play.leg_end {
                        if let Some(capture) = &mut self.capture {
                            capture.shots.push(f + 2);
                        }
                        println!("frame {frame}: tour shot {} at frame {}", self.play.leg, f + 2);
                        self.water.report(&self.command_buffer_allocator, &self.queue);
                        self.play.leg += 1;
                        self.play.leg_end = f64::INFINITY;
                        if self.play.leg as usize >= views.len()
                            && let Some(capture) = &mut self.capture
                        {
                            capture.frames = (f + 4) as u32;
                        }
                    }
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
            // The model editor, see editor.rs: opened, then a creature in a pose, a dragon's bone
            // keyed, a weapon held and one part of it, and a pair of wings, a shot of each
            ("editor", f) => {
                if f == 10 && !self.editor.open {
                    self.editor.toggle(&self.world, eye, &mut self.game);
                    self.set_focus(false);
                }
                if self.editor.open && self.editor.scripted(f, &mut self.game)
                    && let Some(capture) = &mut self.capture
                {
                    capture.shots.push(f + 2);
                }
            }
            ("summons", f) => self.summons_step(f, eye),
            ("hunt", 20) => {
                println!("frame {frame}: aiming at something: {}", self.game.has_target());
                self.game.primary(eye);
                println!("frame {frame}: carrying {}", self.game.carried());
            }
            // Every kind of special ammo flying slowly across the view in rows, for a contact sheet
            ("ammo_sheet", 5) => self.game.ammo_sheet(eye, look),
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
                        // `--set special_pitch=-0.12` looks further off, at the ground there
                        None if f == 1 => self.camera.rotation.x = settings_from_args().iter().find(|(k, _)| k == "special_pitch").and_then(|(_, v)| v.parse().ok()).unwrap_or(-0.35),
                        None => {}
                    }
                }
                // `--set special_key=right` uses its power on right click instead, as a player would
                self.script_controls = if settings_from_args().iter().any(|(k, v)| k == "special_key" && v == "right") {
                    let t = (f as f64 - when as f64) / 60.0;
                    let mut c = self.game.power_controls(t, 1.0 / 60.0);
                    // `--set power_hold=1.2` keeps right click held aiming that long, as a priest
                    // charges the great spear
                    let hold = settings_from_args().into_iter().find(|(k, _)| k == "power_hold").and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(0.0);
                    if t >= 0.0 && t < hold {
                        c.aiming = true;
                    }
                    c
                } else {
                    // `--set special_aim=1` aims it with right click held the while, its ring showing
                    let aiming = settings_from_args().iter().any(|(k, v)| k == "special_aim" && v == "1");
                    combat::Controls { special: f == when, aiming, ..Default::default() }
                };
                if f == 29 || f == 120 || f % 50 == 0 {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
            // Stands in the shallows nearest, looking straight down into the water, to see the
            // player's own body there, see character::body
            ("mirror", 1) => self.stand_in_water(0.35, true),
            // Looks down at the shadow the player's own body casts, the sun behind them
            ("shadow", _) => {
                let vpm = self.world.voxels_per_meter as f64;
                let key = self.sky().key;
                let away = -Vector3::new(key.x, key.y, 0.0).normalize();
                let eye = camera_to_voxels(self.camera.position, self.world.size);
                self.camera.look_at(voxels_to_camera(eye + away * 2.0 * vpm - Vector3::new(0.0, 0.0, 1.7 * vpm), self.world.size));
            }
            ("mirror", f) if f > 1 => self.camera.rotation.x = -1.35,
            // Casts with what's in hand at the nearest of a pack `--set arrive_enemy` put there, again
            // and again from frame `--set cast_from=40`, facing it, as a priest hurls spears
            ("cast", f) => {
                let from = settings_from_args().into_iter().find(|(k, _)| k == "cast_from").and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(40);
                if let Some(target) = self.game.nearest_foe(eye) {
                    self.camera.look_at(voxels_to_camera(target, self.world.size));
                }
                let casting = f >= from && (f - from) % 40 < 6;
                self.script_controls = combat::Controls { attack: casting, holding: casting, ..Default::default() };
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
            // Blows a hole in the shore nearest, half in the water, to see the water flow into it,
            // and says how deep it is in the middle of it as it does
            ("shore", f) => {
                let vpm = self.world.voxels_per_meter as f64;
                // Where the nearest water ahead runs up onto the land, from the water back towards
                // the eye till it's dry, found the first frame and kept
                static SHORE: std::sync::Mutex<Option<[f64; 3]>> = std::sync::Mutex::new(None);
                let mut shore: Option<Vector3<f64>> = SHORE.lock().unwrap().map(Vector3::from);
                'rings: for ring in (4..60).filter(|_| shore.is_none()) {
                    for k in 0..ring * 6 {
                        let a = k as f64 / (ring * 6) as f64 * TAU;
                        let (x, y) = (eye.x + a.cos() * ring as f64 * vpm, eye.y + a.sin() * ring as f64 * vpm);
                        if self.water.depth_at(x, y) > 0.5 * vpm {
                            let toward = Vector3::new(eye.x - x, eye.y - y, 0.0).normalize() * 0.25 * vpm;
                            let mut p = Vector3::new(x, y, 0.0);
                            while self.water.depth_at(p.x, p.y) > 0.05 * vpm {
                                p += toward;
                            }
                            shore = Some(p);
                            *SHORE.lock().unwrap() = Some(p.into());
                            break 'rings;
                        }
                    }
                }
                match shore {
                    None if f == 1 => println!("no water near"),
                    None => {}
                    Some(at) => {
                        let ground = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64;
                        if f == 1 {
                            let back = Vector3::new(at.x - eye.x, at.y - eye.y, 0.0).normalize() * 2.0 * vpm;
                            self.camera.look_at(voxels_to_camera(Vector3::new(at.x + back.x, at.y + back.y, ground), self.world.size));
                            println!("frame {frame}: the shore {:.0} m off, its ground {:.1} m up", (at - Vector3::new(eye.x, eye.y, 0.0)).xy().norm() / vpm, ground / vpm);
                        }
                        // A frost pool as a frost shot leaves one where it lands, then the hole
                        // blown where it lies, with `--set shore_pool=1`
                        if f == 38 && settings_from_args().iter().any(|(k, v)| k == "shore_pool" && v == "1") {
                            self.game.lay_pool(Vector3::new(at.x, at.y, ground + 0.5), Some(weapons::Element::Frost));
                        }
                        if f == 40 {
                            self.blast(Vector3::new(at.x, at.y, ground), 2.5);
                        }
                        if [39, 41, 50, 70, 100, 150, 220, 299].contains(&f) {
                            let deep = |dx: f64| self.water.depth_at(at.x + dx * vpm, at.y) / vpm;
                            println!("frame {frame}: the ground {:.2} m up; water deep {:.2} m in the middle, {:.2} m and {:.2} m a meter either way", ground / vpm, deep(0.0), deep(-1.0), deep(1.0));
                        }
                    }
                }
            }
            // A pool of each element and one of churned earth laid out over the ground ahead, all
            // at once, to watch them spread out, go on, die down and leave their marks, and laid
            // out again every 250 frames, for timing them
            ("pools", f) => {
                if f == 1 {
                    self.camera.rotation.x = -0.62;
                }
                if f >= 20 && (f - 20) % 250 == 0 {
                    let water = &self.water;
                    let middle = self.game.lay_pools(&self.world, &|x, y| water.surface_at(x, y), eye, look);
                    // Looking down at the middle of them, the first time
                    if f == 20 {
                        self.camera.look_at(voxels_to_camera(middle, self.world.size));
                    }
                }
            }
            // One spell cast up over the land ahead, the view narrowed to follow it a long way
            // off, to see it fly on and on
            ("far", f) => {
                // `--set far_turn=2` turns round that many radians first, away from what's near
                if f == 1 {
                    let turn = settings_from_args().into_iter().find(|(k, _)| k == "far_turn").and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(0.0);
                    self.camera.rotation.z += turn;
                    self.camera.rotation.x = 0.22;
                    self.camera.fov = 30.0;
                }
                // Nothing near for it to home in on
                if f <= 30 {
                    self.game.shoo_animals(eye, 80.0);
                }
                let cast = (30..33).contains(&f);
                self.script_controls = combat::Controls { attack: cast, holding: cast, ..Default::default() };
                if f % 15 == 0 || (30..42).contains(&f) {
                    println!("frame {frame}: {}", self.game.in_flight(eye));
                }
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
            // The player stands where they started, for the nearest boss to fight, while the camera
            // watches it from off to one side, to see how it moves, see src\rig.rs: `--set
            // view_distance=12` that many meters off, `view_from=1.4` that many radians round from
            // where the player is, `view_up=2` that many meters up, and `view_attack=1` the player
            // attacking all the while
            ("rig", f) => {
                if f == 1 && self.player.is_none() {
                    self.player = Some(Player::new(eye, &self.world));
                }
                let number = |key: &str, default: f64| settings_from_args().into_iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(default);
                let feet = self.player.as_ref().map_or(eye, |p| p.position);
                // Placed once and held still, so what moves is only the boss, unless `view_follow=1`;
                // and the player's kept on their feet, so the boss goes on fighting them
                static HELD: std::sync::Mutex<Option<(Vector3<f64>, Vector3<f64>)>> = std::sync::Mutex::new(None);
                let placing = f <= 20 || number("view_follow", 0.0) > 0.5;
                let mut held = HELD.lock().unwrap();
                // Or `watch=villager`, `watch=merchant` and the like, the nearest of that look, to see
                // how townsfolk, merchants and animals move
                let watched = match settings_from_args().into_iter().find(|(k, _)| k == "watch") {
                    Some((_, name)) => self.game.nearest_named(feet, &name),
                    None => self.game.nearest_boss(feet),
                };
                if let Some(target) = watched.filter(|_| placing) {
                    let back = (feet - target).xy().try_normalize(1e-6).unwrap_or_else(Vector2::x);
                    let from = number("view_from", 1.4);
                    let distance = number("view_distance", 12.0);
                    // Somewhere with nothing in the way, not in a tree: round one way and the other
                    // from where it's asked for, and nearer
                    let spot = |round: f64, d: f64| {
                        let (c, s) = (round.cos(), round.sin());
                        let way = Vector3::new(back.x * c - back.y * s, back.x * s + back.y * c, 0.0);
                        target + way * d * vpm + Vector3::new(0.0, 0.0, number("view_up", 1.0) * vpm)
                    };
                    let clear = |at: Vector3<f64>| {
                        let steps = ((at - target).norm() / (0.5 * vpm)) as usize;
                        (0..steps.saturating_sub(5)).all(|k| {
                            let p = at + (target - at) * (k as f64 / steps.max(1) as f64);
                            self.world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64) == worldgen::material::AIR
                        })
                    };
                    let tries = [0.0, -2.0 * from, 0.5, -2.0 * from - 0.5, -0.5, -2.0 * from + 0.5, 1.0, -2.0 * from - 1.0];
                    let at = [1.0, 0.75, 0.55].into_iter().flat_map(|f| tries.map(|t| spot(from + t, distance * f))).find(|&at| clear(at)).unwrap_or_else(|| spot(from, distance));
                    *held = Some((at, target));
                    if f == 20 {
                        println!("frame {frame}: watching from {:.1} m off what's at {:?}", (at - target).norm() / vpm, (target / vpm).map(|v| (v * 10.0).round() / 10.0));
                    }
                }
                // Looking at its feet, or `view_look=2.5` that many meters up from them, at the middle
                // of something that floats
                if let Some((at, target)) = *held {
                    self.camera.position = voxels_to_camera(at, self.world.size);
                    self.camera.look_at(voxels_to_camera(target + Vector3::new(0.0, 0.0, number("view_look", 0.0) * vpm), self.world.size));
                }
                self.game.set_health(100.0);
                let pressing = number("view_attack", 0.0) > 0.5 && f > 30;
                self.script_controls = combat::Controls { attack: pressing, holding: pressing, ..Default::default() };
                if f % 60 == 0 {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
            // Circles round the nearest dragon, looking at it, to see it from every side as it goes
            // about: `--set view_distance=20` that many meters off rather than as far as its wings
            // reach twice over, `--set view_turn=0.01` radians round a frame, and `--set
            // view_up=0.3` that much of the way up
            ("dragon", f) => {
                let number = |key: &str, default: f64| settings_from_args().into_iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(default);
                if let Some((target, reach)) = self.game.nearest_dragon(eye) {
                    let distance = number("view_distance", reach * 2.1) * vpm;
                    let round = number("view_turn", 0.008) * f as f64 + number("view_from", 0.6);
                    let up = number("view_up", 0.3);
                    let at = target + Vector3::new(round.cos(), round.sin(), up) * distance;
                    let floor = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64 + 1.5 * vpm;
                    self.camera.position = voxels_to_camera(Vector3::new(at.x, at.y, at.z.max(floor)), self.world.size);
                    self.camera.look_at(voxels_to_camera(target, self.world.size));
                }
                if f % 60 == 0 {
                    println!("frame {frame}: {}", self.game.report(eye));
                }
            }
            // The same round the nearest avian or gargoyle, see avians.rs, as close as `--set
            // view_distance=` says or twice as far as its wings reach, as `view_up=` high; or with
            // `--set view_fixed=1` from where the camera is, only looking at it
            ("aerie", f) => {
                let number = |key: &str, default: f64| settings_from_args().into_iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse::<f64>().ok()).unwrap_or(default);
                if let Some((target, reach)) = self.game.nearest_winged(eye) {
                    if number("view_fixed", 0.0) == 0.0 {
                        let distance = number("view_distance", reach * 2.0) * vpm;
                        let round = number("view_turn", 0.006) * f as f64 + number("view_from", 0.6);
                        let up = number("view_up", 0.25);
                        let at = target + Vector3::new(round.cos(), round.sin(), up) * distance;
                        let floor = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64 + 1.5 * vpm;
                        self.camera.position = voxels_to_camera(Vector3::new(at.x, at.y, at.z.max(floor)), self.world.size);
                    }
                    self.camera.look_at(voxels_to_camera(target, self.world.size));
                }
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
                // Let go now and then, for a power aimed with right click held to go off
                self.script_controls = combat::Controls { heavy: true, aiming: f % 40 != 39, ..Default::default() };
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
                // `--set fire_once=1` shoots once, and `fire_turn=0.2` then turns that far to the side,
                // so where the shot lands isn't behind the gun, for shots of a shell bursting
                let setting = |key: &str| settings_from_args().into_iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse::<f64>().ok());
                let firing = f >= 30 && (setting("fire_once") != Some(1.0) || f <= 31);
                self.script_controls = combat::Controls { attack: firing, holding: firing, ..Default::default() };
                if f == 34
                    && let Some(turn) = setting("fire_turn")
                {
                    self.camera.rotation.z += turn;
                }
                if f % 100 == 0 {
                    println!("frame {frame}: {} holes blown", self.craters.len());
                }
            }
            // Three hand cannon shells coming down on the ground ahead one after another, a small,
            // an ordinary and a big one, left to right, `--set blasts_every=60` frames apart, for
            // shots of each size, see Combat::drop_shell()
            ("blasts", f) => {
                if f == 1 {
                    self.camera.rotation.x = -0.3;
                }
                // Stirred too little to see, so the picture isn't built up over still frames, which
                // would blur away a blast that's over in a moment
                self.camera.rotation.z += if f % 2 == 0 { 1e-5 } else { -1e-5 };
                let every = settings_from_args().into_iter().find(|(k, _)| k == "blasts_every").and_then(|(_, v)| v.parse::<usize>().ok()).unwrap_or(60);
                for (k, size) in [0.55, 1.0, 1.5].into_iter().enumerate() {
                    if f == 30 + every * k {
                        let flat = Vector3::new(look.x, look.y, 0.0).normalize();
                        let right = Vector3::new(flat.y, -flat.x, 0.0);
                        let at = eye + flat * 10.0 * vpm + right * (k as f64 - 1.0) * 6.0 * vpm;
                        let ground = (0..self.world.size[2] as i64).rev().find(|&z| !worldgen::is_passable(self.world.material_at(at.x as i64, at.y as i64, z))).unwrap_or(0) as f64;
                        self.game.drop_shell(Vector3::new(at.x, at.y, ground + 1.0), size);
                        println!("frame {frame}: a shell {size} times as big coming down");
                    }
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
            (name, f) if building::SCRIPTS.contains(&name) => self.bases_script(name, f),
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

    /// `--script summons`, a necromancer's dead raised and fighting, for contact sheets, see
    /// combat\signatures\summons.rs: the Bone Archers, the Bone Phalanx and the Grave Colossus
    /// raised ahead, eight of the dead, going for the nearest of a gang of bandits there. Then with
    /// `--set summons_part=strike` the player turns to a bandit off to the right and strikes it with
    /// a bolt of their own, and all of them turn on it; with `summons_part=behind` a crossbowman
    /// behind shoots the player in the back, and all of them turn on that, the player turning to
    /// see; with `summons_part=none` the same fight without them. A shot every `--set
    /// summons_every=10` frames from frame 280 to 600, and what they're
    /// after said every 15. Best with `--step 0.016667`.
    fn summons_step(&mut self, f: usize, eye: Vector3<f64>) {
        let settings = settings_from_args();
        let setting = |key: &str| settings.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
        let part = setting("summons_part").unwrap_or_else(|| "strike".into());
        let every: usize = setting("summons_every").and_then(|v| v.parse().ok()).unwrap_or(10);
        let vpm = self.world.voxels_per_meter as f64;
        let feet = self.player.as_ref().map_or(eye - Vector3::new(0.0, 0.0, 1.6 * vpm), |p| p.position);
        if f == 1 {
            if self.player.is_none() {
                self.player = Some(Player::new(eye, &self.world));
            }
            self.camera.rotation.x = -0.32;
        }
        if f == 2 {
            // Which way the player faces, kept for where everything's put
            self.play.heading = self.camera.rotation.z + std::f64::consts::FRAC_PI_2;
            let ahead = Vector3::new(self.play.heading.cos(), self.play.heading.sin(), 0.0);
            let right = Vector3::new(ahead.y, -ahead.x, 0.0);
            let water = &self.water;
            let level = |x: f64, y: f64| water.surface_at(x, y);
            let mut foes = Vec::new();
            for k in 0..3 {
                foes.extend(self.game.place_foe(&self.world, &level, animals::Look::Bandit, feet + (ahead * (11.0 + k as f64) + right * (k as f64 - 1.0) * 1.6) * vpm));
            }
            // Further off than the gang, so it's not what they'd go for themselves
            foes.extend(self.game.place_foe(&self.world, &level, animals::Look::Bandit, feet + (right * 16.0 + ahead * 2.0) * vpm));
            // Behind, or behind to one side or the other if that's water
            let behind = [0.0, 0.6, -0.6, 1.1, -1.1].into_iter().find_map(|side| self.game.place_foe(&self.world, &level, animals::Look::Crossbowman, feet + (right * side - ahead) * 16.0 * vpm));
            if behind.is_none() {
                println!("frame {f}: no crossbowman put down behind");
            }
            foes.extend(behind);
            self.play.foes = foes;
            println!("frame {f}: foes {:?}", self.play.foes);
        }
        let flat_of = |heading: f64| Vector3::new(heading.cos(), heading.sin(), 0.0);
        // The three summonings, one after another
        // Each used once the weapon's up in hand
        let casting = [(4, "archers_crystal"), (90, "phalanx_binding"), (176, "colossus_focus")];
        let mut controls = combat::Controls::default();
        // `summons_part=none`: the same without any of the dead, to weigh what they cost to draw
        for (from, id) in casting.into_iter().filter(|_| part != "none") {
            if f == from && !self.game.hold_part(id) {
                println!("frame {f}: no part {id}");
            }
            if (from + 1..from + 86).contains(&f) {
                controls = self.game.power_controls((f as f64 - from as f64 - 30.0) / 60.0, 1.0 / 60.0);
            }
        }
        let foe = |k: usize, app: &Self| app.play.foes.get(k).and_then(|&id| app.game.foe_at(id));
        match part.as_str() {
            "behind" => {
                // Roused, shooting the player in the back, and the player turning to see a moment
                // after
                if f == 290
                    && let Some(&id) = self.play.foes.get(4)
                {
                    self.game.rouse(id);
                }
                if f >= 330
                    && let Some(at) = foe(4, self)
                {
                    self.camera.look_at(voxels_to_camera(at, self.world.size));
                    self.camera.rotation.x = self.camera.rotation.x.min(-0.12);
                }
            }
            _ => {
                // Turned to the one off to the right and striking it
                if f >= 300
                    && let Some(at) = foe(3, self)
                {
                    self.camera.look_at(voxels_to_camera(at, self.world.size));
                    if (310..322).contains(&f) {
                        controls = combat::Controls { attack: true, holding: true, ..Default::default() };
                    }
                }
            }
        }
        if f < 300 && f > 2 {
            let face = flat_of(self.play.heading);
            self.camera.look_at(voxels_to_camera(eye + face * 10.0 * vpm - Vector3::new(0.0, 0.0, 3.2 * vpm), self.world.size));
        }
        self.script_controls = controls;
        if f % 15 == 0 {
            println!("frame {f}: {}", self.game.summons_line());
        }
        if (280..=600).contains(&f)
            && (f - 280) % every.max(1) == 0
            && let Some(capture) = &mut self.capture
        {
            capture.shots.push(f + 2);
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
            // Not sharks, which only ever swim, see animals\sealife.rs
            let monsters: Vec<animals::Look> = animals::Look::ALL.into_iter().filter(|l| l.is_monster() && !l.is_sea()).collect();
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
                // Built in the background, as the strips are, giving way to the game
                worldgen::give_way();
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
        self.builds_pipeline.maybe_reload();
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
        // A grappling hook's rope, see rope.rs, and the wings worn, see wings.rs
        let mut rope = if self.hook.is_some() || self.game.holding_hook() { self.rope_instances(&view) } else { Vec::new() };
        rope.extend(self.wing_instances(&view));
        // The wards round raid bosses, see guardians.rs
        rope.extend(self.ward_instances());
        let mut rope_lights = self.rope_lights(&view);
        // And wards breaking, see guardians.rs
        rope_lights.extend(self.ward_lights());
        // How white the screen is as the winds bring the player back up, see rope.rs, and whether
        // they're gliding
        let fallen = self.fall_dark();
        // The look of the realm the world is, see session.rs
        let realm_look = self.realm_look();
        let gliding_now = self.player.as_ref().is_some_and(|p| p.gliding && !p.on_ground());
        // The torches and lamps built, and where what's picked to build would go, see building.rs
        let base_lights = building::base_lights(&self.bases, &self.world);
        let ghost = if self.building.on && !self.menu_open { self.ghost() } else { None };
        // And what G would gather where the player's looking
        self.update_prompt(!self.menu_open && !self.inventory_open && !self.game.shopping() && !self.session.gate_open && !self.map.open);
        let gate_choice = self.gate_choice();
        let hearth_note = if self.menu_open || self.building.on { None } else { self.hearth_note() };
        let raid_note = self.raid_note();
        // The compass in Realms, and the map, see compass.rs
        let bearing = if self.menu_open || self.session.gate_open || self.map.open { None } else { self.compass() };
        let map_marks = (self.map.open && !self.menu_open).then(|| self.map_marks());
        // How many of the realm's guardians are slain, for the gate's window, see guardians.rs
        let guardians = self.guardians_slain();
        // And hints and messages aren't half hidden behind it or any other window
        self.game.set_covered(map_marks.is_some() || self.skills_open || self.inventory_open || self.session.gate_open || self.game.shopping());

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
        // What the map beside the menu's to show, as the menu was last frame, see loading::Backdrop
        let map_wanted = self.backdrop.as_ref().filter(|_| self.menu_open && !self.started).map(|_| self.map_wanted());
        // For the signposts' words, see compass.rs
        let landscape = self.terrain_settings();
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

        let mut reread_models = false;
        // The land behind the menu's gone for good once a game's begun
        if self.started {
            self.backdrop = None;
        }
        rcx.gui.immediate_ui(|gui| {
            let ctx = gui.context();
            if self.editor.open {
                reread_models = self.editor.ui(&ctx, &mut self.game);
            }

            // Dark while the world's built afresh round the camera, gone too fast for the strips to
            // keep up, see stream.rs
            if let Some(teleport) = self.stream.as_mut().and_then(|s| s.teleport.as_mut())
                && teleport.dark > 0.0
            {
                let dark = teleport.dark.clamp(0.0, 1.0);
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("moving on")));
                painter.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha((dark * 255.0) as u8));
                // Going on with a game somewhere the world wasn't, which takes a few seconds, the
                // loading screen's shown, the land there rising on it as it's built
                if let Some(board) = &mut teleport.board {
                    board.paint(&ctx, dark, !self.focused);
                }
            }
            // Wings spread, said under the crosshair too
            if gliding_now && !self.menu_open {
                ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("gliding"))).text(
                    ctx.screen_rect().center() + egui::vec2(0.0, 42.0),
                    Align2::CENTER_CENTER,
                    "Gliding",
                    egui::FontId::proportional(16.0),
                    Color32::from_rgba_unmultiplied(235, 240, 255, 200),
                );
            }
            if let Some(bearing) = &bearing {
                compass::draw_compass(&ctx, bearing);
            }
            if let Some(marks) = &map_marks {
                compass::draw_map(&ctx, &mut self.map, marks);
            }
            // Brought back up after a fall below the sky isles, see rope.rs
            if fallen > 0.0 {
                ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("fallen")))
                    .rect_filled(ctx.screen_rect(), 0.0, Color32::from_rgba_unmultiplied(225, 235, 250, (fallen * 255.0) as u8));
            }
            // Going through the gate or a portal: the light of its veil, then the dark
            let leaving = self.session.leaving_dark();
            if leaving > 0.0 {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("going through")));
                let violet = (1.0 - (leaving * 2.0 - 1.0).abs()) * 140.0;
                painter.rect_filled(ctx.screen_rect(), 0.0, Color32::from_rgba_unmultiplied(150, 110, 255, violet as u8));
                painter.rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha((leaving * leaving * 255.0) as u8));
            }

            // A new character being made, before the game they're for, see character\creator.rs
            if self.menu_open
                && let Some(creator) = &mut self.session.creator
            {
                if let Some(pick) = character::creator::window(&ctx, creator) {
                    self.session.creator_pick = Some(pick);
                }
            } else if self.menu_open {
                // Dim the world behind the menu a little
                ctx.layer_painter(egui::LayerId::background())
                    .rect_filled(ctx.screen_rect(), 0.0, Color32::from_black_alpha(60));
                // And the map of the world about to be entered over it, see loading::Backdrop
                if let (Some(backdrop), Some(wanted)) = (&mut self.backdrop, &map_wanted) {
                    backdrop.paint(&ctx, wanted);
                }
                // Resume, each mode with Continue and New, Settings and Quit, done at the start of
                // the next frame, see menu_picked(), and how to play under them
                session::menu_window(&ctx, map_wanted.is_some(), |ui| {
                    let session = &mut self.session;
                    let picked = session::menu(&mut session.menu, ui, self.started, session.mode, &session.saves, &mut self.new_seed, self.show_settings);
                    if picked.is_some() {
                        session.action = picked;
                    }
                });
            }

            if !self.menu_open && !self.editor.open && self.loaded_scene.is_terrain() && self.animals_on {
                self.game.hud(&ctx, &view);
                // Drawn only while it's open, so it costs nothing shut
                if self.skills_open && !self.game.skill_window(&ctx) {
                    self.skills_open = false;
                    self.skills_closed = true;
                }
                if !self.fixed_sun {
                    self.cycle.hud(&ctx, raid_note.as_deref());
                }
                night::hud(&self.raid, &self.bases, &self.world, &ctx, &view);
                compass::draw_signposts(&ctx, &view, &self.world, &landscape);
                building::hud(&self.building, &self.bases, ghost.as_ref(), gate_choice, &self.world, &ctx, &view);
                hearths::hud(&ctx, hearth_note.as_deref());
                // A merchant's stall closed, or walked away from: back to playing
                let shopping = self.game.shopping() && self.game.shop_window(&ctx);
                if self.shopping && !shopping && !self.inventory_open && self.capture.is_none() {
                    focus_change = Some(true);
                }
                self.shopping = shopping;
                // The Innert Gate's window, or a portal's, see session.rs
                if self.session.gate_open
                    && let (Some(progress), Some(places)) = (&self.session.realms, &self.session.places)
                {
                    let carried = std::array::from_fn(|r| self.game.innert_blocks(r as u8));
                    if let Some(action) = session::gate_window(&ctx, places.realm, progress, carried, guardians) {
                        self.session.gate_action = Some(action);
                    }
                }
                if !shopping && self.inventory_open {
                    let mut open = true;
                    // With what's been gathered to make tools from, see tools.rs
                    let mut crafting = tools::Crafting { open: self.building.craft_open, materials: self.bases.materials };
                    self.game.inventory(&ctx, &mut open, &mut crafting);
                    (self.building.craft_open, self.bases.materials) = (crafting.open, crafting.materials);
                    if !open {
                        self.inventory_open = false;
                        focus_change = Some(true);
                    }
                }
            }

            // The Controls window, over the menu or the game, see bindings.rs; shut while playing,
            // the mouse is taken back
            if self.keys.open {
                self.keys.window(&ctx);
                if !self.keys.open && !self.menu_open && !self.inventory_open && self.capture.is_none() {
                    focus_change = Some(true);
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
                // The keys as they're bound, changed in the Controls window, see bindings.rs
                use bindings::{Action as A, key, keys_together};
                if ui.button(RichText::new("Controls").size(15.0)).clicked() {
                    self.keys.open = true;
                    focus_change = Some(false);
                }
                let wasd = keys_together(&[A::Forward, A::Left, A::Back, A::Right]);
                ui.label(if self.player.is_some() {
                    format!("Walking: {wasd} to move, {} to jump or swim up, {} to run, {} to fly. With wings on, hold {} in the air to glide.", key(A::Jump), key(A::Run), key(A::Fly), key(A::Jump))
                } else {
                    format!("Flying: {} to move, {} to go faster, {} to walk", keys_together(&[A::Forward, A::Left, A::Back, A::Right, A::FlyUp, A::FlyDown]), key(A::Run), key(A::Fly))
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
                        .text(format!("Move Speed (hold {} to go faster)", key(A::Run))),
                );

                egui::CollapsingHeader::new("Lighting")
                    .default_open(true)
                    .show(ui, |ui| {
                        // The day going round, or the sun held where the sliders put it
                        ui.checkbox(&mut self.fixed_sun, "Hold the Sun Still");
                        if self.fixed_sun {
                            ui.add(egui::Slider::new(&mut self.sun_azimuth, 0.0..=360.0).text("Sun Azimuth"));
                            ui.add(egui::Slider::new(&mut self.sun_elevation, -10.0..=90.0).text("Sun Elevation"));
                        } else {
                            let mut hour = self.cycle.hour();
                            if ui.add(egui::Slider::new(&mut hour, 0.0..=23.99).text("Time of Day")).changed() {
                                self.cycle.set_hour(hour);
                            }
                            ui.checkbox(&mut self.cycle.clock.running, "Day Goes Round");
                        }
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
        // Asked for in the model editor
        if reread_models {
            let allocator = self.memory_allocator.clone();
            self.model_files.reload_all(self.game.model_lists(), move |words| model_buffer(&allocator, words));
            anim::forget();
        }

        if let Some(focused) = focus_change {
            self.focused = focused;
            grab_cursor(&rcx.window, focused);
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

        // Towards whichever of the sun and the moon lights the world, see daynight.rs
        let sky = if self.fixed_sun { daynight::DayNight::fixed_sky(self.sun_azimuth as f64, self.sun_elevation as f64) } else { self.cycle.sky() };
        let sun_direction = [sky.key.x as f32, sky.key.y as f32, sky.key.z as f32, self.world.sea_level.unwrap_or(0.0)];
        // Changes when the sun's put somewhere else rather than moving on a step with the day, for
        // the light kept for it to be thrown away
        let sky_epoch = if self.fixed_sun {
            1 << 32 | (sun_direction[0].to_bits() ^ sun_direction[1].to_bits().rotate_left(11) ^ sun_direction[2].to_bits().rotate_left(22)) as u64
        } else {
            self.cycle.jumps as u64
        };

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
        // Sun shafts on High do the sunbeams' job better. By moonlight there are none.
        let shafts = self.shafts.in_the_haze() && !sky.moonlit;
        let sunbeams = self.sunbeams && !shafts;
        if sunbeams {
            flags |= FLAG_SUNBEAMS;
        }
        if shafts {
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

        // A model file saved: its models worked out again and packed on a thread of their own,
        // then swapped in here, see assets.rs
        let allocator = self.memory_allocator.clone();
        let game = &self.game;
        if let Some(done) = self.model_files.poll(|| game.model_lists(), move |words| model_buffer(&allocator, words)) {
            if let Some(buffer) = done.made {
                self.animal_models = buffer;
            }
            if let Some(wings) = done.wings {
                self.game.set_wing_models(wings);
            }
            if let Some(looks) = done.weapons {
                self.game.set_weapon_looks(looks);
            }
        }

        // The weapon in hand drawn finer once it's made so, see game\fine.rs
        self.game.fine_held();
        // A weapon was found, so there's a new model to draw
        if self.game.models_version() != self.models_version {
            let start = Instant::now();
            self.models_version = self.game.models_version();
            let words = self.game.weapon_model_words();
            let packed = start.elapsed().as_secs_f64() * 1000.0;
            self.weapon_models = model_buffer(&self.memory_allocator, words);
            if self.capture.is_some() {
                println!("Models sent in {:.1} ms, {packed:.1} ms of it packing them", start.elapsed().as_secs_f64() * 1000.0);
            }
        }

        // The weapon in hand, and the animals, food and things flying near the camera, for the
        // shader to trace
        let playing = self.loaded_scene.is_terrain() && self.animals_on;
        let mut instances: Vec<AnimalInstance> = if self.editor.open {
            let eye = camera_to_voxels(self.camera.position, self.world.size);
            self.editor.instances(&self.game, self.world.voxels_per_meter as f64, eye, self.camera.rotation.z, self.camera.rotation.x)
        } else if playing {
            self.game.instances(&self.world, &view)
        } else {
            Vec::new()
        };
        // A grappling hook's rope, and wings spread to glide, see rope.rs
        if !self.editor.open {
            instances.extend(rope);
            // Trees coming down, see felling.rs
            instances.extend(building::felling_instances(&self.building, &self.game, &self.world));
        }
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
        let vpm = self.world.voxels_per_meter as f32;
        let seconds = self.start_time.elapsed().as_secs_f32();
        // The roads' lamps only after dark, see worldgen\roads.rs, so by day they take none of the
        // lights there's room for
        let day = daynight::daylight(sky.sun.z) as f32;
        let lit = |l: &[f32; 8]| {
            let k = worldgen::by_night([l[4], l[5], l[6]], day);
            (k > 0.0).then(|| {
                let mut l = *l;
                for c in &mut l[4..7] {
                    *c *= k;
                }
                l
            })
        };
        let still = still_lights(self.lights.iter().chain(base_lights.iter()).filter_map(lit).map(|l| flicker(l, seconds, vpm)), camera_voxels, vpm);
        let mut fighting = if playing { self.game.lights(&view, sky.night) } else { Vec::new() };
        fighting.extend(rope_lights);
        fighting.sort_by(|a, b| combat::light_rank(a, view.eye).total_cmp(&combat::light_rank(b, view.eye)));
        fighting.truncate(FIGHTING_LIGHTS);
        let lights: Vec<[f32; 8]> = fighting.into_iter().chain(still).collect();
        let light_buffer = self.instance_allocator.allocate_slice::<[f32; 8]>(lights.len().max(1) as u64).unwrap();
        light_buffer.write().unwrap()[..lights.len()].copy_from_slice(&lights);

        // The view, for traverse.comp and shafts.comp
        let previous_world_to_pixel = self
            .last_view
            .as_ref()
            .map_or(Matrix4::identity(), |last| world_to_pixel(&last.pixel_to_ray.cast()));
        let view_buffer = self.instance_allocator.allocate_sized::<View>().unwrap();
        *view_buffer.write().unwrap() = View::new(pixel_to_ray.cast(), previous_world_to_pixel.cast(), &sky, realm_look);

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
        // Last frame can be reprojected onto this one as long as only the camera moved. The sun moving
        // on a step, see daynight.rs, only changes the light a little: what was there is let go of
        // over a few frames rather than all at once.
        let history_limit = match &self.last_view {
            _ if self.reset_accum => 0,
            Some(last) if *last == view && !instances.is_empty() => ANIMATED_HISTORY,
            Some(last) if *last == view => STILL_HISTORY,
            Some(last) if (last.sun_direction, last.render_mode, last.flags, last.haze) == (view.sun_direction, view.render_mode, view.flags, view.haze) => MOVING_HISTORY,
            Some(last) if (last.render_mode, last.flags, last.haze) == (view.render_mode, view.flags, view.haze) && self.sky_epoch == sky_epoch => SUN_STEP_HISTORY,
            _ => 0,
        };
        self.sky_epoch = sky_epoch;
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
        // Light gathered under another sun, or in another world, is no good. The sun moving on with
        // the day only nudges it along, see daynight.rs.
        let cache_for = (sky_epoch, self.terrain.seed);
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
        }
        // Holes blown and stains splashed since the last frame, after what's come of strips, as
        // they're newer
        Self::record_edits(&mut self.edit_uploads, &self.voxel_set, &self.water, &mut builder);
        if let Some(stream) = &mut self.stream {
            stream.skips.record(&mut builder, &self.skips_pipeline, &self.voxel_set.views[0].1, &self.descriptor_set_allocator);
        }
        // Empty bricks round what's just been built skip no further than it, see building.rs
        let busy = self.stream.as_ref().is_some_and(|s| s.skips.busy());
        building::record_reskips(&mut self.building, &mut builder, &self.builds_pipeline, &self.voxel_set.views[0].1, &self.descriptor_set_allocator, &self.world, busy);
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

        // The empty world Innert starts on isn't drawn at all behind its menu, which has the
        // loading screen's page behind the map instead, see loading::Backdrop
        let bare = self.bare && self.menu_open;
        let shaded = self.render_mode == RenderMode::Shaded && !bare;
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
        if !bare {
            unsafe {
                builder
                    .dispatch([
                        render_extent[0].div_ceil(8),
                        render_extent[1].div_ceil(8),
                        1,
                    ])
                    .unwrap();
            }
        }
        if let Some(timer) = &mut timer {
            timer.mark(&mut builder, "trace");
        }

        if shaded && sky.moonlit {
            self.shafts.moonlit();
        } else if shaded {
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
                exposure: sky.exposure as f32,
                night: sky.night as f32,
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

        let submitting = Instant::now();
        // Sent to the GPU now, before the semaphore it's drawn behind and the present: left to the
        // present, now and then it went wrong inside vulkano, which let go of the frame's hold on
        // its images without having taken it and stopped the game ("Image was not locked for GPU
        // write"), on AVI as it was too, a few hundred frames into a run.
        // And before a shot's copy of the screen goes behind it, with `--set gui=1`: sent together
        // with that, vulkano checked what the frame's commands use against the GUI's commands they
        // were chained behind, which answer only for what they use themselves, so all the rest was
        // checked against who holds it, and found still held by the frame before whenever the GPU
        // wasn't done with that yet, as with `nowait`. That stopped the game ("the resource is
        // already in use, and there is no tracking of concurrent usages"). Sent on their own, the
        // frame's commands are checked against themselves, as on every other frame.
        if let Err(e) = gui_future.flush() {
            println!("Sending the frame to the GPU went wrong: {e}");
        }
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
                let copied = gui_future.then_execute(self.queue.clone(), copy.build().unwrap()).unwrap();
                if let Err(e) = copied.flush() {
                    println!("Sending the shot's copy of the screen to the GPU went wrong: {e}");
                }
                copied.boxed()
            }
            None => gui_future,
        };
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
                self.water.report(&self.command_buffer_allocator, &self.queue);

                if capture.stats_from > 0 {
                    println!("Frame times from frame {} on, once the world's there", capture.stats_from);
                }
                let mut times: Vec<f64> = capture.frame_times.iter().skip(capture.stats_from).map(|t| t.as_secs_f64() * 1000.0).collect();
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
                    let mut later: Vec<f64> = capture.frame_times.iter().skip(capture.stats_from + 10).map(|t| t.as_secs_f64() * 1000.0).collect();
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
                        let long: Vec<String> = capture.frame_times.iter().enumerate().skip(capture.stats_from + 10).filter(|(_, t)| t.as_secs_f64() > 0.025).map(|(k, t)| format!("{}: {:.0} ms", k + 1, t.as_secs_f64() * 1000.0)).collect();
                        println!("long frames: {}", long.join(", "));
                    }
                }
                if let Some(timer) = &mut capture.gpu {
                    println!("{}", timer.report());
                }
                // A mode being played is saved on the way out, as in play
                if self.session.mode.is_some() {
                    self.save_game(true);
                    println!("STATE at the end: {}", self.state_line());
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
        .with_title(if editor::is_app() { "Innert Model Editor" } else { "Innert" })
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
        // Another thread's gone wrong, and the game ends as soon as that's been said: saved first,
        // it goes no further meanwhile, see crash.rs
        if crash::round(|| self.emergency_save()) {
            std::thread::sleep(Duration::from_millis(50));
            return;
        }
        if let Some(capture) = &self.capture {
            crash::test(capture.frame_times.len());
        }
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
            fn ClipCursor(rect: *const std::ffi::c_void) -> i32;
        }
        const MB_ICONERROR: u32 = 0x10;
        // Over the game, from whichever thread went wrong, with the mouse free to click it away
        const MB_SETFOREGROUND: u32 = 0x10000;
        const MB_TOPMOST: u32 = 0x40000;
        let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
        unsafe {
            ClipCursor(std::ptr::null());
            MessageBoxW(std::ptr::null_mut(), wide(text).as_ptr(), wide("Innert").as_ptr(), MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST);
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::JoinHandle;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut c_void;
        fn SetStdHandle(which: u32, handle: *mut c_void) -> i32;
        fn CreatePipe(read: *mut *mut c_void, write: *mut *mut c_void, attributes: *mut c_void, size: u32) -> i32;
        fn ReadFile(file: *mut c_void, buffer: *mut u8, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
        fn WriteFile(file: *mut c_void, buffer: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    /// Room in the pipe for what's printed while the file can't keep up
    const PIPE_BYTES: u32 = 8 << 20;

    /// The file's handle and the pipe's write end, as numbers so they can be shared, and the thread
    /// writing to the file
    struct Printing {
        file: usize,
        write: usize,
        writer: JoinHandle<()>,
    }
    static STATE: Mutex<Option<Printing>> = Mutex::new(None);
    /// Whether that thread is waiting for more to be printed, having written out all there was
    static WAITING: AtomicBool = AtomicBool::new(false);

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
                WAITING.store(true, Ordering::SeqCst);
                let read_ok = unsafe { ReadFile(read as *mut c_void, buffer.as_mut_ptr(), buffer.len() as u32, &mut got, std::ptr::null_mut()) };
                WAITING.store(false, Ordering::SeqCst);
                if read_ok == 0 || got == 0 {
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
        *state = Some(Printing { file, write: write as usize, writer });
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
    /// whatever panicked may have been in the middle of printing to: till the thread writing it
    /// has been waiting for more a little while, or a second at most. Not by peeking into the pipe,
    /// which waited for ever behind that thread's own wait for more, and the game with it.
    pub fn drain() {
        if !STATE.try_lock().ok().is_some_and(|state| state.is_some()) {
            return;
        }
        let since = std::time::Instant::now();
        let mut idle = None;
        while since.elapsed() < std::time::Duration::from_secs(1) {
            if !WAITING.load(Ordering::SeqCst) {
                idle = None;
            } else if idle.get_or_insert_with(std::time::Instant::now).elapsed() > std::time::Duration::from_millis(30) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
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

/// The model editor's own app, innert-editor: the game's renderer on a plain studio, opening
/// straight into the editor, see editor.rs
pub fn editor_main() {
    editor::set_app();
    main();
}

/// The game
pub fn main() {
    if std::env::args().any(|a| a == "--screenshot") {
        perf::count_heap();
    }
    // `--export-models <dir>` writes every model as a MagicaVoxel file and stops, see assets.rs
    if let Some(code) = assets::export_from_args() {
        std::process::exit(code);
    }
    startup::init();
    // `--set vram_budget=2048` takes the graphics card to have no more memory for the game than
    // that, in MB, as a smaller card would
    if let Some(megabytes) = settings_from_args().iter().find(|(k, _)| k == "vram_budget").and_then(|(_, v)| v.parse().ok()) {
        perf::cap_vram(megabytes);
    }
    // `--set no_arena=1` builds the land without the Drowned Ring, to measure what it costs
    if settings_from_args().iter().any(|(k, v)| k == "no_arena" && v == "1") {
        worldgen::leave_out_arenas();
    }
    // Decoding the sounds on a thread of their own, done long before anything's heard
    audio::preload();
    // A panic anywhere is said and ends the game, see crash.rs
    crash::install();
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Walking down a cave lit by a torch every few meters, far more than STILL_LIGHTS of them in
    /// sight, the lights sent never go over STILL_LIGHTS, the nearest is always lit full, and none
    /// jumps in brightness from one step to the next: each comes and goes smoothly
    #[test]
    fn many_torches_come_and_go_smoothly() {
        let vpm = 16.0f32;
        let torch = |k: usize| [k as f32 * 4.0 * vpm, 0.0, 2.0 * vpm, 9.0 * vpm, 1.0, 0.55, 0.22, 0.0];
        let torches: Vec<[f32; 8]> = (0..60).map(torch).collect();
        let mut last: Option<Vec<([f32; 3], f32)>> = None;
        for step in 0..400 {
            let eye = Vector3::new(step as f32 * 0.25 * vpm, 0.5 * vpm, 1.7 * vpm);
            let lights = still_lights(torches.iter().copied(), eye, vpm);
            assert!(lights.len() <= STILL_LIGHTS);
            let nearest = torches.iter().map(|l| (Vector3::new(l[0], l[1], l[2]) - eye).norm()).fold(f32::MAX, f32::min);
            assert!(lights.iter().any(|l| (Vector3::new(l[0], l[1], l[2]) - eye).norm() == nearest && l[4] == 1.0), "the nearest isn't lit full {step} steps in");
            let now: Vec<([f32; 3], f32)> = lights.iter().map(|l| ([l[0], l[1], l[2]], l[4])).collect();
            if let Some(before) = &last {
                for (at, bright) in torches.iter().map(|l| [l[0], l[1], l[2]]).map(|at| {
                    let of = |v: &Vec<([f32; 3], f32)>| v.iter().find(|(p, _)| *p == at).map_or(0.0, |(_, b)| *b);
                    (at, (of(&now), of(before)))
                }) {
                    assert!((bright.0 - bright.1).abs() < 0.25, "the torch at {at:?} jumps from {} to {} {step} steps in", bright.1, bright.0);
                }
            }
            last = Some(now);
        }
    }

    /// Flames flicker a little and lanterns less, round how bright they are; crystals and the glows
    /// of fallen cities don't
    #[test]
    fn flames_flicker_crystals_dont() {
        let vpm = 16.0;
        let swing = |color: [f32; 3]| {
            let l = [100.0, 200.0, 300.0, 144.0, color[0], color[1], color[2], 0.0];
            let seen: Vec<f32> = (0..200).map(|k| flicker(l, k as f32 * 0.037, vpm)[4] / color[0]).collect();
            seen.iter().copied().fold(0.0f32, f32::max) - seen.iter().copied().fold(f32::MAX, f32::min)
        };
        let (torch, lantern) = (swing([1.0, 0.55, 0.22]), swing([1.0, 0.71, 0.36]));
        assert!(torch > 0.15 && torch < 0.45, "a torch flickers by {torch}");
        assert!(lantern > 0.02 && lantern < torch * 0.5, "a lantern flickers by {lantern}, a torch by {torch}");
        assert_eq!(swing([0.35, 0.85, 1.0]), 0.0);
        assert_eq!(swing([-0.2, -0.3, -0.1]), 0.0);
    }
}
