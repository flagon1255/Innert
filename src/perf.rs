//! How much of the graphics card's memory there is room for, which the render scale is kept within
//! in play, see vram() and render_bytes(). And for long screenshot runs: `--set perf_log=60` says
//! every 60 seconds how the frames went, how much of the graphics card's memory is used, and
//! what's piled up, to see whether anything gets slower the longer the game's played.
//! `--set sky_check=30` looks every 30 frames at how much of the view is sky, to catch frames the
//! world's gone missing from.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use vulkano::{VulkanObject, device::Device};

/// The system allocator, counting how much is allocated in screenshot runs, see heap() and
/// count_heap()
pub struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static HEAP: AtomicIsize = AtomicIsize::new(0);
static HEAP_PEAK: AtomicIsize = AtomicIsize::new(0);

/// Starts counting what's allocated, see heap(): only in screenshot runs, as every allocation
/// counted on every thread at once costs a little
pub fn count_heap() {
    COUNTING.store(true, Ordering::Relaxed);
}

fn grew(by: isize) {
    if COUNTING.load(Ordering::Relaxed) {
        let now = HEAP.fetch_add(by, Ordering::Relaxed) + by;
        if by > 0 {
            HEAP_PEAK.fetch_max(now, Ordering::Relaxed);
        }
    }
}

// SAFETY: passes everything straight on to the system allocator
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            grew(layout.size() as isize);
        }
        p
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            grew(layout.size() as isize);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        grew(-(layout.size() as isize));
    }

    unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, size) };
        if !q.is_null() {
            grew(size as isize - layout.size() as isize);
        }
        q
    }
}

/// How many bytes the game has allocated now, and the most it has had at once since the last time
/// this was asked, counted from when count_heap() was: what was allocated before and let go of
/// after is taken off, so it starts out a little under
pub fn heap() -> (usize, usize) {
    let now = HEAP.load(Ordering::Relaxed);
    (now.max(0) as usize, HEAP_PEAK.swap(now, Ordering::Relaxed).max(0) as usize)
}

/// `--set vram_budget=2048`: the most of the graphics card's memory, in MB, the game takes it to
/// have room for, as on a smaller card than the one it's on
static VRAM_CAP: AtomicU64 = AtomicU64::new(u64::MAX);

pub fn cap_vram(megabytes: u64) {
    VRAM_CAP.store(megabytes << 20, Ordering::Relaxed);
}

/// How much of the graphics card's own memory this process may use before Windows starts moving
/// some of it out to the computer's memory, and how much it's using, in bytes, summed over the
/// card's own heaps, see VK_EXT_memory_budget. Where the driver can't say, four fifths of how big
/// they are, with nothing known to be used. No more than `--set vram_budget` says, see cap_vram().
pub fn vram(device: &Device) -> Option<Vram> {
    card_vram(device.physical_device(), device.enabled_extensions().ext_memory_budget)
}

/// The same for a graphics card before the game's started on it, see vram(), asking the driver
/// with `budget` if the card can say
pub fn card_vram(physical: &std::sync::Arc<vulkano::device::physical::PhysicalDevice>, budget: bool) -> Option<Vram> {
    let cap = VRAM_CAP.load(Ordering::Relaxed);
    if !budget {
        let heaps = &physical.memory_properties().memory_heaps;
        let size: u64 = heaps.iter().filter(|h| h.flags.intersects(vulkano::memory::MemoryHeapFlags::DEVICE_LOCAL)).map(|h| h.size).sum();
        return (size > 0).then(|| Vram { budget: (size / 5 * 4).min(cap), used: 0 });
    }
    let mut budget = ash::vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
    let mut properties = ash::vk::PhysicalDeviceMemoryProperties2::default().push_next(&mut budget);
    let fns = physical.instance().fns();
    // SAFETY: the physical device belongs to the instance, and supports the extension, which is
    // all asking it about its memory takes
    unsafe { (fns.v1_1.get_physical_device_memory_properties2)(physical.handle(), &mut properties) };
    let heaps = properties.memory_properties;
    let mut vram = Vram { budget: 0, used: 0 };
    for i in 0..heaps.memory_heap_count as usize {
        if heaps.memory_heaps[i].flags.contains(ash::vk::MemoryHeapFlags::DEVICE_LOCAL) {
            vram.budget += budget.heap_budget[i];
            vram.used += budget.heap_usage[i];
        }
    }
    vram.budget = vram.budget.min(cap);
    (vram.budget > 0).then_some(vram)
}

#[derive(Clone, Copy, Debug)]
pub struct Vram {
    pub budget: u64,
    pub used: u64,
}

pub const MB: f64 = 1024.0 * 1024.0;

/// What's gathered between two reports
pub struct PerfLog {
    every: Duration,
    started: Instant,
    since: Instant,
    /// Milliseconds each frame took
    times: Vec<f32>,
    /// Times the world moved on to the next piece of the landscape
    pub moves: u32,
    /// Seconds each piece built meanwhile took, from starting it to moving over to it
    pub builds: Vec<f32>,
    /// Times everything sized to the render resolution was made again
    pub recreated: u32,
    /// How near the camera came to the edge of the world, in meters
    pub edge_min: f64,
    /// Seconds the edge of the world was nearer than where things fade into the sky, so the
    /// world could be seen to end
    pub cut: f64,
    /// How near the edge of the world came straight ahead of the camera, in meters, and the seconds
    /// it was nearer than where things fade into the sky, where it'd be seen
    pub ahead_min: f64,
    pub cut_ahead: f64,
    /// Shots taken of the world seen to end, and the frame of the last, see App::render()
    pub edge_shots: u32,
    pub last_edge_shot: usize,
    /// How much of the view was sky, each time it was looked at
    sky: Vec<f32>,
    /// Frames the world was missing from
    pub blank: u32,
    /// The most things traced at once
    pub instances: usize,
}

