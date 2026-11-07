//! Sun shafts, see shaders\shafts.comp: what it keeps from frame to frame, and which of its stages
//! run when. `--set sun_shafts=off`, `low` or `high`, and `--set sun_shafts_strength=1.5` for more.

use std::sync::Arc;

use egui_winit_vulkano::egui;
use nalgebra::{Matrix4, Vector3};
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{AutoCommandBufferBuilder, ClearColorImageInfo, PrimaryAutoCommandBuffer},
    descriptor_set::{DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator},
    format::{ClearColorValue, Format},
    image::{Image, ImageCreateInfo, ImageUsage, view::ImageView},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{Pipeline, PipelineBindPoint},
};

use crate::gpu_timing::GpuTimer;
use crate::hot_reload::HotReloadComputePipeline;

/// How sun shafts are drawn
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    Off,
    /// The sky smeared towards the sun on screen, which only shows looking towards it
    Low,
    /// Shafts in the air itself, wherever sunlight reaches it
    High,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Off, Quality::Low, Quality::High];

    /// `off`, `low` or `high`, as `--set sun_shafts=` takes them
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "off" | "0" | "false" | "none" => Some(Quality::Off),
            "low" => Some(Quality::Low),
            "high" | "1" | "true" | "on" => Some(Quality::High),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Quality::Off => "Off",
            Quality::Low => "Low",
            Quality::High => "High",
        }
    }
}

// Has to match the stages in shaders\shafts.comp
#[derive(Clone, Copy)]
enum Stage {
    Fill,
    Shade,
    March,
    Resolve,
    Mask,
    Radial,
    Composite,
}

// Has to match the FLAG_* constants in shaders\shafts.comp
const FLAG_HISTORY: u32 = 1;
const FLAG_LOW: u32 = 2;
const FLAG_STILL: u32 = 4;
const FLAG_FINE: u32 = 8;

/// Texels along each side of the map of where sunlight stops, has to match shaders\shafts.comp
const MAP_SIZE: u32 = 1024;
/// Meters across a texel of it: it covers MAP_SIZE times that round the camera, about as far as
/// the world is there to see before it fades into the sky
const TEXEL_M: f64 = 0.16;
/// Rows of the map filled in each frame while it's filled in afresh, spread over a few frames so
/// none of them is held up
const REBUILD_ROWS: u32 = 128;
/// And otherwise, going round it over and over, so it keeps up with holes blown in the world
const REFRESH_ROWS: u32 = 8;
/// Past this many voxels from where the map was started, it's started again where the world is,
/// so the numbers in it stay small enough to be exact
const ANCHOR_REACH: i64 = 1 << 15;
/// How far shafts reach, in meters: as far as traverse.comp gathers the haze
const REACH_M: f32 = 160.0;
/// How much dust there is in the air for so much haze, where it shows, see march() in shafts.comp
const DUST_PER_HAZE: f32 = 10.0;
/// How bright the screen space shafts are
const LOW_GAIN: f32 = 0.25;

/// No more than 128 bytes, see limits.rs: the view's in a buffer of its own, Frame::view
#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct Constants {
    sun: [f32; 4],
    air: [f32; 4],
    map: [f32; 4],
    window: [i32; 4],
    world: [u32; 4],
    extra: [f32; 4],
    info: [u32; 4],
}

/// What shafts.comp needs from the frame being drawn
pub struct Frame<'a> {
    /// As in traverse.comp
    pub pixel_to_ray: Matrix4<f32>,
    /// The view traverse.comp was given: that, and world to last frame's rendered pixel
    pub view: &'a Subbuffer<crate::View>,
    /// xyz: towards the sun, w: sea level in voxels
    pub sun_direction: [f32; 4],
    /// Haze per meter at sea level
    pub haze: f32,
    pub world_size: [u32; 3],
    pub voxels_per_meter: u32,
    /// The world's corner in the whole landscape, in voxels
    pub world_origin: [i64; 2],
    /// Where its first column is kept in the room there is for its columns, see World::wrap
    pub world_wrap: [u32; 2],
    pub seconds: f32,
    pub frame: u32,
    /// Whether the camera's been still since last frame
    pub still: bool,
    /// The main world's voxels: its brick grid and bricks
    pub grid: &'a Arc<ImageView>,
    pub bricks: &'a Subbuffer<[u32]>,
    /// traverse.comp's depths by frame parity, its guide, and what post.comp's denoiser leaves
    pub depths: &'a [Arc<ImageView>; 2],
    pub guide: &'a Arc<ImageView>,
    pub hdr: &'a Arc<ImageView>,
}

