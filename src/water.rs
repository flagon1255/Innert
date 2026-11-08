//! Water flowing over the landscape. shaders\water.comp simulates it on the GPU, one column of the
//! main world at a time, and shaders\traverse.comp draws it as voxels. A copy of how deep it is
//! comes back to the CPU every half second or so, for swimming, animals and the bucket.
//! Positions are in voxels of the main world.
//!
//! The main world moves on over the landscape a strip at a time, see World::window(), and the water
//! with it: each column's kept where it is in the landscape modulo the room there is for them, so
//! the water flows on as it was with nothing moved, and only the strip coming in starts out fresh,
//! see take_strip().

use std::f64::consts::PI;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use nalgebra::Vector3;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, BufferImageCopy, ClearColorImageInfo, CommandBufferUsage, CopyBufferToImageInfo,
        CopyImageToBufferInfo, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator},
    device::Queue,
    format::Format,
    image::{Image, ImageCreateInfo, ImageUsage, view::ImageView},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sync::GpuFuture,
};

use crate::worldgen::{World, is_passable};

/// Has to match the stages in shaders\water.comp
#[derive(Clone, Copy)]
enum Stage {
    Flux,
    Depth,
    Surface,
    Blocks,
    SuperBlocks,
}

/// Columns per side of the blocks rays skip dry land in, has to match shaders\water.comp
const BLOCK: u32 = 8;
/// Blocks per side of the larger blocks rays skip dry land in a long way at a time, kept beside the
/// blocks in the same image, has to match shaders\water.comp
const SUPER: u32 = 8;
/// The longest step the simulation takes, in seconds. Longer ones aren't stable in deep water.
const STEP: f64 = 1.0 / 120.0;
/// At most this many steps a frame, so a slow frame doesn't make the next one slower still
const MAX_STEPS: u32 = 6;
/// The water flows in an area this many columns across around the camera. Further away it keeps
/// still until the camera comes near, and rivers flowing in from there keep flowing in. The lower
/// quality presets make it smaller, see set_flowing_area().
static ACTIVE_COLUMNS: AtomicU32 = AtomicU32::new(1024);

/// How many columns across the water flows in around the camera, see ACTIVE_COLUMNS
pub fn set_flowing_area(columns: u32) {
    ACTIVE_COLUMNS.store(columns.max(64), Ordering::Relaxed);
}
/// Frames between copies of the water back to the CPU
const READBACK_FRAMES: u32 = 30;
/// Water shallower than this, in meters, doesn't count as water for walking about in
const WADE_DEPTH: f64 = 0.04;

#[derive(BufferContents, Clone, Copy)]
#[repr(C)]
struct Constants {
    brush: [f32; 4],
    wake: [f32; 4],
    params: [f32; 4],
    info: [u32; 4],
    /// How many columns the world is across, and where its first is kept, see shaders\water.comp
    room: [u32; 4],
}

/// What the water's kept in on the GPU that a strip coming into the world is sent over into, see
/// fill_strip()
#[derive(Clone)]
pub struct WaterImages {
    pub ground: Arc<Image>,
    pub depth: Arc<Image>,
    pub flux: Arc<Image>,
}

pub struct Water {
    /// How many columns the world is across
    size: [u32; 2],
    /// How many columns there's room for in the images and the CPU's copy, which the world's go
    /// round, and where its first is kept, see World::phys and World::wrap
    room: [u32; 2],
    wrap: [u32; 2],
    voxels_per_meter: f64,
    sea_level: f32,
    spring_count: u32,
    set: Arc<DescriptorSet>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    layout: Arc<vulkano::descriptor_set::layout::DescriptorSetLayout>,
    // For traverse.comp
    pub ground_view: Arc<ImageView>,
    pub surface_view: Arc<ImageView>,
    pub blocks_view: Arc<ImageView>,
    depth_view: Arc<ImageView>,
    flux_view: Arc<ImageView>,
    depth_image: Arc<Image>,
    flux_image: Arc<Image>,
    /// Whether there's any water to simulate
    pub active: bool,

