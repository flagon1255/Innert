//! How long the GPU spends on each part of a frame, for screenshot runs: timestamps written between
//! the passes and read back once the frame's finished, see Capture in main.rs. Also how long the CPU
//! waited for it, which leaves how long the CPU took itself.
//!
//! Each frame's timed in a query pool of its own, POOLS of them in turn: a frame's times are read
//! when its pool comes round again, by when it's long done, so runs that don't wait for each frame
//! (`--set nowait=1`) are timed over all their frames. Read only once a frame had been waited for,
//! they were the last frame's alone.

use std::sync::Arc;
use std::time::Duration;

use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    device::Device,
    query::{QueryPool, QueryPoolCreateInfo, QueryResultFlags, QueryType},
    sync::PipelineStage,
};

/// The most parts a frame is timed in, plus one
const MOST_MARKS: u32 = 16;
/// Frames timed at once, more than are ever sent to the GPU before the oldest is waited for
const POOLS: usize = 3;

pub struct GpuTimer {
    pools: Vec<Arc<QueryPool>>,
    /// The parts timed so far in each pool's frame, in order, till they're added up
    names: Vec<Vec<&'static str>>,
    /// The pool of the frame being timed
    current: usize,
    /// Nanoseconds per tick of the timestamps
    period: f64,
    /// The bits of them that count: 36 on Intel's integrated graphics, which go round every hour or
    /// so, 64 on NVIDIA's
    mask: u64,
    /// Milliseconds spent on each part over all the frames, in the order they first came
    totals: Vec<(&'static str, f64)>,
    frames: u32,
    /// Milliseconds the CPU spent waiting for the GPU to finish frames, in all, and how many frames
    /// it waited for
    waited: f64,
    waits: u32,
}

impl GpuTimer {
    /// None where the queue the game draws with can't time its work, which Vulkan leaves to each
    /// card, saying so
    pub fn new(device: &Arc<Device>) -> Option<Self> {
        let family = device.active_queue_family_indices().first().copied().unwrap_or(0) as usize;
        let bits = device.physical_device().queue_family_properties().get(family).and_then(|q| q.timestamp_valid_bits);
        let Some(bits) = bits.filter(|_| device.physical_device().properties().timestamp_compute_and_graphics) else {
            println!("This graphics card can't time its work, so there are no GPU times");
            return None;
        };
        let pools = (0..POOLS)
            .map(|_| QueryPool::new(device.clone(), QueryPoolCreateInfo { query_count: MOST_MARKS, ..QueryPoolCreateInfo::query_type(QueryType::Timestamp) }).unwrap())
            .collect();
        let period = device.physical_device().properties().timestamp_period as f64;
        let mask = if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 };
        Some(GpuTimer { pools, names: vec![Vec::new(); POOLS], current: 0, period, mask, totals: Vec::new(), frames: 0, waited: 0.0, waits: 0 })
    }

    /// Starts timing a frame, before any of its commands, in the next pool, the frame timed in it
    /// before added up first
    pub fn begin(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        self.current = (self.current + 1) % POOLS;
        self.add_up(self.current, QueryResultFlags::empty());
        self.names[self.current].clear();
        let pool = self.pools[self.current].clone();
        unsafe {
            builder.reset_query_pool(pool.clone(), 0..MOST_MARKS).unwrap();
            builder.write_timestamp(pool, 0, PipelineStage::AllCommands).unwrap();
        }
    }

    /// Ends the part of the frame called `name`: the commands recorded since the last mark
    pub fn mark(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, name: &'static str) {
        let names = &mut self.names[self.current];
        let query = names.len() as u32 + 1;
        if query >= MOST_MARKS {
            return;
        }
        names.push(name);
        unsafe {
            builder.write_timestamp(self.pools[self.current].clone(), query, PipelineStage::AllCommands).unwrap();
        }
    }

    /// Adds up the parts of the frame timed in pool `k`, if they're there to be read: waiting for
    /// them with QueryResultFlags::WAIT, which is only for frames sure to be sent, otherwise left
    /// out if the frame never was
    fn add_up(&mut self, k: usize, flags: QueryResultFlags) {
        let count = self.names[k].len() + 1;
        if count < 2 {
            return;
        }
        let mut ticks = vec![0u64; count];
        let read = self.pools[k].get_results(0..count as u32, &mut ticks, flags).unwrap_or(false);
        let names = std::mem::take(&mut self.names[k]);
        if !read {
            return;
        }
        for (i, &name) in names.iter().enumerate() {
            let ms = (ticks[i + 1].wrapping_sub(ticks[i]) & self.mask) as f64 * self.period / 1e6;
            match self.totals.iter_mut().find(|(n, _)| *n == name) {
                Some((_, total)) => *total += ms,
                None => self.totals.push((name, ms)),
            }
        }
        self.frames += 1;
    }

    /// Adds up the parts of the frame once the GPU's finished it, `waited` being how long the CPU
    /// waited for that
    pub fn finish(&mut self, waited: Duration) {
        self.add_up(self.current, QueryResultFlags::WAIT);
        self.waited += waited.as_secs_f64() * 1000.0;
        self.waits += 1;
    }

    /// Starts adding up afresh, from the next frame finished: from when the world's there to play in,
    /// as for the frame times with `--set time_from_ready=1`, see session.rs
    pub fn restart(&mut self) {
        self.totals.clear();
        self.frames = 0;
        self.waited = 0.0;
        self.waits = 0;
    }

    /// The average milliseconds per frame on each part, and in all, the last frames' too
    pub fn report(&mut self) -> String {
        for k in 0..POOLS {
            self.add_up(k, QueryResultFlags::empty());
        }
        let frames = self.frames.max(1) as f64;
        let parts: Vec<String> = self.totals.iter().map(|(name, total)| format!("{name} {:.2}", total / frames)).collect();
        let all: f64 = self.totals.iter().map(|(_, total)| total).sum::<f64>() / frames;
        let waited = self.waited / self.waits.max(1) as f64;
        format!("GPU per frame: {all:.2} ms ({}), CPU waited for it {waited:.2} ms, timed over {} frames", parts.join(", "), self.frames)
    }
}