/// The half resolution images, made for the render resolution
struct Targets {
    extent: [u32; 2],
    render_extent: [u32; 2],
    /// What they're bound with, by frame parity, and what that was made for, kept so new images
    /// can't be taken for them
    sets: [Arc<DescriptorSet>; 2],
    hdr: Arc<ImageView>,
    map_size: u32,
}

/// What the map of where sunlight stops is for: a sun direction, and voxels so big
#[derive(PartialEq, Clone, Copy)]
struct MapKey {
    sun: [u32; 3],
    voxels_per_meter: u32,
}

pub struct Shafts {
    pub quality: Quality,
    /// 1 as it's meant to look, more for thicker air
    pub strength: f32,
    pipeline: HotReloadComputePipeline,
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    /// The map of where sunlight stops, MAP_SIZE across once High's been used, one texel till then
    map: (Arc<Image>, Arc<ImageView>),
    /// How much of the air round the camera is in the shade, see shade() in shafts.comp
    state: Subbuffer<[u32]>,
    targets: Option<Targets>,
    map_key: Option<MapKey>,
    /// The world's corner the map is counted from, in voxels of the landscape
    anchor: [i64; 2],
    /// The map's first texel round the camera, counted from the anchor
    window: Option<[i64; 2]>,
    /// The next row to fill in while it's filled in afresh, and the next to fill in again otherwise
    rebuild: Option<u32>,
    refresh: u32,
    /// Whether last frame drew shafts the same way at the same resolution, to follow this frame
    history: bool,
    /// What composite() adds this frame, if anything
    pending: Option<(Constants, Arc<DescriptorSet>, Arc<DescriptorSet>, [u32; 2])>,
    /// `--set sun_shafts_debug=1` says now and then how shady it is round the camera, and whether
    /// it's time to, as soon as the GPU's done with what says
    debug: bool,
    report_due: bool,
}

fn storage_image(memory_allocator: &Arc<StandardMemoryAllocator>, extent: [u32; 2], format: Format, usage: ImageUsage) -> (Arc<Image>, Arc<ImageView>) {
    let image = Image::new(
        memory_allocator.clone(),
        ImageCreateInfo { format, extent: [extent[0], extent[1], 1], usage, ..Default::default() },
        AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_DEVICE, ..Default::default() },
    )
    .unwrap();
    let view = ImageView::new_default(image.clone()).unwrap();
    (image, view)
}

/// The map's axes across the sunlight, as sunBasis() in shafts.comp
fn sun_basis(sun: Vector3<f64>) -> (Vector3<f64>, Vector3<f64>) {
    let helper = if sun.z.abs() < 0.99 { Vector3::z() } else { Vector3::x() };
    let u = helper.cross(&sun).normalize();
    (u, sun.cross(&u))
}

