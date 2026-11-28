//! How far the main world's empty bricks skip, worked out again on the GPU round where each strip
//! that comes in joins the world, see shaders\skips.comp. Built on its own, a strip's empty bricks
//! skip no further than its edges, nor do those at the edge of the world it comes in beside, see
//! World::keep_skips_inside(): a world that had moved on a while, with a seam like that every 16 m
//! each way, took a third longer to trace than the piece it started as. Three passes, each in
//! SLICES slices, one a frame: a pass in one frame was 0.3 ms of it on an RTX 4050.

use std::collections::VecDeque;
use std::sync::Arc;

use vulkano::{
    buffer::BufferContents,
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{DescriptorSet, WriteDescriptorSet, allocator::StandardDescriptorSetAllocator},
    image::view::ImageView,
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
};

/// The passes, has to match the stages in shaders\skips.comp
const STAGES: u32 = 3;
/// Each pass is done a slice of it a frame, in this many
const SLICES: u32 = 8;
/// The furthest an empty brick's worked out to skip, in bricks, has to match MOST in
/// shaders\skips.comp
const MOST: i32 = 32;
/// The longest line of bricks the shader can work along, has to match LONGEST there
const LONGEST: i32 = 1024;
/// Has to match the local size in shaders\skips.comp
const GROUP: u32 = 64;

/// Has to match the push constants in shaders\skips.comp
#[derive(BufferContents, Clone, Copy, Debug, PartialEq)]
#[repr(C)]
struct Constants {
    /// Where the strip joins the world, in bricks counted from its first: from x and y to x and y
    band: [i32; 4],
    /// How many bricks across, along y and high the world is, and the pass
    world: [i32; 4],
    /// Where its first brick is kept in the grid, which they go round
    wrap: [i32; 4],
    /// The first of the columns along z, or of the layers, a slice of a pass works on, and one past
    /// its last
    slice: [i32; 4],
}

impl Constants {
    /// The bricks along x and y each pass works on, as the shader works them out, see acrossX()
    /// and acrossY() there
    fn across(&self) -> ([i32; 2], [i32; 2]) {
        let [x0, y0, x1, y1] = self.band;
        ([(x0 - MOST).max(0), (x1 + MOST).min(self.world[0])], [(y0 - MOST).max(0), (y1 + MOST).min(self.world[1])])
    }

    /// Slice `k` of `stage`: the first and one past the last column along z, for the first pass,
    /// or layer, for the others
    fn slice(&self, stage: u32, k: u32) -> [i32; 2] {
        let (xs, ys) = self.across();
        let n = if stage == 0 { (xs[1] - xs[0]) * (ys[1] - ys[0]) } else { self.world[2] };
        let each = (n as u32).div_ceil(SLICES) as i32;
        [(k as i32 * each).min(n), ((k as i32 + 1) * each).min(n)]
    }

    /// The groups to dispatch for `stage`, over slice `slice` of it
    fn groups(&self, stage: u32, slice: [i32; 2]) -> [u32; 3] {
        let (xs, _) = self.across();
        let n = (slice[1] - slice[0]) as u32;
        match stage {
            // A column along z for each invocation
            0 => [n.div_ceil(GROUP), 1, 1],
            // A line along y for each group, at each x and z of the slice
            1 => [(xs[1] - xs[0]) as u32, n, 1],
            // A line along x for each group, at each y of the band and z of the slice
            _ => [(self.band[3] - self.band[1]) as u32, n, 1],
        }
    }
}

/// Where the skips are to be worked out again for the strip from `lo` to `hi`, in voxels counted
/// from the world's first, that's just come into the world, `size` voxels across and high, its first
/// column kept at `wrap` in the grid: along the way it came in, MOST bricks either side of it, as far
/// as the world goes, and all along the world the other way, as the strip is. None for a world wider
/// than the shader can work along.
fn plan(lo: [u32; 2], hi: [u32; 2], size: [u32; 3], wrap: [u32; 2]) -> Option<Constants> {
    let world = size.map(|n| (n / 8) as i32);
    if world[0] > LONGEST || world[1] > LONGEST {
        return None;
    }
    let band = [0, 1].map(|i| {
        let (lo, hi) = ((lo[i] / 8) as i32, hi[i].div_ceil(8) as i32);
        if lo <= 0 && hi >= world[i] { (0, world[i]) } else { ((lo - MOST).max(0), (hi + MOST).min(world[i])) }
    });
    let constants = Constants {
        band: [band[0].0, band[1].0, band[0].1, band[1].1],
        world: [world[0], world[1], world[2], 0],
        wrap: [(wrap[0] / 8) as i32, (wrap[1] / 8) as i32, 0, 0],
        slice: [0; 4],
    };
    (constants.band[2] > constants.band[0] && constants.band[3] > constants.band[1]).then_some(constants)
}