    // The CPU's copy, one value per column, kept as the images keep them
    ground: Vec<f32>,
    depth: Vec<f32>,
    // The depths get copied into this, and read once the GPU is done with it. Only the water
    // around the camera flows, so only the columns it's flowed in since the last time come back:
    // `dirty`, from x and y to x and y counted from the world's first column, and `readback_rects`
    // for those on their way, each where it's kept and how many across
    readback: Subbuffer<[f32]>,
    readback_pending: bool,
    readback_rects: Vec<([u32; 2], [u32; 2])>,
    dirty: Option<[u32; 4]>,
    frames_since_readback: u32,
    /// Strips that have come into the world whose surface is still to be worked out, from x and
    /// y to x and y counted from the world's first column, see record()
    resurface: Vec<[u32; 4]>,

    /// Water poured in or scooped out, sent with the next step
    brush: Option<[f32; 4]>,
    /// Simulated time owed to catch up with real time
    time_debt: f64,
    /// Whether the surface of all the water has been worked out, not just near the camera
    drawn_once: bool,
    /// Whether the ground's changed since the surface round the camera was last worked out, like
    /// when a hole's been blown in it, see record()
    ground_changed: bool,
}

/// The smallest box around both, from x and y to x and y
fn union(a: [u32; 4], b: [u32; 4]) -> [u32; 4] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// A box of columns of a world `size` columns across, from x and y to x and y, in the columns of
/// one whose corner is `shift` columns on, if any of it's in that one
fn shifted(region: [u32; 4], shift: [i64; 2], size: [u32; 2]) -> Option<[u32; 4]> {
    let moved = |v: u32, i: usize| (v as i64 - shift[i]).clamp(0, size[i] as i64) as u32;
    let [x0, y0, x1, y1] = [moved(region[0], 0), moved(region[1], 1), moved(region[2], 0), moved(region[3], 1)];
    (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
}

/// Where the columns from `lo` to `hi` counted from the world's first are kept in `room` columns
/// with its first at `wrap`, split where they go round: each the first kept, the first of the
/// world's it is, and how many across
fn kept(lo: [u32; 2], hi: [u32; 2], room: [u32; 2], wrap: [u32; 2]) -> Vec<([u32; 2], [u32; 2], [u32; 2])> {
    let spans = |i: usize| -> Vec<(u32, u32, u32)> {
        let n = hi[i].saturating_sub(lo[i]);
        if n == 0 {
            return Vec::new();
        }
        let first = (lo[i] + wrap[i]) % room[i];
        if first + n <= room[i] {
            vec![(first, lo[i], n)]
        } else {
            let a = room[i] - first;
            vec![(first, lo[i], a), (0, lo[i] + a, n - a)]
        }
    };
    let (xs, ys) = (spans(0), spans(1));
    let mut rects = Vec::new();
    for &(ky, wy, ny) in &ys {
        for &(kx, wx, nx) in &xs {
            rects.push(([kx, ky], [wx, wy], [nx, ny]));
        }
    }
    rects
}

fn storage_image(memory_allocator: &Arc<StandardMemoryAllocator>, extent: [u32; 2], format: Format) -> Arc<Image> {
    Image::new(
        memory_allocator.clone(),
        ImageCreateInfo {
            format,
            extent: [extent[0], extent[1], 1],
            usage: ImageUsage::STORAGE | ImageUsage::TRANSFER_DST | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
            ..Default::default()
        },
    )
    .unwrap()
}

/// What water.comp is bound with: the images, `views`, and the springs, see Water::set_springs(),
/// with how many springs there are
fn springs_set(
    memory_allocator: &Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>,
    layout: &Arc<vulkano::descriptor_set::layout::DescriptorSetLayout>,
    views: [&Arc<ImageView>; 5],
    springs: &[[f32; 4]],
    origin: [i64; 2],
    vpm: f64,
) -> (Arc<DescriptorSet>, u32) {
    // In columns of this world, counted from its corner
    let in_columns: Vec<[f32; 4]> = springs
        .iter()
        .map(|s| [(s[0] as f64 * vpm - origin[0] as f64) as f32, (s[1] as f64 * vpm - origin[1] as f64) as f32, (s[2] as f64 * vpm.powi(3)) as f32, (s[3] as f64 * vpm) as f32])
        .collect();
    let count = in_columns.len() as u32;
    let buffer = Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        // An empty buffer can't be bound
        if in_columns.is_empty() { vec![[0.0f32; 4]] } else { in_columns },
    )
    .unwrap();
    let mut writes: Vec<WriteDescriptorSet> = views.iter().enumerate().map(|(i, view)| WriteDescriptorSet::image_view(i as u32, (*view).clone())).collect();
    writes.push(WriteDescriptorSet::buffer(5, buffer));
    (DescriptorSet::new(descriptor_set_allocator.clone(), layout.clone(), writes, []).unwrap(), count)
}