impl Shafts {
    pub fn new(pipeline: HotReloadComputePipeline, memory_allocator: &Arc<StandardMemoryAllocator>, descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>, quality: Quality, strength: f32) -> Self {
        // Where the CPU can read it, for `--set sun_shafts_debug=1`
        let state = Buffer::new_slice::<u32>(
            memory_allocator.clone(),
            BufferCreateInfo { usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST, ..Default::default() },
            AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS, ..Default::default() },
            4,
        )
        .unwrap();
        Shafts {
            quality,
            strength,
            pipeline,
            memory_allocator: memory_allocator.clone(),
            descriptor_set_allocator: descriptor_set_allocator.clone(),
            map: storage_image(memory_allocator, [1, 1], Format::R32_UINT, ImageUsage::STORAGE | ImageUsage::TRANSFER_DST),
            state,
            targets: None,
            map_key: None,
            anchor: [0, 0],
            window: None,
            rebuild: None,
            refresh: 0,
            history: false,
            pending: None,
            debug: crate::settings_from_args().iter().any(|(k, v)| k == "sun_shafts_debug" && v == "1"),
            report_due: false,
        }
    }

    /// Whether traverse.comp should leave the sunlight the haze scatters to shafts.comp, see
    /// FLAG_SHAFTS there
    pub fn in_the_haze(&self) -> bool {
        self.quality == Quality::High
    }

    /// The settings for them in the settings window
    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Sun Shafts");
            for quality in Quality::ALL {
                ui.selectable_value(&mut self.quality, quality, quality.name());
            }
        });
        ui.add_enabled(self.quality != Quality::Off, egui::Slider::new(&mut self.strength, 0.0..=3.0).text("Sun Shaft Strength"));
    }

    /// Lets go of the half resolution images, and the image drawn into they hold on to, as when
    /// everything sized to the render resolution is let go of before it's made anew, see
    /// App::make_render_images() in main.rs: they're made again when next needed
    pub fn let_go_of_targets(&mut self) {
        self.targets = None;
    }

    /// Makes the half resolution images and what binds them, if they aren't there for the
    /// resolution and images being drawn with
    fn targets(&mut self, frame: &Frame) -> &Targets {
        let render_extent = [frame.hdr.image().extent()[0], frame.hdr.image().extent()[1]];
        let map_size = self.map.0.extent()[0];
        let fits = self.targets.as_ref().is_some_and(|t| t.render_extent == render_extent && Arc::ptr_eq(&t.hdr, frame.hdr) && t.map_size == map_size);
        if !fits {
            let extent = render_extent.map(|n| n.div_ceil(2).max(1));
            let half = || storage_image(&self.memory_allocator, extent, Format::R16G16B16A16_SFLOAT, ImageUsage::STORAGE).1;
            let marched = half();
            let halves = [half(), half()];
            let layout = self.pipeline.layout().set_layouts()[0].clone();
            let sets = [0, 1].map(|parity| {
                DescriptorSet::new(
                    self.descriptor_set_allocator.clone(),
                    layout.clone(),
                    [
                        WriteDescriptorSet::image_view(0, self.map.1.clone()),
                        WriteDescriptorSet::image_view(1, frame.depths[parity].clone()),
                        WriteDescriptorSet::image_view(2, frame.guide.clone()),
                        WriteDescriptorSet::image_view(3, frame.hdr.clone()),
                        WriteDescriptorSet::image_view(4, marched.clone()),
                        WriteDescriptorSet::image_view(5, halves[1 - parity].clone()),
                        WriteDescriptorSet::image_view(6, halves[parity].clone()),
                        WriteDescriptorSet::buffer(7, self.state.clone()),
                    ],
                    [],
                )
                .unwrap()
            });
            self.targets = Some(Targets { extent, render_extent, sets, hdr: frame.hdr.clone(), map_size });
            self.history = false;
        }
        self.targets.as_ref().unwrap()
    }

    /// Runs what's to be done before post.comp: filling in the map and gathering the shafts on
    /// High, smearing the sky on Low. composite() adds them once the denoiser's done. With a
    /// `timer`, the map and the rest are timed apart.
    pub fn record(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, frame: &Frame, mut timer: Option<&mut GpuTimer>) {
        self.pending = None;
        if self.debug {
            self.report_due |= frame.frame % 60 == 0;
            if self.report_due
                && let Ok(words) = self.state.read()
            {
                println!("frame {}: sun shafts {:?}, shade round the camera {:.2}", frame.frame, self.quality, f32::from_bits(words[0]));
                self.report_due = false;
            }
        }
        if self.pipeline.maybe_reload() {
            self.history = false;
            self.map_key = None;
        }
        let sun = Vector3::new(frame.sun_direction[0], frame.sun_direction[1], frame.sun_direction[2]).cast::<f64>();
        // Off, nothing's kept on the GPU, and only High needs the map. What's let go of is kept
        // alive by the frames still using it till they're done.
        let map_size = if self.quality == Quality::High { MAP_SIZE } else { 1 };
        if self.map.0.extent()[0] != map_size {
            self.map = storage_image(&self.memory_allocator, [map_size, map_size], Format::R32_UINT, ImageUsage::STORAGE | ImageUsage::TRANSFER_DST);
            self.map_key = None;
        }
        if self.quality == Quality::Off {
            self.targets = None;
        }
        // No air, or no sun, to light up
        if self.quality == Quality::Off || frame.haze <= 0.0 || sun.z < -0.02 || self.strength <= 0.0 && self.quality == Quality::Low {
            self.history = false;
            return;
        }

        let vpm = frame.voxels_per_meter.max(1);
        let parity = (frame.frame % 2) as usize;
        let (extent, set) = {
            let targets = self.targets(frame);
            (targets.extent, targets.sets[parity].clone())
        };
        let render_extent = self.targets.as_ref().unwrap().render_extent;
        // And the view, see View in shafts.comp
        let voxels = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.pipeline.layout().set_layouts()[1].clone(),
            [WriteDescriptorSet::image_view(0, frame.grid.clone()), WriteDescriptorSet::buffer(1, frame.bricks.clone()), WriteDescriptorSet::buffer(2, frame.view.clone())],
            [],
        )
        .unwrap();
        // Stronger with the sun low, when its light comes in slanting under the trees
        let low_sun = 1.0 - 0.65 * smoothstep(0.2, 0.85, sun.z as f32);
        let [sx, sy, sz] = frame.world_size;
        let mut constants = Constants {
            sun: [frame.sun_direction[0], frame.sun_direction[1], frame.sun_direction[2], 0.0],
            air: [frame.haze, vpm as f32, frame.sun_direction[3], frame.seconds],
            map: [0.0, 0.0, 0.0, 1.0],
            window: [0; 4],
            world: [sx, sy, sz, 0],
            extra: [frame.haze * DUST_PER_HAZE * self.strength * low_sun, REACH_M * vpm as f32, 0.0, 0.0],
            info: [0, frame.frame, 0, frame.world_wrap[0] | frame.world_wrap[1] << 16],
        };
        let half_groups = [extent[0].div_ceil(8), extent[1].div_ceil(8), 1];
        builder.bind_pipeline_compute(self.pipeline.clone()).unwrap();
        builder.bind_descriptor_sets(PipelineBindPoint::Compute, self.pipeline.layout().clone(), 0, vec![set.clone(), voxels.clone()]).unwrap();

        match self.quality {
            Quality::High => {
                let history = self.history;
                self.update_map(builder, frame, sun, &mut constants);
                let history = history && self.history;
                constants.info[2] = if history { FLAG_HISTORY } else { 0 } | if frame.still { FLAG_STILL } else { 0 };
                self.dispatch(builder, Stage::Shade, constants, [1, 1, 1]);
                if let Some(timer) = timer.as_deref_mut() {
                    timer.mark(builder, "shaft map");
                }
                self.dispatch(builder, Stage::March, constants, half_groups);
                self.dispatch(builder, Stage::Resolve, constants, half_groups);
            }
            Quality::Low => {
                // Only looking towards the sun, fading out as it goes off the edge of the screen
                let Some((pixel, fade)) = sun_on_screen(&frame.pixel_to_ray, sun, render_extent) else {
                    self.history = false;
                    return;
                };
                constants.extra[2] = pixel[0];
                constants.extra[3] = pixel[1];
                constants.map[3] = LOW_GAIN * self.strength * low_sun * fade * (frame.haze / 0.0012).clamp(0.5, 3.0).sqrt();
                self.dispatch(builder, Stage::Mask, constants, half_groups);
                self.dispatch(builder, Stage::Radial, constants, half_groups);
                constants.info[2] = FLAG_FINE;
                self.dispatch(builder, Stage::Radial, constants, half_groups);
                constants.info[2] = FLAG_LOW;
            }
            Quality::Off => unreachable!(),
        }
        if let Some(timer) = timer {
            timer.mark(builder, "shafts");
        }
        self.history = self.quality == Quality::High;
        self.pending = Some((constants, set, voxels, render_extent));
    }

    /// Adds the shafts gathered by record() to what post.comp's denoiser left, before its bloom
    pub fn composite(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        let Some((constants, set, voxels, render_extent)) = &self.pending else {
            return;
        };
        builder.bind_pipeline_compute(self.pipeline.clone()).unwrap();
        builder.bind_descriptor_sets(PipelineBindPoint::Compute, self.pipeline.layout().clone(), 0, vec![set.clone(), voxels.clone()]).unwrap();
        self.dispatch(builder, Stage::Composite, *constants, [render_extent[0].div_ceil(8), render_extent[1].div_ceil(8), 1]);
    }

    fn dispatch(&self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, stage: Stage, mut constants: Constants, groups: [u32; 3]) {
        constants.info[0] = stage as u32;
        builder.push_constants(self.pipeline.layout().clone(), 0, constants).unwrap();
        unsafe {
            builder.dispatch(groups).unwrap();
        }
    }

    /// Keeps the map of where sunlight stops round the camera: starts it afresh for a new sun,
    /// fills in what the camera's come up to, and a few rows more each frame to keep it up to date
    fn update_map(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, frame: &Frame, sun: Vector3<f64>, constants: &mut Constants) {
        let vpm = frame.voxels_per_meter.max(1) as f64;
        let key = MapKey { sun: [0, 1, 2].map(|i| frame.sun_direction[i].to_bits()), voxels_per_meter: frame.voxels_per_meter };
        let far = (0..2).any(|i| (frame.world_origin[i] - self.anchor[i]).abs() > ANCHOR_REACH);
        let size = MAP_SIZE as i64;
        let afresh = |shafts: &mut Self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>| {
            builder
                .clear_color_image(ClearColorImageInfo { clear_value: ClearColorValue::Uint([u32::MAX; 4]), ..ClearColorImageInfo::image(shafts.map.0.clone()) })
                .unwrap();
            // Not known yet, see shade() in shafts.comp
            builder.fill_buffer(shafts.state.clone(), (-1.0f32).to_bits()).unwrap();
            shafts.rebuild = Some(0);
            shafts.window = None;
            shafts.history = false;
        };
        if self.map_key != Some(key) || far {
            self.map_key = Some(key);
            self.anchor = frame.world_origin;
            afresh(self, builder);
        }

        let texels_per_voxel = 1.0 / (TEXEL_M * vpm).max(1.0);
        let (u, v) = sun_basis(sun);
        let offset = Vector3::new((frame.world_origin[0] - self.anchor[0]) as f64, (frame.world_origin[1] - self.anchor[1]) as f64, 0.0);
        let corner = [offset.dot(&u) * texels_per_voxel, offset.dot(&v) * texels_per_voxel];
        let camera = frame.pixel_to_ray.fixed_view::<3, 1>(0, 3).into_owned().cast::<f64>();
        let at = [camera.dot(&u) * texels_per_voxel + corner[0], camera.dot(&v) * texels_per_voxel + corner[1]];
        let window = at.map(|t| t.floor() as i64 - size / 2);

        // Rectangles of texels to fill in: x, y, width, height
        let mut fills: Vec<[i64; 4]> = Vec::new();
        if let Some(old) = self.window {
            let moved = [window[0] - old[0], window[1] - old[1]];
            if moved[0].abs() >= size || moved[1].abs() >= size {
                afresh(self, builder);
            } else {
                // The rows and columns come up to, which go where the ones left behind were
                match moved[0] {
                    d if d > 0 => fills.push([old[0] + size, window[1], d, size]),
                    d if d < 0 => fills.push([window[0], window[1], -d, size]),
                    _ => {}
                }
                match moved[1] {
                    d if d > 0 => fills.push([window[0], old[1] + size, size, d]),
                    d if d < 0 => fills.push([window[0], window[1], size, -d]),
                    _ => {}
                }
            }
        }
        self.window = Some(window);
        match self.rebuild {
            Some(row) => {
                let rows = REBUILD_ROWS.min(MAP_SIZE - row);
                fills.push([window[0], window[1] + row as i64, size, rows as i64]);
                self.rebuild = (row + rows < MAP_SIZE).then_some(row + rows);
            }
            None => {
                fills.push([window[0], window[1] + self.refresh as i64, size, REFRESH_ROWS as i64]);
                self.refresh = (self.refresh + REFRESH_ROWS) % MAP_SIZE;
            }
        }

        constants.sun[3] = offset.dot(&sun) as f32;
        constants.map = [corner[0] as f32, corner[1] as f32, texels_per_voxel as f32, 1.0];
        constants.window[0] = window[0] as i32;
        constants.window[1] = window[1] as i32;
        for [x, y, width, height] in fills {
            let mut fill = *constants;
            fill.window[2] = x as i32;
            fill.window[3] = y as i32;
            fill.world[3] = width as u32 | (height as u32) << 16;
            self.dispatch(builder, Stage::Fill, fill, [(width as u32).div_ceil(8), (height as u32).div_ceil(8), 1]);
        }
    }
}

