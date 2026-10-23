//! Water flowing over the landscape. shaders\water.comp simulates it on the GPU, one column of the
//! main world at a time, and shaders\traverse.comp draws it as voxels. A copy of how deep it is
//! comes back to the CPU every half second or so, for swimming, animals and the bucket.
//! Positions are in voxels of the main world.

use std::f64::consts::PI;
use std::sync::Arc;

use nalgebra::Vector3;
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, ClearColorImageInfo, CommandBufferUsage, CopyBufferToImageInfo,
        CopyImageToBufferInfo, PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract,
        allocator::StandardCommandBufferAllocator,
    },
    descriptor_set::{DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator},
    device::Queue,
    format::Format,
    image::{Image, ImageCreateInfo, ImageUsage, view::ImageView},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
};

use crate::worldgen::{World, is_passable};

/// Has to match the stages in shaders\water.comp
#[derive(Clone, Copy)]
enum Stage {
    Flux,
    Depth,
    Surface,
    Blocks,
}

/// Columns per side of the blocks rays skip dry land in, has to match shaders\water.comp
const BLOCK: u32 = 8;
/// The longest step the simulation takes, in seconds. Longer ones aren't stable in deep water.
const STEP: f64 = 1.0 / 120.0;
/// At most this many steps a frame, so a slow frame doesn't make the next one slower still
const MAX_STEPS: u32 = 6;
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
}

pub struct Water {
    size: [u32; 2],
    voxels_per_meter: f64,
    sea_level: f32,
    spring_count: u32,
    set: Arc<DescriptorSet>,
    // For traverse.comp
    pub ground_view: Arc<ImageView>,
    pub surface_view: Arc<ImageView>,
    pub blocks_view: Arc<ImageView>,
    depth_image: Arc<Image>,
    /// Whether there's any water to simulate
    pub active: bool,

    // The CPU's copy, one value per column
    ground: Vec<f32>,
    depth: Vec<f32>,
    // The depths get copied into this, and read once the GPU is done with it
    readback: Subbuffer<[f32]>,
    readback_pending: bool,
    frames_since_readback: u32,

    /// Water poured in or scooped out, sent with the next step
    brush: Option<[f32; 4]>,
    /// Simulated time owed to catch up with real time
    time_debt: f64,
}