/// The skips round where the strip joins the world to work out again, and the next slice of which
/// pass
struct Job {
    constants: Constants,
    stage: u32,
    slice: u32,
}

#[derive(Default)]
pub struct Skips {
    jobs: VecDeque<Job>,
    /// The grid the shader's bound with, and the set binding it
    set: Option<(Arc<ImageView>, Arc<DescriptorSet>)>,
}

impl Skips {
    /// The strip from `lo` to `hi`, in voxels counted from the world's first, has just come into the
    /// world, `size` voxels across and high, its first column kept at `wrap` in the grid: the skips
    /// either side of where it joins are worked out again over the next frames. As the world is now,
    /// so they still hold however it moves on meanwhile: nothing's skipped past its edges.
    pub fn joined(&mut self, lo: [u32; 2], hi: [u32; 2], size: [u32; 3], wrap: [u32; 2]) {
        if let Some(constants) = plan(lo, hi, size, wrap) {
            self.jobs.push_back(Job { constants, stage: 0, slice: 0 });
        }
    }

    /// Whether the skips are being worked out again somewhere
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// The world's built afresh, or changed all over: what was still to be done is let go of
    pub fn forget(&mut self) {
        self.jobs.clear();
    }

    /// The next slice of a pass, if any, with the frame being recorded, by `pipeline`,
    /// shaders\skips.comp, on the grid `grid` of the main world
    pub fn record(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, pipeline: &Arc<ComputePipeline>, grid: &Arc<ImageView>, allocator: &Arc<StandardDescriptorSetAllocator>) {
        let Some(job) = self.jobs.front_mut() else {
            return;
        };
        let set = match &self.set {
            Some((view, set)) if Arc::ptr_eq(view, grid) => set.clone(),
            _ => {
                let layout = pipeline.layout().set_layouts()[0].clone();
                let set = DescriptorSet::new(allocator.clone(), layout, [WriteDescriptorSet::image_view(0, grid.clone())], []).unwrap();
                self.set = Some((grid.clone(), set.clone()));
                set
            }
        };
        let slice = job.constants.slice(job.stage, job.slice);
        let mut constants = job.constants;
        constants.world[3] = job.stage as i32;
        constants.slice = [slice[0], slice[1], 0, 0];
        let groups = constants.groups(job.stage, slice);
        if groups.iter().all(|&n| n > 0) {
            builder
                .bind_pipeline_compute(pipeline.clone())
                .unwrap()
                .bind_descriptor_sets(PipelineBindPoint::Compute, pipeline.layout().clone(), 0, vec![set])
                .unwrap()
                .push_constants(pipeline.layout().clone(), 0, constants)
                .unwrap();
            unsafe {
                builder.dispatch(groups).unwrap();
            }
        }
        job.slice += 1;
        if job.slice == SLICES {
            job.slice = 0;
            job.stage += 1;
            if job.stage == STAGES {
                self.jobs.pop_front();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A strip in along x reaches all along the world along y, and its skips are worked out again
    // as far as MOST bricks into the world beside it
    #[test]
    fn where_strips_join_the_world() {
        let (size, strip, wrap) = ([3584, 3584, 1024], 256, [1536, 1024]);
        // In at the far end along x, at the near end along y
        let x = plan([3584 - strip, 0], [3584, 3584], size, wrap).unwrap();
        let y = plan([0, 0], [3584, strip], size, wrap).unwrap();
        assert_eq!(x.band, [448 - 32 - MOST, 0, 448, 448]);
        assert_eq!(y.band, [0, 0, 448, 32 + MOST]);
        // What each pass works on stays inside the world, and the lines are no longer than the
        // shader has room for
        for c in [x, y] {
            let (xs, ys) = c.across();
            assert!(xs[0] >= 0 && xs[1] <= 448 && ys[0] >= 0 && ys[1] <= 448);
            assert!(xs[1] - xs[0] <= LONGEST && ys[1] - ys[0] <= LONGEST);
            assert_eq!(c.wrap, [192, 128, 0, 0]);
            assert_eq!(c.world, [448, 448, 128, 0]);
            for stage in 0..STAGES {
                // The slices of each pass cover all of it, once, in order
                let slices: Vec<[i32; 2]> = (0..SLICES).map(|k| c.slice(stage, k)).collect();
                let all = if stage == 0 { (xs[1] - xs[0]) * (ys[1] - ys[0]) } else { c.world[2] };
                assert_eq!(slices[0][0], 0);
                assert_eq!(slices[SLICES as usize - 1][1], all);
                assert!(slices.windows(2).all(|w| w[0][1] == w[1][0]));
                for s in slices {
                    assert!(c.groups(stage, s).iter().all(|&n| n <= 65535));
                }
            }
        }
        // Wider than the shader can work along, it's left as it is
        assert_eq!(plan([0, 0], [256, 16384], [16384, 16384, 1024], [0, 0]), None);
    }
}