fn smoothstep(lo: f32, hi: f32, x: f32) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where the sun is on screen, in rendered pixels, if it's ahead, and how much to make of shafts
/// from there: all of it on screen, fading out as it goes further off it
fn sun_on_screen(pixel_to_ray: &Matrix4<f32>, sun: Vector3<f64>, extent: [u32; 2]) -> Option<([f32; 2], f32)> {
    let rays = pixel_to_ray.fixed_view::<3, 3>(0, 0).into_owned().cast::<f64>();
    let p = rays.try_inverse()? * sun;
    if p.z <= 1e-6 {
        return None;
    }
    let pixel = [p.x / p.z, p.y / p.z];
    let [w, h] = extent.map(|n| n as f64);
    let outside = (-pixel[0]).max(pixel[0] - w).max(0.0).hypot((-pixel[1]).max(pixel[1] - h).max(0.0));
    let fade = 1.0 - smoothstep(0.0, 0.6 * w.max(h) as f32, outside as f32);
    (fade > 0.0).then_some(([pixel[0] as f32, pixel[1] as f32], fade))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualities_parse() {
        assert_eq!(Quality::parse("off"), Some(Quality::Off));
        assert_eq!(Quality::parse("Low"), Some(Quality::Low));
        assert_eq!(Quality::parse("HIGH"), Some(Quality::High));
        assert_eq!(Quality::parse("1"), Some(Quality::High));
        assert_eq!(Quality::parse("medium"), None);
        for quality in Quality::ALL {
            assert_eq!(Quality::parse(quality.name()), Some(quality));
        }
    }

    // Straight ahead is the middle of the screen, and behind is nowhere on it
    #[test]
    fn sun_ahead_is_on_screen() {
        // A camera at the origin looking along +y, 100 by 50 pixels, x right and y down
        let mut pixel_to_ray = Matrix4::<f32>::identity();
        pixel_to_ray.fixed_view_mut::<3, 3>(0, 0).copy_from(&nalgebra::Matrix3::new(0.01, 0.0, -0.5, 0.0, 0.0, 1.0, 0.0, -0.01, 0.25));
        let (pixel, fade) = sun_on_screen(&pixel_to_ray, Vector3::y(), [100, 50]).unwrap();
        assert!((pixel[0] - 50.0).abs() < 1e-3 && (pixel[1] - 25.0).abs() < 1e-3);
        assert_eq!(fade, 1.0);
        assert!(sun_on_screen(&pixel_to_ray, -Vector3::y(), [100, 50]).is_none());
    }

    // The map's axes are square to the sun and to each other
    #[test]
    fn sun_basis_is_square() {
        for sun in [Vector3::new(0.3, 0.5, 0.81), Vector3::new(0.0, 0.0, 1.0), Vector3::new(-0.9, 0.1, 0.05)] {
            let sun = sun.normalize();
            let (u, v) = sun_basis(sun);
            assert!(u.dot(&v).abs() < 1e-9 && u.dot(&sun).abs() < 1e-9 && v.dot(&sun).abs() < 1e-9);
            assert!((u.norm() - 1.0).abs() < 1e-9 && (v.norm() - 1.0).abs() < 1e-9);
        }
    }
}