/// Sends `values` over to `image` at `at`, `extent` across, a row after another
fn send_rect(
    memory_allocator: &Arc<StandardMemoryAllocator>,
    command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
    queue: &Arc<Queue>,
    image: &Arc<Image>,
    at: [u32; 2],
    extent: [u32; 2],
    words: usize,
    fill: impl Fn(&mut [u32], usize),
) {
    let row = extent[0] as usize * words;
    crate::staging::send(
        memory_allocator,
        command_buffer_allocator,
        queue,
        row * extent[1] as usize,
        crate::staging::whole(row),
        |piece, first| fill(piece, first),
        |builder, piece, first| {
            let (y, rows) = ((first / row) as u32, (piece.len() as usize / row) as u32);
            builder
                .copy_buffer_to_image(CopyBufferToImageInfo {
                    regions: [BufferImageCopy {
                        image_subresource: image.subresource_layers(),
                        image_offset: [at[0], at[1] + y, 0],
                        image_extent: [extent[0], rows, 1],
                        ..Default::default()
                    }]
                    .into(),
                    ..CopyBufferToImageInfo::buffer_image(piece, image.clone())
                })
                .unwrap();
        },
    );
}

impl Water {
    /// The ground in the columns from `lo` to `hi` changed, like when a hole was blown in it:
    /// takes the new ground from `world`'s water map here, and gives what's to go over to the GPU
    /// with the next frame, see App::flush_edits() and stream.rs: each part of the room it's in,
    /// where, how big, and the words
    pub fn take_ground(&mut self, world: &World, lo: [i64; 2], hi: [i64; 2]) -> Vec<([u32; 2], [u32; 2], Vec<u32>)> {
        let Some(map) = &world.water else {
            return Vec::new();
        };
        if !self.active || map.ground.len() != self.ground.len() {
            return Vec::new();
        }
        let [sx, sy] = self.size.map(i64::from);
        let lo = [lo[0].clamp(0, sx) as u32, lo[1].clamp(0, sy) as u32];
        let hi = [hi[0].clamp(0, sx) as u32, hi[1].clamp(0, sy) as u32];
        let width = self.room[0] as usize;
        self.ground_changed = true;
        kept(lo, hi, self.room, self.wrap)
            .into_iter()
            .map(|(at, _, extent)| {
                let mut words = Vec::with_capacity(extent[0] as usize * extent[1] as usize);
                for y in 0..extent[1] as usize {
                    let start = at[0] as usize + (at[1] as usize + y) * width;
                    let row = &map.ground[start..start + extent[0] as usize];
                    self.ground[start..start + extent[0] as usize].copy_from_slice(row);
                    words.extend(row.iter().map(|g| g.to_bits()));
                }
                (at, extent, words)
            })
            .collect()
    }