fn storage_image(memory_allocator: &Arc<StandardMemoryAllocator>, extent: [u32; 2], format: Format) -> (Arc<Image>, Arc<ImageView>) {
    let image = Image::new(
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
    .unwrap();
    let view = ImageView::new_default(image.clone()).unwrap();
    (image, view)
}

impl Water {
    /// `springs` are where water wells up, see worldgen::Terrain::springs(): x, y, cubic meters a
    /// second and radius, in meters from the corner of the whole landscape
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
        let size = if map.is_some() { [world.size[0], world.size[1]] } else { [1, 1] };
        let columns = (size[0] * size[1]) as usize;
        let ground = map.map_or(vec![0.0; columns], |m| m.ground.clone());
        let depth: Vec<f32> = match map {
            Some(m) => m.surface.iter().zip(&m.ground).map(|(&s, &g)| (s - g).max(0.0)).collect(),
            None => vec![0.0; columns],
        };

        let (ground_image, ground_view) = storage_image(memory_allocator, size, Format::R32_SFLOAT);
        let (depth_image, depth_view) = storage_image(memory_allocator, size, Format::R32_SFLOAT);
        let (flux_image, flux_view) = storage_image(memory_allocator, size, Format::R32G32B32A32_SFLOAT);
        let (surface_image, surface_view) = storage_image(memory_allocator, size, Format::R32G32B32A32_SFLOAT);
        let blocks = size.map(|n| n.div_ceil(BLOCK));
        let (blocks_image, blocks_view) = storage_image(memory_allocator, blocks, Format::R32_SFLOAT);

        let upload = |data: &[f32]| {
            Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                data.iter().copied(),
            )
            .unwrap()
        };
        let mut builder = AutoCommandBufferBuilder::primary(
            command_buffer_allocator.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        builder
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(upload(&ground), ground_image))
            .unwrap()
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(upload(&depth), depth_image.clone()))
            .unwrap()
            .clear_color_image(ClearColorImageInfo::image(flux_image))
            .unwrap()
            .clear_color_image(ClearColorImageInfo {
                clear_value: [-1.0, 0.0, 0.0, 0.0].into(),
                ..ClearColorImageInfo::image(surface_image)
            })
            .unwrap()
            .clear_color_image(ClearColorImageInfo {
                clear_value: [-1.0, 0.0, 0.0, 0.0].into(),
                ..ClearColorImageInfo::image(blocks_image)
            })
            .unwrap();
        let _ = builder.build().unwrap().execute(queue.clone()).unwrap();

        // In columns of this world, counted from its corner
        let vpm = world.voxels_per_meter as f64;
        let in_columns: Vec<[f32; 4]> = springs
            .iter()
            .map(|s| {
                [
                    (s[0] as f64 * vpm - world.origin[0] as f64) as f32,
                    (s[1] as f64 * vpm - world.origin[1] as f64) as f32,
                    (s[2] as f64 * vpm.powi(3)) as f32,
                    (s[3] as f64 * vpm) as f32,
                ]
            })
            .collect();
        let springs_buffer = Buffer::from_iter(
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
            if in_columns.is_empty() { vec![[0.0f32; 4]] } else { in_columns.clone() },
        )
        .unwrap();

        let set = DescriptorSet::new(
            descriptor_set_allocator.clone(),
            pipeline.layout().set_layouts()[0].clone(),
            [
                WriteDescriptorSet::image_view(0, ground_view.clone()),
                WriteDescriptorSet::image_view(1, depth_view),
                WriteDescriptorSet::image_view(2, flux_view),
                WriteDescriptorSet::image_view(3, surface_view.clone()),
                WriteDescriptorSet::image_view(4, blocks_view.clone()),
                WriteDescriptorSet::buffer(5, springs_buffer),
            ],
            [],
        )
        .unwrap();

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

        Water {
            size,
            voxels_per_meter: vpm,
            sea_level: world.sea_level.unwrap_or(0.0),
            spring_count: in_columns.len() as u32,
            set,
            ground_view,
            surface_view,
            blocks_view,
            depth_image,
            active: map.is_some(),
            ground,
            depth,
            readback,
            readback_pending: false,
            frames_since_readback: 0,
            brush: None,
            time_debt: 0.0,
        }
    }

    fn index(&self, x: f64, y: f64) -> Option<usize> {
        let (x, y) = (x.floor(), y.floor());
        if !self.active || x < 0.0 || y < 0.0 || x >= self.size[0] as f64 || y >= self.size[1] as f64 {
            return None;
        }
        Some(x as usize + y as usize * self.size[0] as usize)
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

    /// Runs the simulation for a frame `frame_time` seconds long. `wake` is something wading
    /// through the water: where it is and how fast it's going, in voxels and voxels per second.
    pub fn record(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        pipeline: &Arc<ComputePipeline>,
        frame_time: f64,
        wake: Option<([f64; 2], [f64; 2])>,
    ) {
        if !self.active {
            return;
        }
        self.time_debt = (self.time_debt + frame_time).min(STEP * MAX_STEPS as f64);
        let steps = (self.time_debt / STEP) as u32;
        self.time_debt -= steps as f64 * STEP;

        let wake = wake.map_or([0.0; 4], |(p, v)| [p[0] as f32, p[1] as f32, v[0] as f32, v[1] as f32]);
        let constants = |stage: Stage, brush: [f32; 4]| Constants {
            brush,
            wake,
            params: [STEP as f32, self.sea_level, self.voxels_per_meter as f32, 0.0],
            info: [stage as u32, self.spring_count, 0, 0],
        };

        builder
            .bind_pipeline_compute(pipeline.clone())
            .unwrap()
            .bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, vec![self.set.clone()])
            .unwrap();
        let columns = [self.size[0].div_ceil(8), self.size[1].div_ceil(8), 1];
        let dispatch = |builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, c: Constants, groups: [u32; 3]| {
            builder.push_constants(pipeline.layout().clone(), 0, c).unwrap();
            unsafe {
                builder.dispatch(groups).unwrap();
            }
        };
        for step in 0..steps {
            let brush = if step == 0 { self.brush.take().unwrap_or([0.0; 4]) } else { [0.0; 4] };
            dispatch(builder, constants(Stage::Flux, [0.0; 4]), columns);
            dispatch(builder, constants(Stage::Depth, brush), columns);
        }
        dispatch(builder, constants(Stage::Surface, [0.0; 4]), columns);
        let blocks = self.size.map(|n| n.div_ceil(BLOCK).div_ceil(8));
        dispatch(builder, constants(Stage::Blocks, [0.0; 4]), [blocks[0], blocks[1], 1]);

        // Bring the depths back to the CPU now and then, picking them up once the GPU is done
        if self.readback_pending
            && let Ok(data) = self.readback.read()
        {
            self.depth.copy_from_slice(&data);
            self.readback_pending = false;
        }
        self.frames_since_readback += 1;
        if !self.readback_pending && self.frames_since_readback >= READBACK_FRAMES {
            self.frames_since_readback = 0;
            builder
                .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(self.depth_image.clone(), self.readback.clone()))
                .unwrap();
            self.readback_pending = true;
        }
    }
}