impl PerfLog {
    pub fn new(every: f64) -> Self {
        let now = Instant::now();
        PerfLog {
            every: Duration::from_secs_f64(every.max(1.0)),
            started: now,
            since: now,
            times: Vec::new(),
            moves: 0,
            builds: Vec::new(),
            recreated: 0,
            edge_min: f64::INFINITY,
            cut: 0.0,
            ahead_min: f64::INFINITY,
            cut_ahead: 0.0,
            edge_shots: 0,
            last_edge_shot: 0,
            sky: Vec::new(),
            blank: 0,
            instances: 0,
        }
    }

    pub fn frame(&mut self, ms: f32) {
        self.times.push(ms);
    }

    pub fn sky(&mut self, fraction: f32) {
        self.sky.push(fraction);
    }

    pub fn due(&self) -> bool {
        self.since.elapsed() >= self.every
    }

    /// Seconds since the run started
    pub fn elapsed(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// One line of what's been gathered since the last, with `more` on the end, and starts over
    pub fn report(&mut self, more: &str) -> String {
        let mut sorted = self.times.clone();
        sorted.sort_by(f32::total_cmp);
        let n = sorted.len();
        let at = |q: f64| if n == 0 { 0.0 } else { sorted[((n - 1) as f64 * q).round() as usize] };
        let average = if n == 0 { 0.0 } else { sorted.iter().sum::<f32>() / n as f32 };
        let sky_avg = if self.sky.is_empty() { -1.0 } else { self.sky.iter().sum::<f32>() / self.sky.len() as f32 };
        let sky_max = self.sky.iter().copied().fold(-1.0f32, f32::max);
        let line = format!(
            "PERF t={:.0} frames={n} avg={average:.2} p50={:.2} p99={:.2} max={:.1} over20={} over33={} over100={} moves={} build_max={:.1} recreated={} edge_min={:.0} cut={:.1} ahead_min={:.0} cut_ahead={:.1} sky_avg={sky_avg:.2} sky_max={sky_max:.2} blank={} instances={} | {more}",
            self.elapsed(),
            at(0.5),
            at(0.99),
            at(1.0),
            sorted.iter().filter(|&&t| t > 20.0).count(),
            sorted.iter().filter(|&&t| t > 33.3).count(),
            sorted.iter().filter(|&&t| t > 100.0).count(),
            self.moves,
            self.builds.iter().copied().fold(0.0f32, f32::max),
            self.recreated,
            if self.edge_min.is_finite() { self.edge_min } else { -1.0 },
            self.cut,
            if self.ahead_min.is_finite() { self.ahead_min } else { -1.0 },
            self.cut_ahead,
            self.blank,
            self.instances,
        );
        self.since = Instant::now();
        self.times.clear();
        self.moves = 0;
        self.builds.clear();
        self.recreated = 0;
        self.edge_min = f64::INFINITY;
        self.cut = 0.0;
        self.ahead_min = f64::INFINITY;
        self.cut_ahead = 0.0;
        self.sky.clear();
        self.blank = 0;
        self.instances = 0;
        line
    }
}

/// Where `--script play` has got to: it walks and runs about in legs, turning between them, stopping
/// now and then, fighting as it goes, so the world moves on many times over a long run
pub struct Play {
    pub leg: u32,
    pub heading: f64,
    /// Seconds into the run that the current leg started and ends
    pub leg_start: f64,
    pub leg_end: f64,
    /// Where the player was a little while ago and when, to turn away from what they're stuck on
    pub check: Option<(nalgebra::Vector3<f64>, f64)>,
    pub next_pack: f64,
    /// `--set churn=90`: seconds between changes to the settings, as a player trying them out
    pub churn: Option<f64>,
    pub next_churn: f64,
    pub churns: u32,
    /// `--set shot_every=120`: seconds between shots of the view
    pub shot_every: Option<f64>,
    pub next_shot: f64,
    /// Which minute of the run it's been fighting with what weapon for
    pub minute: Option<u64>,
    pub rng: u32,
}

impl Play {
    pub fn new(churn: Option<f64>, shot_every: Option<f64>) -> Self {
        Play {
            leg: 0,
            heading: 0.0,
            leg_start: 0.0,
            leg_end: 0.0,
            check: None,
            next_pack: 30.0,
            churn,
            next_churn: churn.unwrap_or(0.0),
            churns: 0,
            shot_every,
            next_shot: shot_every.unwrap_or(0.0),
            minute: None,
            rng: 0x9e37_79b9,
        }
    }

    /// 0 to 1
    pub fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng % 10_000) as f64 / 10_000.0
    }
}

/// How many bytes everything sized to the render or window resolution takes, see
/// get_render_images(): per rendered pixel the image traced into (4), two colors gathered over
/// frames (16 each), two depths (4 each), the guide (8), post's two (8 each) and bloom's two at a
/// sixteenth of the pixels (16 each), and per window pixel the resampled image (4) and upscale's
/// two (8 each)
pub fn render_bytes(render: [u32; 2], window: [u32; 2]) -> u64 {
    let rendered = render[0] as u64 * render[1] as u64;
    let bloom = render[0].div_ceil(4).max(1) as u64 * render[1].div_ceil(4).max(1) as u64;
    let shown = window[0] as u64 * window[1] as u64;
    rendered * (4 + 32 + 8 + 8 + 16) + bloom * 32 + shown * (4 + 16)
}

/// Seconds as minutes and seconds, for what's printed
pub fn clock(seconds: f64) -> String {
    format!("{}:{:02}", (seconds / 60.0) as u64, seconds as u64 % 60)
}
