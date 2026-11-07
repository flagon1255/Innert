//! How long the GPU spends on each part of a frame, for screenshot runs: timestamps written between
//! the passes and read back once the frame's finished, see Capture in main.rs. Also how long the CPU
//! waited for it, which leaves how long the CPU took itself.

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

pub struct GpuTimer {
    pool: Arc<QueryPool>,
    /// Nanoseconds per tick of the timestamps
    period: f64,
    /// The bits of them that count: 36 on Intel's integrated graphics, which go round every hour or
    /// so, 64 on NVIDIA's
    mask: u64,
    /// The parts timed so far this frame, in order
    names: Vec<&'static str>,
    /// Milliseconds spent on each part over all the frames, in the order they first came
    totals: Vec<(&'static str, f64)>,
    frames: u32,
    /// Milliseconds the CPU spent waiting for the GPU to finish frames, in all
    waited: f64,
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
        let pool = QueryPool::new(
            device.clone(),
            QueryPoolCreateInfo { query_count: MOST_MARKS, ..QueryPoolCreateInfo::query_type(QueryType::Timestamp) },
        )
        .unwrap();
        let period = device.physical_device().properties().timestamp_period as f64;
        let mask = if bits >= 64 { u64::MAX } else { (1u64 << bits) - 1 };
        Some(GpuTimer { pool, period, mask, names: Vec::new(), totals: Vec::new(), frames: 0, waited: 0.0 })
    }

    /// Starts timing a frame, before any of its commands
    pub fn begin(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        self.names.clear();
        unsafe {
            builder.reset_query_pool(self.pool.clone(), 0..MOST_MARKS).unwrap();
            builder.write_timestamp(self.pool.clone(), 0, PipelineStage::AllCommands).unwrap();
        }
    }

    /// Ends the part of the frame called `name`: the commands recorded since the last mark
    pub fn mark(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, name: &'static str) {
        let query = self.names.len() as u32 + 1;
        if query >= MOST_MARKS {
            return;
        }
        self.names.push(name);
        unsafe {
            builder.write_timestamp(self.pool.clone(), query, PipelineStage::AllCommands).unwrap();
        }
    }

    /// Adds up the parts of the frame once the GPU's finished it, `waited` being how long the CPU
    /// waited for that
    pub fn finish(&mut self, waited: Duration) {
        let count = self.names.len() + 1;
        if count < 2 {
            return;
        }
        let mut ticks = vec![0u64; count];
        if !self.pool.get_results(0..count as u32, &mut ticks, QueryResultFlags::WAIT).unwrap_or(false) {
            return;
        }
        for (k, &name) in self.names.iter().enumerate() {
            let ms = (ticks[k + 1].wrapping_sub(ticks[k]) & self.mask) as f64 * self.period / 1e6;
            match self.totals.iter_mut().find(|(n, _)| *n == name) {
                Some((_, total)) => *total += ms,
                None => self.totals.push((name, ms)),
            }
        }
        self.frames += 1;
        self.waited += waited.as_secs_f64() * 1000.0;
    }

    /// The average milliseconds per frame on each part, and in all
    pub fn report(&self) -> String {
        let frames = self.frames.max(1) as f64;
        let parts: Vec<String> = self.totals.iter().map(|(name, total)| format!("{name} {:.2}", total / frames)).collect();
        let all: f64 = self.totals.iter().map(|(_, total)| total).sum::<f64>() / frames;
        format!("GPU per frame: {all:.2} ms ({}), CPU waited for it {:.2} ms", parts.join(", "), self.waited / frames)
    }
}