    /// `springs` are where water wells up: x, y, cubic meters a second and radius, in meters from
    /// the corner of the whole landscape, see set_springs()
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        memory_allocator: &Arc<StandardMemoryAllocator>,
        command_buffer_allocator: &Arc<StandardCommandBufferAllocator>,
        descriptor_set_allocator: &Arc<StandardDescriptorSetAllocator>,
        queue: &Arc<Queue>,
        pipeline: &ComputePipeline,
        world: &World,
        springs: &[[f32; 4]],
    ) -> Self {
        let map = world.water.as_ref();
        let (size, room) = if map.is_some() { ([world.size[0], world.size[1]], world.phys) } else { ([1, 1], [1, 1]) };
        let columns = (room[0] * room[1]) as usize;
        let ground = map.map_or(vec![0.0; columns], |m| m.ground.clone());
        let depth: Vec<f32> = match map {
            Some(m) => m.surface.iter().zip(&m.ground).map(|(&s, &g)| (s - g).max(0.0)).collect(),
            None => vec![0.0; columns],
        };

        let image = |extent: [u32; 2], format| storage_image(memory_allocator, extent, format);
        let ground_image = image(room, Format::R32_SFLOAT);
        let depth_image = image(room, Format::R32_SFLOAT);
        // Half floats, see shaders\water.comp
        let flux_image = image(room, Format::R16G16B16A16_SFLOAT);
        let surface_image = image(room, Format::R16G16B16A16_SFLOAT);
        // The blocks, then to their right the larger blocks
        let blocks_image = image([room[0].div_ceil(BLOCK) + room[0].div_ceil(BLOCK * SUPER), room[1].div_ceil(BLOCK)], Format::R32_SFLOAT);
        let readback = Buffer::new_slice::<f32>(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            columns as u64,
        )
        .unwrap();
        let view = |image: &Arc<Image>| ImageView::new_default(image.clone()).unwrap();
        let (ground_view, depth_view, flux_view, surface_view, blocks_view) =
            (view(&ground_image), view(&depth_image), view(&flux_image), view(&surface_image), view(&blocks_image));

        // The ground and how deep the water is go over a few rows at a time, see staging.rs
        for (data, image) in [(&ground, &ground_image), (&depth, &depth_image)] {
            send_rect(memory_allocator, command_buffer_allocator, queue, image, [0, 0], room, 1, |piece, first| {
                for (word, value) in piece.iter_mut().zip(&data[first..]) {
                    *word = value.to_bits();
                }
            });
        }
        let mut builder = AutoCommandBufferBuilder::primary(
            command_buffer_allocator.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        builder
            .clear_color_image(ClearColorImageInfo::image(flux_image.clone()))
            .unwrap()
            .clear_color_image(ClearColorImageInfo {
                // No water lying or falling anywhere, see shaders\water.comp
                clear_value: [-1.0, 0.0, 60000.0, -60000.0].into(),
                ..ClearColorImageInfo::image(surface_image)
            })
            .unwrap()
            .clear_color_image(ClearColorImageInfo {
                clear_value: [-1.0, 0.0, 0.0, 0.0].into(),
                ..ClearColorImageInfo::image(blocks_image)
            })
            .unwrap();
        // Waited for on a fence: a future dropped unfinished waits for the whole queue to go idle,
        // holding it the while, so frames being drawn meanwhile couldn't be sent
        builder.build().unwrap().execute(queue.clone()).unwrap().then_signal_fence_and_flush().unwrap().wait(None).unwrap();

        let layout = pipeline.layout().set_layouts()[0].clone();
        let views = [&ground_view, &depth_view, &flux_view, &surface_view, &blocks_view];
        let vpm = world.voxels_per_meter as f64;
        let (set, spring_count) = springs_set(memory_allocator, descriptor_set_allocator, &layout, views, springs, [world.origin[0], world.origin[1]], vpm);
        Water {
            size,
            room,
            wrap: if map.is_some() { world.wrap } else { [0, 0] },
            voxels_per_meter: vpm,
            sea_level: world.sea_level.unwrap_or(0.0),
            spring_count,
            set,
            descriptor_set_allocator: descriptor_set_allocator.clone(),
            memory_allocator: memory_allocator.clone(),
            layout,
            ground_view,
            surface_view,
            blocks_view,
            depth_view,
            flux_view,
            depth_image,
            flux_image,
            active: map.is_some(),
            ground,
            depth,
            readback,
            readback_pending: false,
            readback_rects: Vec::new(),
            dirty: None,
            frames_since_readback: 0,
            resurface: Vec::new(),
            brush: None,
            time_debt: 0.0,
            drawn_once: false,
            ground_changed: false,
        }
    }

    /// Where water wells up, rivers' springs and inside buildings: x, y, cubic meters a second and
    /// radius, in meters from the corner of the whole landscape, for the world whose corner is at
    /// `origin` in its voxels. Made anew whenever the world moves on.
    pub fn set_springs(&mut self, springs: &[[f32; 4]], origin: [i64; 2]) {
        let views = [&self.ground_view, &self.depth_view, &self.flux_view, &self.surface_view, &self.blocks_view];
        (self.set, self.spring_count) = springs_set(&self.memory_allocator, &self.descriptor_set_allocator, &self.layout, views, springs, origin, self.voxels_per_meter);
    }

    /// The world moved on over the landscape `shift` columns, so its first is kept at `wrap` now:
    /// nothing in the images moves, only what's counted from the world's first column
    pub fn moved(&mut self, shift: [i64; 2], wrap: [u32; 2]) {
        self.wrap = wrap;
        self.dirty = self.dirty.and_then(|region| shifted(region, shift, self.size));
        self.resurface = self.resurface.iter().filter_map(|&region| shifted(region, shift, self.size)).collect();
    }

    /// What the GPU keeps the water in, for a strip coming in to be sent over into, see
    /// fill_strip()
    pub fn images(&self) -> WaterImages {
        WaterImages { ground: self.ground_view.image().clone(), depth: self.depth_image.clone(), flux: self.flux_image.clone() }
    }

    /// The CPU's copy of what stream.rs sends over of a strip, once it's in the world: its ground and
    /// how deep the water is on it, `origin` its first column in the whole landscape and `size` how
    /// many across. Its surface is worked out by the next record() it's in the world for.
    pub fn take_strip(&mut self, origin: [i64; 2], size: [u32; 2], ground: &[f32], surface: &[f32]) {
        if !self.active {
            return;
        }
        // A row at a time, in a run or two where it goes round: hundreds of thousands of columns
        let width = self.room[0] as usize;
        let xs: Vec<(usize, usize, usize)> = crate::worldgen::runs(origin[0], size[0], self.room[0]).collect();
        for y in 0..size[1] as usize {
            let row = (origin[1] + y as i64).rem_euclid(self.room[1] as i64) as usize * width;
            for &(at, along, n) in &xs {
                for k in 0..n {
                    let i = y * size[0] as usize + along + k;
                    let (g, s) = (ground.get(i).copied().unwrap_or(0.0), surface.get(i).copied().unwrap_or(0.0));
                    self.ground[row + at + k] = g;
                    self.depth[row + at + k] = (s - g).max(0.0);
                }
            }
        }
    }

    /// The strip from `lo` to `hi`, counted from the world's first column, has just come into the
    /// world: its water's surface is to be worked out to be drawn
    pub fn resurface(&mut self, lo: [u32; 2], hi: [u32; 2]) {
        self.resurface.push([lo[0], lo[1], hi[0], hi[1]]);
    }

    /// How much of the GPU's memory it takes, near enough: ground, depth, flux and surface per
    /// column, and the blocks
    pub fn bytes(&self) -> u64 {
        let columns = self.room[0] as u64 * self.room[1] as u64;
        let blocks = self.room[0].div_ceil(BLOCK) as u64 * self.room[1].div_ceil(BLOCK) as u64;
        columns * (4 + 4 + 8 + 8) + blocks * 4
    }

    /// How much it keeps on the CPU: the ground and depths
    pub fn cpu_bytes(&self) -> u64 {
        ((self.ground.capacity() + self.depth.capacity()) * 4) as u64
    }

    fn index(&self, x: f64, y: f64) -> Option<usize> {
        let (x, y) = (x.floor(), y.floor());
        if !self.active || x < 0.0 || y < 0.0 || x >= self.size[0] as f64 || y >= self.size[1] as f64 {
            return None;
        }
        let at = |v: f64, i: usize| {
            let p = v as u32 + self.wrap[i];
            (if p >= self.room[i] { p - self.room[i] } else { p }) as usize
        };
        Some(at(x, 0) + at(y, 1) * self.room[0] as usize)
    }

    /// The top of the water at a point, or NEG_INFINITY where it's dry. A little behind the GPU.
    pub fn surface_at(&self, x: f64, y: f64) -> f64 {
        match self.index(x, y) {
            Some(i) if self.depth[i] as f64 > WADE_DEPTH * self.voxels_per_meter => (self.ground[i] + self.depth[i]) as f64,
            _ => f64::NEG_INFINITY,
        }
    }

    /// How deep the water is at a point
    pub fn depth_at(&self, x: f64, y: f64) -> f64 {
        self.index(x, y).map_or(0.0, |i| self.depth[i] as f64)
    }

    /// How much water there is within `radius` of a point, in cubic voxels
    pub fn volume_near(&self, x: f64, y: f64, radius: f64) -> f64 {
        let mut volume = 0.0;
        let r = radius.ceil() as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                if ((dx * dx + dy * dy) as f64) < radius * radius {
                    volume += self.depth_at(x + dx as f64, y + dy as f64);
                }
            }
        }
        volume
    }

    /// Pours `volume` cubic voxels of water in a mound around a point `radius` across, or takes it
    /// out if negative. The mound's shape has to match the brush in shaders\water.comp.
    pub fn pour(&mut self, x: f64, y: f64, radius: f64, volume: f64) {
        // The volume under a mound 1 high that falls off like (1 + cos(pi r / radius)) / 2
        let depth = volume / (radius * radius * (PI / 2.0 - 2.0 / PI));
        self.brush = Some([x as f32, y as f32, radius as f32, depth as f32]);
        // Keep the CPU's copy roughly right until the next one comes back
        let r = radius.ceil() as i64;
        for dy in -r..=r {
            for dx in -r..=r {
                let from = ((dx * dx + dy * dy) as f64).sqrt();
                if from < radius
                    && let Some(i) = self.index(x + dx as f64, y + dy as f64)
                {
                    let weight = 0.5 + 0.5 * (PI * from / radius).cos();
                    self.depth[i] = (self.depth[i] + (depth * weight) as f32).max(0.0);
                }
            }
        }
    }

    /// The first water or ground along a ray within `reach`: where, and whether it's water
    pub fn raycast(&self, world: &World, eye: Vector3<f64>, look: Vector3<f64>, reach: f64) -> Option<(Vector3<f64>, bool)> {
        let steps = (reach * 4.0) as usize;
        (1..=steps).find_map(|i| {
            let p = eye + look * (i as f64 * 0.25);
            if p.z < self.surface_at(p.x, p.y) {
                Some((p, true))
            } else if !is_passable(world.material_at(p.x.floor() as i64, p.y.floor() as i64, p.z.floor() as i64)) {
                Some((p, false))
            } else {
                None
            }
        })
    }

    /// Runs the simulation for a frame `frame_time` seconds long, in the area around `camera`, a
    /// column. `wake` is something wading through the water: where it is and how fast it's going,
    /// in voxels and voxels per second.
    pub fn record(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: &Arc<ComputePipeline>,
        frame_time: f64,
        camera: [f64; 2],
        wake: Option<([f64; 2], [f64; 2])>,
        mut mark: impl FnMut(&mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, &'static str),
    ) {
        if !self.active {
            return;
        }
        self.time_debt = (self.time_debt + frame_time).min(STEP * MAX_STEPS as f64);
        let steps = (self.time_debt / STEP) as u32;
        self.time_debt -= steps as f64 * STEP;

        // Only the water near the camera flows. The first time round, all of it gets its surface
        // worked out to draw.
        let area = self.size.map(|n| n.min(ACTIVE_COLUMNS.load(Ordering::Relaxed)));
        let corner = [0, 1].map(|i| {
            let c = (camera[i] as i64 - area[i] as i64 / 2).clamp(0, (self.size[i] - area[i]) as i64);
            (c as u32 / BLOCK) * BLOCK
        });
        let (surface_corner, surface_area) = if self.drawn_once { (corner, area) } else { ([0, 0], self.size) };
        let first = !self.drawn_once;
        self.drawn_once = true;
        if steps > 0 {
            let flowing = [corner[0], corner[1], corner[0] + area[0], corner[1] + area[1]];
            self.dirty = Some(self.dirty.map_or(flowing, |d| union(d, flowing)));
        }

        let wake = wake.map_or([0.0; 4], |(p, v)| [p[0] as f32, p[1] as f32, v[0] as f32, v[1] as f32]);
        let room = [self.size[0], self.size[1], self.wrap[0], self.wrap[1]];
        let constants = |stage: Stage, brush: [f32; 4], corner: [u32; 2]| Constants {
            brush,
            wake,
            params: [STEP as f32, self.sea_level, self.voxels_per_meter as f32, 0.0],
            info: [stage as u32, self.spring_count, corner[0], corner[1]],
            room,
        };

        builder
            .bind_pipeline_compute(pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, vec![self.set.clone()])
            .unwrap();
        let groups = |area: [u32; 2]| [area[0].div_ceil(8), area[1].div_ceil(8), 1];
        let dispatch = |builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, c: Constants, groups: [u32; 3]| {
            builder.push_constants(pipeline.layout().clone(), 0, c).unwrap();
            unsafe {
                builder.dispatch(groups).unwrap();
            }
        };
        for step in 0..steps {
            let brush = if step == 0 { self.brush.take().unwrap_or([0.0; 4]) } else { [0.0; 4] };
            dispatch(builder, constants(Stage::Flux, [0.0; 4], corner), groups(area));
            dispatch(builder, constants(Stage::Depth, brush, corner), groups(area));
        }
        // Screenshot runs with `--set gpu_times=1` time each part
        mark(builder, "water flow");
        // The surface round the camera, and of strips that have just come in, whose blocks go by it.
        // Round the camera only when the water's moved since, or the ground has: it steps 120 times
        // a second, so on faster frames than that it often hasn't, and the surface would come out
        // the same as last frame's.
        let mut surfaces = Vec::new();
        if steps > 0 || first || std::mem::take(&mut self.ground_changed) {
            surfaces.push((surface_corner, surface_area));
        }
        for [x0, y0, x1, y1] in std::mem::take(&mut self.resurface) {
            let (lo, hi) = ([x0 / BLOCK * BLOCK, y0 / BLOCK * BLOCK], [x1, y1]);
            surfaces.push((lo, [hi[0] - lo[0], hi[1] - lo[1]]));
        }
        for &(corner, area) in &surfaces {
            dispatch(builder, constants(Stage::Surface, [0.0; 4], corner), groups(area));
        }
        for &(corner, area) in &surfaces {
            let blocks = area.map(|n| n.div_ceil(BLOCK).div_ceil(8));
            dispatch(builder, constants(Stage::Blocks, [0.0; 4], corner), [blocks[0], blocks[1], 1]);
        }
        // All of the larger blocks, a few thousand, each the highest of the blocks in it
        if !surfaces.is_empty() {
            let supers = self.room.map(|n| n.div_ceil(BLOCK * SUPER).div_ceil(8));
            dispatch(builder, constants(Stage::SuperBlocks, [0.0; 4], [0, 0]), [supers[0], supers[1], 1]);
        }
        mark(builder, "water surface");

        // Bring the depths where it's been flowing back to the CPU now and then, picking them up
        // once the GPU is done
        if self.readback_pending {
            let total: u64 = self.readback_rects.iter().map(|(_, e)| e[0] as u64 * e[1] as u64).sum();
            if let Ok(data) = self.readback.clone().slice(0..total.max(1)).read() {
                let width = self.room[0] as usize;
                let mut offset = 0;
                for &(at, extent) in &self.readback_rects {
                    let row = extent[0] as usize;
                    for y in 0..extent[1] as usize {
                        let start = at[0] as usize + (at[1] as usize + y) * width;
                        self.depth[start..start + row].copy_from_slice(&data[offset..offset + row]);
                        offset += row;
                    }
                }
                self.readback_pending = false;
            }
        }
        self.frames_since_readback += 1;
        if !self.readback_pending
            && self.frames_since_readback >= READBACK_FRAMES
            && let Some(region) = self.dirty.take()
        {
            self.frames_since_readback = 0;
            let [x0, y0, x1, y1] = region;
            let mut offset = 0u64;
            self.readback_rects.clear();
            let mut regions = Vec::new();
            for (at, _, extent) in kept([x0, y0], [x1, y1], self.room, self.wrap) {
                regions.push(BufferImageCopy {
                    buffer_offset: offset * 4,
                    image_subresource: self.depth_image.subresource_layers(),
                    image_offset: [at[0], at[1], 0],
                    image_extent: [extent[0], extent[1], 1],
                    ..Default::default()
                });
                offset += extent[0] as u64 * extent[1] as u64;
                self.readback_rects.push((at, extent));
            }
            if !regions.is_empty() {
                builder
                    .copy_image_to_buffer(CopyImageToBufferInfo {
                        regions: regions.into(),
                        ..CopyImageToBufferInfo::image_buffer(self.depth_image.clone(), self.readback.clone())
                    })
                    .unwrap();
                self.readback_pending = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_flowed_moves_with_the_world() {
        assert_eq!(union([10, 20, 30, 40], [5, 25, 35, 30]), [5, 20, 35, 40]);
        // The world moved on 100 columns in x and 50 back in y
        assert_eq!(shifted([200, 100, 400, 300], [100, -50], [1000, 1000]), Some([100, 150, 300, 350]));
        // Partly off the edge of the new one
        assert_eq!(shifted([0, 0, 300, 300], [200, 0], [1000, 1000]), Some([0, 0, 100, 300]));
        assert_eq!(shifted([900, 900, 1000, 1000], [-200, -200], [1000, 1000]), None);
        // Wholly off it
        assert_eq!(shifted([0, 0, 100, 100], [200, 0], [1000, 1000]), None);
    }

    /// Columns of the world are kept where they are modulo the room there is, so a box of them is
    /// one rectangle of the images, or two or four where it goes round
    #[test]
    fn boxes_of_columns_go_round_the_room() {
        // Nowhere near going round
        assert_eq!(kept([10, 20], [30, 40], [100, 100], [5, 5]), vec![([15, 25], [10, 20], [20, 20])]);
        // Going round along x: the first 10 kept at 90 to 99, the rest from 0
        assert_eq!(kept([0, 0], [30, 5], [100, 100], [90, 0]), vec![([90, 0], [0, 0], [10, 5]), ([0, 0], [10, 0], [20, 5])]);
        // Both ways: four pieces, covering all of it
        let rects = kept([0, 0], [80, 80], [100, 100], [60, 70]);
        assert_eq!(rects.len(), 4);
        assert_eq!(rects.iter().map(|(_, _, e)| e[0] * e[1]).sum::<u32>(), 80 * 80);
        for (at, from, extent) in rects {
            assert_eq!(at[0], (from[0] + 60) % 100);
            assert_eq!(at[1], (from[1] + 70) % 100);
            assert!(at[0] + extent[0] <= 100 && at[1] + extent[1] <= 100);
        }
    }
}
