//! How the world moves on over the endless landscape: a strip at a time. The world is kept in room
//! for a strip more than it is across each way, see World::window(), and each column of the
//! landscape is kept at where it is modulo that room, so nothing has to be moved as the world moves
//! on: the strip coming in is built in the background and sent over into the room left free by the
//! one that went, and the world's corner moves on by a strip, see App::scroll().
//!
//! There's room for one strip each way beyond the world, its slot. The strip that left last is kept
//! in it, so turning round and going back the way one came needs nothing built: it's only let go
//! of once the strip that's to go there is built. Only the strip coming in is ever built, half a
//! second to a second of the background threads' time, and it goes over to the GPU a few
//! megabytes a frame, copied with the frame itself, so no frame waits for it and nothing drawn
//! sees it before it's all there.
//!
//! That keeps up with about 16 to 20 m a second along one way. Flying faster, the free camera's
//! slowed on its way out from the middle of the world, see App::keep_up(), so the edge is never
//! seen; taken further at once, the world's built afresh round it behind a fade, see Teleport.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Condvar, Mutex};

use crate::worldgen::{Installing, PagePool, PieceFeatures, Releasing, Strip};

use super::*;

/// How wide the strips the world moves on by are, in meters. Its empty bricks skip no further than
/// a strip's edges, see World::keep_skips_inside(), which at 8 m made rays a few percent slower.
pub const STRIP_M: u32 = 16;
/// How far past the middle of the strip the camera's in the camera goes before the world moves on,
/// in meters, so standing about on the edge of one doesn't move it back and forth
const PAST_M: f64 = 1.0;
/// Layers of bricks put in or let go of each frame, see World::install_step(): a few hundred
/// microseconds' worth
const LAYERS_A_FRAME: u32 = 16;
/// How near the edge of the world the camera can come before it's moved on all at once, behind a
/// fade, in strips: only when it's going faster than strips can be built
const TELEPORT_STRIPS: f64 = 1.5;
/// Words in each piece of host memory what's built goes over to the GPU through, 4 MB, and the
/// most of them there are, see Staging
const STAGE_WORDS: usize = 1 << 20;
const STAGE_PIECES: usize = 12;
/// About how long the GPU takes to copy a piece over, in milliseconds, see staging.rs
const STAGE_MS: f64 = 0.35;
/// How long the screen takes to go dark, or light again, round moving the world on all at once
const FADE_SECONDS: f32 = 0.25;
/// Going faster than this, in meters a second, the next strip's started on as soon as the world's
/// moved on, rather than once the camera's past the middle of the strip it's in: flying, not
/// walking, where what's behind is kept as long as can be for turning round
const FAST_M_S: f64 = 10.0;
/// How far out from the middle of the world the free camera goes on out at full speed, and how
/// far at all, in strips, see App::keep_up(): only that far, less a strip, is the world behind,
/// and its edge is still well past where things fade into the sky
const SLOW_FROM_STRIPS: f64 = 1.0;
const STOP_AT_STRIPS: f64 = 2.5;
/// How much of where things fade into the sky the edge of the world can come within and still not
/// be seen, as screenshot runs count it, see perf::PerfLog::cut: within a strip of that, what's
/// built in the background is hurried, see worldgen::hurry()
const HURRY_FADE: f64 = 0.7;
/// A strip's hurried, too, once it's been this long on its way: on four cores one takes a second
/// or so to build and send over, but once one took four, its first step held up three
const HURRY_AFTER: Duration = Duration::from_secs(2);

/// What a part of the room beyond the world, each way, holds
#[derive(Default)]
struct Slot {
    /// The strip of the landscape it holds that's in use: its first column, and how many across. It
    /// can be moved on to straight away. None once the world's moved on the other way, as then it
    /// doesn't reach all the way along the world any more.
    holds: Option<([i64; 2], [u32; 2])>,
    /// What's in it, still to be let go of before another strip's put there
    content: Option<([i64; 2], [u32; 2])>,
    /// The frame since when nothing's been drawn with it
    free_since: u32,
}

/// The host memory strips go over to the GPU through: pieces of it are filled on the threads
/// building the strips and copied with the frames, see App::record_uploads(), then filled again
/// once those frames are done. Only so many, made as they're first needed.
pub struct Staging {
    free: Mutex<(Vec<Subbuffer<[u32]>>, usize)>,
    freed: Condvar,
    memory_allocator: Arc<StandardMemoryAllocator>,
}

impl Staging {
    /// All of it made at once, while loading: made as it was needed, making each held a frame up
    /// by 60 ms waiting on the driver
    pub fn new(memory_allocator: Arc<StandardMemoryAllocator>) -> Self {
        let staging = Staging { free: Mutex::new((Vec::new(), 0)), freed: Condvar::new(), memory_allocator };
        let made: Vec<Subbuffer<[u32]>> = (0..STAGE_PIECES).map(|_| staging.take()).collect();
        staging.give_back(made);
        staging
    }

    /// A piece to fill, waiting for one to be free if all there can be are in use
    fn take(&self) -> Subbuffer<[u32]> {
        let mut free = self.free.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(piece) = free.0.pop() {
                return piece;
            }
            if free.1 < STAGE_PIECES {
                free.1 += 1;
                return Buffer::new_slice::<u32>(
                    self.memory_allocator.clone(),
                    BufferCreateInfo { usage: BufferUsage::TRANSFER_SRC, ..Default::default() },
                    AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..Default::default() },
                    STAGE_WORDS as u64,
                )
                .unwrap();
            }
            free = self.freed.wait(free).unwrap_or_else(|e| e.into_inner());
        }
    }

    fn give_back(&self, pieces: impl IntoIterator<Item = Subbuffer<[u32]>>) {
        self.free.lock().unwrap_or_else(|e| e.into_inner()).0.extend(pieces);
        self.freed.notify_all();
    }
}

/// What a piece of a strip goes into on the GPU, see App::record_uploads()
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Grid,
    Bricks,
    Grass,
    WaterGround,
    WaterDepth,
    WaterFlux,
}

/// A piece of a strip on its way to the GPU: the host memory it's in, how much of it, and where it
/// goes, as copies into an image or into the bricks
struct Upload {
    piece: Subbuffer<[u32]>,
    words: usize,
    target: Target,
    images: Vec<BufferImageCopy>,
    buffers: Vec<BufferCopy>,
    /// Whether the host memory's Staging's, to be filled again, rather than made for it alone
    pooled: bool,
}

/// A strip of the landscape being built in the background, and sent over to the GPU. Dropped, it's
/// called off: the thread building it gives its pages back, see run_job().
struct Job {
    axis: usize,
    forward: bool,
    /// Where the world's corner was when it was started: the strip only goes in if it's still there
    base: [i64; 2],
    from_job: Receiver<FromJob>,
    /// Till it's told to send the strip over, once there's room for it: called off till then
    /// without anything being lost, see App::plan_next()
    to_job: Option<Sender<()>>,
    /// Once it's built: how many pages its bricks are in
    built: Option<usize>,
    started: Instant,
    /// How many of App::craters and App::stains it's built with: those since are blown in it once
    /// it's in
    craters: usize,
    stains: usize,
}

enum FromJob {
    Built { pages: usize },
    Piece(Upload),
    Sent(Box<Arrival>),
}

/// A strip that's been built and sent over, and what's needed to put it in
struct Arrival {
    strip: Strip,
    features: PieceFeatures,
    /// Where the rivers flow into the world once it's moved on to the strip, see window_springs()
    springs: Vec<[f32; 4]>,
    built: Duration,
    sent: Duration,
}

/// A strip being put in, a few layers a frame, and what's to be done with it once it's in
struct Pending {
    installing: Installing,
    features: PieceFeatures,
    springs: Vec<[f32; 4]>,
    axis: usize,
    forward: bool,
    started: Instant,
    craters: usize,
    stains: usize,
    /// Whether all its layers are in
    done: bool,
    /// Whether it's only just arrived, this frame
    fresh: bool,
}

/// A strip that's just been put in, for App::scroll(): where it is, what it has besides its voxels,
/// where rivers flow into the world once it's moved on to it, and how many holes and stains it was
/// built with
struct Arrived {
    origin: [i64; 2],
    size: [u32; 2],
    features: PieceFeatures,
    springs: Vec<[f32; 4]>,
    craters: usize,
    stains: usize,
}

/// How strips have been coming in, for perf logs
#[derive(Default)]
pub struct StreamStats {
    pub strips: u32,
    pub reused: u32,
    pub teleports: u32,
    pub discarded: u32,
    /// Milliseconds each strip took to build, to send over, and in all from starting it to moving
    /// on to it
    pub built_ms: Vec<f32>,
    pub sent_ms: Vec<f32>,
    pub total_ms: Vec<f32>,
    /// The longest a frame spent putting a strip in, letting go of one, or moving on, in ms
    pub frame_ms_max: f32,
    /// Megabytes copied with the frames, and the most in one
    pub uploaded_mb: f32,
    pub upload_frame_mb: f32,
}

pub struct Stream {
    /// How many columns across a strip is
    pub strip: u32,
    slots: [Slot; 2],
    job: Option<Job>,
    installing: Option<Pending>,
    releasing: Option<(usize, Releasing)>,
    /// Pages let go of, and the frame they were, to give back to the pool once nothing's drawn
    /// with them any more, see RETIRED_FRAMES
    quarantine: VecDeque<(u32, Vec<u32>)>,
    /// Pieces of strips waiting to go over with the frames, and those gone, with the frame they went
    /// with, to fill again once it's done
    uploads: VecDeque<Upload>,
    returning: VecDeque<(u32, Vec<Subbuffer<[u32]>>)>,
    staging: Arc<Staging>,
    /// What each strip the world's made of has besides its voxels, see adopt_features(): those
    /// reaching into the world
    pub pieces: Vec<PieceFeatures>,
    /// Where the rivers flow into the world where it is now, see window_springs()
    river_springs: Vec<[f32; 4]>,
    pub stats: StreamStats,
    /// Moving the world on all at once, behind a fade, when it couldn't keep up
    pub teleport: Option<Teleport>,
    /// How far the empty bricks skip, worked out again where strips join the world, see skips.rs
    pub skips: crate::skips::Skips,
}

/// Building the world afresh round where the camera's got to, behind a fade, when it's going too
/// fast for the strips to keep up, see App::update_teleport(): all of it as one strip, into pages of
/// its own
pub struct Teleport {
    job: Job,
    pool: Arc<Mutex<PagePool>>,
    /// The world's corner it's built at
    origin: [i64; 2],
    arrival: Option<Box<Arrival>>,
    /// How dark the screen is, 0 to 1, and whether it's getting lighter again
    pub dark: f32,
    lighter: bool,
    last: Instant,
}

impl Stream {
    pub fn new(strip: u32, pieces: Vec<PieceFeatures>, river_springs: Vec<[f32; 4]>, memory_allocator: Arc<StandardMemoryAllocator>) -> Self {
        Stream {
            strip,
            slots: Default::default(),
            job: None,
            installing: None,
            releasing: None,
            quarantine: VecDeque::new(),
            uploads: VecDeque::new(),
            returning: VecDeque::new(),
            staging: Arc::new(Staging::new(memory_allocator)),
            pieces,
            river_springs,
            stats: StreamStats::default(),
            teleport: None,
            skips: Default::default(),
        }
    }

    /// Whether a strip's being built, sent over or put in
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.installing.is_some() || !self.uploads.is_empty()
    }

    /// The springs of the world where it is now: where rivers flow in, and those of its strips
    pub fn springs(&self) -> Vec<[f32; 4]> {
        let mut springs = self.river_springs.clone();
        for piece in &self.pieces {
            springs.extend(piece.springs.iter().filter(|s| (0..2).all(|i| s[i] >= piece.lo[i] && s[i] < piece.hi[i])));
        }
        springs
    }

    /// Takes what the job building a strip has to say since last time, the pieces of it to go over
    /// with the frames among it
    fn hear(&mut self, from_job: &Receiver<FromJob>) -> Result<Option<FromJob>, ()> {
        loop {
            match from_job.try_recv() {
                Ok(FromJob::Piece(upload)) => self.uploads.push_back(upload),
                Ok(other) => return Ok(Some(other)),
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => return Err(()),
            }
        }
    }
}

/// Where the columns from `lo`, `n` across along one way, are kept in `room`: each the first kept,
/// how far into them it is, and how many, split where they go round
fn spans(lo: i64, n: u32, room: u32) -> Vec<(u32, u32, u32)> {
    let first = lo.rem_euclid(room as i64) as u32;
    if first + n <= room { vec![(first, 0, n)] } else { vec![(first, 0, room - first), (0, room - first, n - (room - first))] }
}

/// Fills `len` words a piece at a time, `whole` of them at least to a piece, into the host memory
/// they go over through, `fill` writing the words from the given one on into the piece it's handed,
/// and sends each to be copied with the frames to wherever `copies` says. False if called off.
fn stage(
    staging: &Staging,
    to_main: &Sender<FromJob>,
    len: usize,
    whole: usize,
    target: Target,
    fill: impl Fn(&mut [u32], usize),
    copies: impl Fn(usize, usize) -> (Vec<BufferImageCopy>, Vec<BufferCopy>),
) -> bool {
    let per_piece = (STAGE_WORDS / whole.max(1)).max(1) * whole.max(1);
    let mut first = 0;
    while first < len {
        let n = per_piece.min(len - first);
        let piece = staging.take();
        fill(&mut piece.clone().slice(0..n as u64).write().unwrap(), first);
        let (images, buffers) = copies(first, n);
        if to_main.send(FromJob::Piece(Upload { piece, words: n, target, images, buffers, pooled: true })).is_err() {
            return false;
        }
        first += n;
    }
    true
}

/// Sends a strip that's been built over to the GPU, where it goes in `room` columns, see
/// World::phys: its bricks' grid, its pages of bricks, its grass and its water, as pieces to be
/// copied with the frames. Nothing's drawn with any of it till the world moves on to it.
fn send(staging: &Staging, to_main: &Sender<FromJob>, strip: &Strip, room: [u32; 2], water: bool) -> bool {
    let [bx, by] = strip.size.map(|n| n / 8);
    // The grid, whole layers of it to a piece, in one or two parts each where it goes round
    let rects: Vec<((u32, u32, u32), (u32, u32, u32))> = spans(strip.origin[1].div_euclid(8), by, room[1] / 8)
        .into_iter()
        .flat_map(|y| spans(strip.origin[0].div_euclid(8), bx, room[0] / 8).into_iter().map(move |x| (x, y)))
        .collect();
    let layer = (bx * by) as usize;
    let grid = stage(
        staging,
        to_main,
        strip.cells.len(),
        layer,
        Target::Grid,
        |piece, first| piece.copy_from_slice(&strip.cells[first..first + piece.len()]),
        |first, n| {
            let (z, layers) = ((first / layer) as u32, (n / layer) as u32);
            let images = rects
                .iter()
                .map(|&((kx, ix, nx), (ky, iy, ny))| BufferImageCopy {
                    buffer_offset: ((ix + iy * bx) * 4) as u64,
                    buffer_row_length: bx,
                    buffer_image_height: by,
                    image_offset: [kx, ky, z],
                    image_extent: [nx, ny, layers],
                    ..Default::default()
                })
                .collect();
            (images, Vec::new())
        },
    );
    if !grid {
        return false;
    }

    // The pages, a few to a piece, each to where it is in the bricks
    let page_words = worldgen::PAGE_BRICKS * worldgen::BRICK_WORDS;
    let pages = &strip.pages;
    let bricks = stage(
        staging,
        to_main,
        pages.len() * page_words,
        page_words,
        Target::Bricks,
        |piece, first| {
            for (k, chunk) in piece.chunks_mut(page_words).enumerate() {
                chunk.copy_from_slice(&pages[first / page_words + k].2);
            }
        },
        |first, n| {
            let buffers = (0..n / page_words)
                .map(|k| {
                    let (id, used, _) = &pages[first / page_words + k];
                    BufferCopy {
                        src_offset: (k * page_words * 4) as u64,
                        dst_offset: *id as u64 * page_words as u64 * 4,
                        size: (*used as u64).max(1) * worldgen::BRICK_WORDS as u64 * 4,
                        ..Default::default()
                    }
                })
                .collect();
            (Vec::new(), buffers)
        },
    );
    if !bricks {
        return false;
    }

    // Its grass and water, rows of columns at a time, in up to two parts where they go round
    let [sx, sy] = strip.size;
    let column_rects: Vec<((u32, u32, u32), (u32, u32, u32))> =
        spans(strip.origin[1], sy, room[1]).into_iter().flat_map(|y| spans(strip.origin[0], sx, room[0]).into_iter().map(move |x| (x, y))).collect();
    let columns = |target: Target, words: usize, value: &dyn Fn(usize, &mut [u32])| -> bool {
        column_rects.iter().all(|&((kx, ix, nx), (ky, iy, ny))| {
            let row = nx as usize * words;
            stage(
                staging,
                to_main,
                row * ny as usize,
                row,
                target,
                |piece, first| {
                    for (k, column) in piece.chunks_mut(words).enumerate() {
                        let i = first / words + k;
                        let (x, y) = (i % nx as usize, i / nx as usize);
                        value(ix as usize + x + (iy as usize + y) * sx as usize, column);
                    }
                },
                |first, n| {
                    let (y, rows) = ((first / row) as u32, (n / row) as u32);
                    (vec![BufferImageCopy { image_offset: [kx, ky + y, 0], image_extent: [nx, rows, 1], ..Default::default() }], Vec::new())
                },
            )
        })
    };
    if !strip.grass.is_empty() && !columns(Target::Grass, 1, &|i, w| w[0] = strip.grass[i]) {
        return false;
    }
    if water && !strip.ground.is_empty() {
        let (ground, surface) = (&strip.ground, &strip.surface);
        let at = |v: &[f32], i: usize| v.get(i).copied().unwrap_or(0.0);
        if !columns(Target::WaterGround, 1, &|i, w| w[0] = at(ground, i).to_bits())
            || !columns(Target::WaterDepth, 1, &|i, w| w[0] = (at(surface, i) - at(ground, i)).max(0.0).to_bits())
            // Half floats four to a column, all zero: none of it flowing anywhere yet
            || !columns(Target::WaterFlux, 2, &|_, w| w.fill(0))
        {
            return false;
        }
    }
    true
}

/// What a strip is built with, see start_job()
struct JobInput {
    settings: TerrainSettings,
    rivers: Arc<RiverCache>,
    origin: [i64; 2],
    size: [u32; 2],
    /// The world's corner once it's moved on to the strip, and how many columns across it is, for
    /// where the rivers flow into it then
    next_corner: [i64; 2],
    world_size: [u32; 2],
    craters: Vec<([f64; 3], f64)>,
    stains: Vec<([f64; 3], f64, u8, bool)>,
    pool: Arc<Mutex<PagePool>>,
    staging: Arc<Staging>,
    /// How many columns there's room for each way, see World::phys, and whether there's water
    room: [u32; 2],
    water: bool,
}

/// Builds a strip of the landscape, with the holes blown and the stains splashed in it, then once
/// told there's room for it, sends it over to the GPU a piece at a time
fn run_job(input: JobInput, to_main: Sender<FromJob>, from_main: Receiver<()>) {
    worldgen::give_way();
    let started = Instant::now();
    let JobInput { settings, rivers, origin, size, next_corner, world_size, craters, stains, pool, staging, room, water } = input;
    let (terrain, mut world) = worldgen::generate_area(&settings, origin, size, &rivers);
    // The holes blown in the landscape are still there, and the stains splashed on it
    let vpm = world.voxels_per_meter as f64;
    let local = |world: &World, center: [f64; 3], radius: f64| {
        let p = [center[0] * vpm - world.origin[0] as f64, center[1] * vpm - world.origin[1] as f64, center[2] * vpm];
        let reach = radius * vpm + 2.0;
        (p[0] > -reach && p[1] > -reach && p[0] < world.size[0] as f64 + reach && p[1] < world.size[1] as f64 + reach).then_some(p)
    };
    for (center, radius) in craters {
        if let Some(p) = local(&world, center, radius) {
            world.carve(p, radius * vpm);
        }
    }
    for (center, radius, material, burn) in stains {
        if let Some(p) = local(&world, center, radius) {
            world.paint(p, radius * vpm, material, burn);
        }
    }
    let features = terrain.piece_features();
    drop(terrain);
    let m = |v: i64| v as f32 / vpm as f32;
    let springs = worldgen::window_springs(&settings, &rivers, [m(next_corner[0]), m(next_corner[1])], [m(next_corner[0] + world_size[0] as i64), m(next_corner[1] + world_size[1] as i64)]);
    let strip = Strip::new(world, &pool, room[0] - world_size[0]);
    let built = started.elapsed();
    // Called off, its pages go back: nothing's drawn with them
    let give_back = |strip: &Strip| pool.lock().unwrap_or_else(|e| e.into_inner()).give_back(strip.pages.iter().map(|p| p.0));
    // Told there's room for it, or called off
    if to_main.send(FromJob::Built { pages: strip.pages.len() }).is_err() || from_main.recv().is_err() {
        give_back(&strip);
        return;
    }
    let sending = Instant::now();
    if !send(&staging, &to_main, &strip, room, water) {
        give_back(&strip);
        return;
    }
    let sent = sending.elapsed();
    if let Err(std::sync::mpsc::SendError(FromJob::Sent(arrival))) = to_main.send(FromJob::Sent(Box::new(Arrival { strip, features, springs, built, sent }))) {
        give_back(&arrival.strip);
    }
}

impl App {
    /// The strip of the landscape just past the edge of the world along `axis`, the way `forward`
    /// says, where the world is now: its first column and how many across
    fn incoming(&self, strip: u32, axis: usize, forward: bool) -> ([i64; 2], [u32; 2]) {
        let strip = strip as i64;
        let origin = [self.world.origin[0], self.world.origin[1]];
        let w = self.world.size[axis] as i64;
        let mut lo = origin;
        lo[axis] = if forward { origin[axis] + w } else { origin[axis] - strip };
        let mut size = [self.world.size[0], self.world.size[1]];
        size[axis] = strip as u32;
        (lo, size)
    }

    /// What a job building a strip of the landscape from `origin`, `size` columns across, takes,
    /// into pages from `pool`, the world's corner moving on to `next_corner` once it's in
    fn job_input(&self, stream: &Stream, origin: [i64; 2], size: [u32; 2], next_corner: [i64; 2], pool: Arc<Mutex<PagePool>>) -> JobInput {
        JobInput {
            settings: self.terrain_settings(),
            rivers: self.rivers.clone(),
            origin,
            size,
            next_corner,
            world_size: [self.world.size[0], self.world.size[1]],
            craters: self.craters.clone(),
            stains: self.stains.clone(),
            pool,
            staging: stream.staging.clone(),
            room: self.world.phys,
            water: self.water.active,
        }
    }

    /// Starts building the strip coming in along `axis` the way `forward` says. What its slot holds
    /// is kept till it's built, see update_window(), in case it's turned back to meanwhile.
    fn start_job(&mut self, stream: &mut Stream, axis: usize, forward: bool) {
        let (origin, size) = self.incoming(stream.strip, axis, forward);
        let mut next_corner = [self.world.origin[0], self.world.origin[1]];
        next_corner[axis] += if forward { stream.strip as i64 } else { -(stream.strip as i64) };
        let input = self.job_input(stream, origin, size, next_corner, self.world.page_pool().expect("the world that moves on has pages"));
        let (to_main, from_job) = channel();
        let (to_job, from_main) = channel();
        std::thread::Builder::new().name("strip".into()).spawn(move || run_job(input, to_main, from_main)).unwrap();
        let base = [self.world.origin[0], self.world.origin[1]];
        stream.job = Some(Job { axis, forward, base, from_job, to_job: Some(to_job), built: None, started: Instant::now(), craters: self.craters.len(), stains: self.stains.len() });
    }

    /// Room on the GPU for every page there is, `pages` of them: made now, before anything's sent
    /// into it, the bricks there are copied over with the next frame, see grow_with_frame(), with
    /// a tenth more so it's seldom needed again
    fn room_for_pages(&mut self, pages: usize) {
        let needed = pages * worldgen::PAGE_BRICKS;
        if needed > self.voxel_set.capacity {
            let started = Instant::now();
            self.voxel_set.grow_with_frame(needed, spare_bricks().max(needed / 10), &self.memory_allocator, &self.descriptor_set_allocator, &self.render_pipeline);
            println!("Made room on the GPU for {} bricks in {:.1} ms", self.voxel_set.capacity, started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    /// Moves the world on as the camera goes, a strip at a time: builds the strip it's heading
    /// into, sends it over, puts it in a few layers a frame and moves on to it once it's in. Called
    /// every frame.
    pub(super) fn update_window(&mut self) {
        let Some(mut stream) = self.stream.take() else {
            return;
        };
        let frame_start = Instant::now();
        // What's built in the background gives way to the game, see worldgen::give_way(), unless
        // the edge of the world's getting to within a strip of where it'd be seen, the strip being
        // built is taking far longer than it should, held up by what else the computer's doing, or
        // the world's being built afresh behind a fade
        let camera = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let edge = (0..2).map(|i| self.world.size[i] as f64 * 0.5 - (camera[i] - self.world.size[i] as f64 * 0.5).abs()).fold(f64::INFINITY, f64::min);
        let fade = fade_distance(self.world.size, stream.strip, self.world.voxels_per_meter as f64, self.view_distance);
        let held_up = stream.job.as_ref().is_some_and(|job| job.started.elapsed() > HURRY_AFTER);
        worldgen::hurry(stream.teleport.is_some() || held_up || edge < HURRY_FADE * fade + stream.strip as f64);
        // Pages, and pieces of host memory, nothing's drawn with any more, to be filled again
        let released = self.world.take_released();
        if !released.is_empty() {
            stream.quarantine.push_back((self.frame, released));
        }
        while stream.quarantine.front().is_some_and(|(frame, _)| self.frame.wrapping_sub(*frame) >= RETIRED_FRAMES) {
            let (_, pages) = stream.quarantine.pop_front().unwrap();
            if let Some(pool) = self.world.page_pool() {
                pool.lock().unwrap_or_else(|e| e.into_inner()).give_back(pages);
            }
        }
        while stream.returning.front().is_some_and(|(frame, _)| self.frame.wrapping_sub(*frame) >= RETIRED_FRAMES) {
            let (_, pieces) = stream.returning.pop_front().unwrap();
            stream.staging.give_back(pieces);
        }
        if stream.teleport.is_some() {
            self.update_teleport(&mut stream);
            self.stream = Some(stream);
            return;
        }
        // Letting go of what was in the slot a strip's coming into
        if let Some((_, releasing)) = &mut stream.releasing
            && self.world.release_step(releasing, LAYERS_A_FRAME)
        {
            stream.releasing = None;
        }
        let released_at = frame_start.elapsed();

        // What the strip being built has to say
        if let Some(job) = stream.job.take() {
            let mut keep = Some(job);
            let heard = stream.hear(&keep.as_ref().unwrap().from_job);
            match heard {
                Ok(Some(FromJob::Built { pages })) => keep.as_mut().unwrap().built = Some(pages),
                Ok(Some(FromJob::Sent(arrival))) => {
                    let job = keep.take().unwrap();
                    let Arrival { strip, features, springs, built, sent } = *arrival;
                    // Still where it goes, unless the world's moved on another way meanwhile
                    if self.world.origin[..2] == job.base {
                        stream.stats.built_ms.push(built.as_secs_f32() * 1000.0);
                        stream.stats.sent_ms.push(sent.as_secs_f32() * 1000.0);
                        // Its water on the CPU now, a frame before it's put in: where it goes was let
                        // go of, and nothing's drawn there till the world moves on to it
                        self.water.take_strip(strip.origin, strip.size, &strip.ground, &strip.surface);
                        let installing = World::install(strip);
                        stream.installing = Some(Pending { installing, features, springs, axis: job.axis, forward: job.forward, started: job.started, craters: job.craters, stains: job.stains, done: false, fresh: true });
                    } else {
                        // Its pages go back, nothing drawn with them
                        stream.stats.discarded += 1;
                        if let Some(pool) = self.world.page_pool() {
                            pool.lock().unwrap_or_else(|e| e.into_inner()).give_back(strip.pages.iter().map(|p| p.0));
                        }
                    }
                }
                Ok(Some(FromJob::Piece(_))) | Ok(None) => {}
                Err(()) => {
                    println!("A strip of the world couldn't be built");
                    keep = None;
                }
            }
            stream.job = keep;
        }
        let heard_at = frame_start.elapsed();
        // Once it's built, what its slot holds is let go of, a few layers a frame, if it's still
        // where the camera's heading
        if let Some(job) = &stream.job
            && job.built.is_some()
            && job.to_job.is_some()
            && stream.releasing.is_none()
            && stream.slots[job.axis].content.is_some()
            && self.wanted(&stream, job.axis, job.forward)
            && let Some((lo, n)) = stream.slots[job.axis].content.take()
        {
            stream.slots[job.axis].holds = None;
            stream.releasing = Some((job.axis, World::release(lo, [lo[0] + n[0] as i64, lo[1] + n[1] as i64])));
        }
        // Then once nothing drawn has it any more, it's told to send the strip over, if it's still
        // where it goes
        if stream.job.as_ref().is_some_and(|j| j.to_job.is_some() && self.world.origin[..2] != j.base) {
            stream.job = None;
        }
        if let Some(job) = &stream.job
            && job.built.is_some()
            && job.to_job.is_some()
            && stream.releasing.is_none()
            && stream.slots[job.axis].content.is_none()
            && self.frame.wrapping_sub(stream.slots[job.axis].free_since) >= RETIRED_FRAMES
        {
            stream.slots[job.axis].holds = None;
            let pages = self.world.page_pool().map_or(0, |p| p.lock().unwrap_or_else(|e| e.into_inner()).count()) as usize;
            self.room_for_pages(pages);
            if let Some(to_job) = stream.job.as_mut().and_then(|j| j.to_job.take()) {
                let _ = to_job.send(());
            }
        }
        let roomed_at = frame_start.elapsed();

        // Putting a strip in, and moving on to it once it's in and all of it's gone over
        let mut put_in = Duration::ZERO;
        // Not in the frame it's arrived in, whose water's been taken in, a few milliseconds
        if let Some(pending) = &mut stream.installing
            && !std::mem::take(&mut pending.fresh)
            && (pending.done || {
                let putting = Instant::now();
                let done = self.world.install_step(&mut pending.installing, LAYERS_A_FRAME);
                put_in = putting.elapsed();
                done
            })
        {
            pending.done = true;
            if stream.uploads.is_empty() {
                let Pending { installing, features, springs, axis, forward, started, craters, stains, .. } = stream.installing.take().unwrap();
                let strip = installing.strip;
                stream.stats.total_ms.push(started.elapsed().as_secs_f32() * 1000.0);
                stream.stats.strips += 1;
                self.scroll(&mut stream, axis, forward, Some(Arrived { origin: strip.origin, size: strip.size, features, springs, craters, stains }));
                // Tens of megabytes of maps, let go of aside
                std::thread::spawn(move || drop(strip));
            }
        }

        let installed = frame_start.elapsed();

        // Where the world's to move on to next
        if stream.installing.is_none() {
            self.plan_next(&mut stream);
        }
        let took = frame_start.elapsed().as_secs_f32() * 1000.0;
        stream.stats.frame_ms_max = stream.stats.frame_ms_max.max(took);
        // Screenshot runs say what held a frame up
        if took > 4.0
            && let Some(capture) = &self.capture
        {
            let ms = |d: Duration| d.as_secs_f32() * 1000.0;
            println!(
                "Strips took {took:.1} ms at frame {}: letting go {:.1} ms, hearing {:.1} ms, room {:.1} ms, putting in {:.1} ms, moving on {:.1} ms, what's next {:.1} ms",
                capture.frame_times.len() + 1,
                ms(released_at),
                ms(heard_at - released_at),
                ms(roomed_at - heard_at),
                ms(put_in),
                ms(installed - roomed_at - put_in),
                took - ms(installed)
            );
        }
        self.stream = Some(stream);
    }

}

impl Stream {
    /// Copies pieces of strips coming in over to the GPU with this frame, `frame` taking about
    /// `frame_ms`: as much as about a tenth of a frame's time, or all there is while the screen's
    /// dark, see Teleport, into the world's grid, bricks, grass and water, `voxel_set` and `water`,
    /// whatever they are by now
    pub(super) fn record_uploads(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>, voxel_set: &GpuWorld, water: &Water, frame: u32, frame_ms: f64) {
        let stream = self;
        if stream.uploads.is_empty() {
            return;
        }
        let dark = stream.teleport.as_ref().is_some_and(|t| t.dark >= 1.0);
        let most = if dark { usize::MAX } else { ((frame_ms * 0.1 / STAGE_MS) as usize).clamp(1, STAGE_PIECES / 2) };
        let mut gone = Vec::new();
        let mut words = 0usize;
        let grass = voxel_set.views.iter().find(|(b, _)| *b == 2).map(|(_, v)| v.image().clone());
        let water = water.images();
        // Holes blown in a strip just in, see send_edits(), all go with the first frame it's in
        while (gone.len() < most || stream.uploads.front().is_some_and(|u| !u.pooled))
            && let Some(upload) = stream.uploads.pop_front()
        {
            let piece = upload.piece.clone().slice(0..upload.words as u64);
            let image = match upload.target {
                Target::Grid => Some(voxel_set.grid.clone()),
                Target::Grass => grass.clone(),
                Target::WaterGround => Some(water.ground.clone()),
                Target::WaterDepth => Some(water.depth.clone()),
                Target::WaterFlux => Some(water.flux.clone()),
                Target::Bricks => None,
            };
            match image {
                Some(image) => {
                    let regions: Vec<BufferImageCopy> = upload.images.iter().map(|r| BufferImageCopy { image_subresource: image.subresource_layers(), ..r.clone() }).collect();
                    builder.copy_buffer_to_image(CopyBufferToImageInfo { regions: regions.into(), ..CopyBufferToImageInfo::buffer_image(piece, image) }).unwrap();
                }
                None if upload.target == Target::Bricks => {
                    builder.copy_buffer(CopyBufferInfo { regions: upload.buffers.clone().into(), ..CopyBufferInfo::buffers(piece, voxel_set.bricks.clone()) }).unwrap();
                }
                None => {}
            }
            words += upload.words;
            if upload.pooled {
                gone.push(upload.piece);
            }
        }
        let mb = words as f32 * 4.0 / (1 << 20) as f32;
        stream.stats.uploaded_mb += mb;
        stream.stats.upload_frame_mb = stream.stats.upload_frame_mb.max(mb);
        stream.returning.push_back((frame, gone));
    }
}

impl App {

    /// Decides which way the world moves on next, as far as the camera's gone from its middle and
    /// which way it's heading: straight away if the strip's already there, otherwise building it
    fn plan_next(&mut self, stream: &mut Stream) {
        let vpm = self.world.voxels_per_meter as f64;
        let camera = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let now = Instant::now();
        // Which way and how fast it's going over the ground, in voxels a second, smoothed
        if let Some((last, at)) = self.last_camera {
            let dt = (now - at).as_secs_f64();
            if dt > 1e-4 && dt < 0.5 {
                let velocity = Vector3::new(camera.x - last.x, camera.y - last.y, 0.0) / dt;
                self.camera_drift += (velocity - self.camera_drift) * (dt / 0.5).min(1.0);
            }
        }
        self.last_camera = Some((camera, now));
        let strip = stream.strip as f64;
        // How far it is from the middle each way
        let off = [0, 1].map(|i| camera[i] - self.world.size[i] as f64 * 0.5);
        let threshold = strip * 0.5 + PAST_M * vpm;

        // Too far out, going faster than strips can be built: moved on all at once
        let edge = (0..2).map(|i| (self.world.size[i] as f64 * 0.5 - off[i].abs()) / strip).fold(f64::INFINITY, f64::min);
        if edge < TELEPORT_STRIPS {
            self.start_teleport(stream);
            return;
        }

        // A strip being built the way the camera's no longer going is called off, unless it's being
        // sent over already, so what's behind is kept for it to go back to
        if let Some(job) = &stream.job
            && job.to_job.is_some()
            && !self.wanted(stream, job.axis, job.forward)
        {
            stream.job = None;
        }

        // Past the edge of the strip in the middle: moved on that way, the furthest first
        let mut order = [0, 1];
        if off[1].abs() > off[0].abs() {
            order = [1, 0];
        }
        for axis in order {
            if off[axis].abs() <= threshold {
                continue;
            }
            let forward = off[axis] > 0.0;
            let wanted = self.incoming(stream.strip, axis, forward);
            // A job not yet sending anything over is called off for it, and one that is goes on,
            // but won't go in where the world's moved on to
            let sending = stream.job.as_ref().is_some_and(|j| j.to_job.is_none());
            if stream.slots[axis].holds == Some(wanted) {
                if !sending {
                    stream.job = None;
                }
                stream.stats.reused += 1;
                self.scroll(stream, axis, forward, None);
                return;
            }
            match &stream.job {
                // Already on its way, or another, which is let finish first
                Some(_) => return,
                None => {
                    self.start_job(stream, axis, forward);
                    return;
                }
            }
        }
        // Not yet: the strip it's heading into is built ahead of time, once it's past the middle
        // going that way or straight away going fast, unless it's already there
        if stream.job.is_some() {
            return;
        }
        let speed = self.camera_drift.xy().norm() / vpm;
        if speed < 0.5 {
            return;
        }
        let axis = if self.camera_drift.x.abs() >= self.camera_drift.y.abs() { 0 } else { 1 };
        let forward = self.camera_drift[axis] > 0.0;
        if (off[axis] > 0.0) != forward && speed < FAST_M_S {
            return;
        }
        let wanted = self.incoming(stream.strip, axis, forward);
        if stream.slots[axis].holds != Some(wanted) {
            self.start_job(stream, axis, forward);
        }
    }

    /// Whether the camera's heading for the strip coming in along `axis` the way `forward` says:
    /// past the strip in the middle that way, or not past it the other way and not going back
    fn wanted(&self, stream: &Stream, axis: usize, forward: bool) -> bool {
        let vpm = self.world.voxels_per_meter as f64;
        let camera = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let off = camera[axis] - self.world.size[axis] as f64 * 0.5;
        let threshold = stream.strip as f64 * 0.5 + PAST_M * vpm;
        let sign = if forward { 1.0 } else { -1.0 };
        off * sign > threshold || (off * sign > -threshold && self.camera_drift[axis] * sign > -0.5 * vpm)
    }

    /// Of `step`, the free camera's next move in voxels, as much as keeps it from flying on faster
    /// than the world can move on: past a strip out from the world's middle, the further out the
    /// less of the way on out, and none of it STOP_AT_STRIPS out, so its edge never comes near
    /// where it'd be seen. Nothing while the world's being built afresh round it, see Teleport.
    pub(super) fn keep_up(&self, step: Vector3<f64>) -> Vector3<f64> {
        let Some(stream) = &self.stream else {
            return step;
        };
        if self.held_still() {
            return Vector3::zeros();
        }
        let camera = camera_to_voxels(self.camera.position, self.world.size);
        let strip = stream.strip as f64;
        let mut step = step;
        for i in 0..2 {
            let off = camera[i] - self.world.size[i] as f64 * 0.5;
            if step[i] * off > 0.0 {
                let out = (off.abs() / strip - SLOW_FROM_STRIPS) / (STOP_AT_STRIPS - SLOW_FROM_STRIPS);
                step[i] *= (1.0 - out).clamp(0.0, 1.0);
            }
        }
        step
    }

    /// Sends `cells` of the world's grid, changed by holes blown and stains splashed in a strip
    /// that's just come in, and their bricks, and the ground water lies on from `ground`'s first
    /// column to its second, over with the frame like the strip was: flush_edits() waits for the
    /// frames there are first, which held up the frame moving on by 10 to 16 ms. None of the frames
    /// still being drawn have the strip, and the bricks they go in are new.
    fn send_edits(&mut self, stream: &mut Stream, mut cells: Vec<usize>, ground: Option<([i64; 2], [i64; 2])>) {
        cells.sort_unstable();
        cells.dedup();
        let host = |words: Vec<u32>| {
            Buffer::from_iter(
                self.memory_allocator.clone(),
                BufferCreateInfo { usage: BufferUsage::TRANSFER_SRC, ..Default::default() },
                AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE, ..Default::default() },
                words,
            )
            .unwrap()
        };
        let mut uploads = Vec::new();
        if !cells.is_empty() {
            let grid: Vec<u32> = cells.iter().map(|&c| self.world.brick_grid[c]).collect();
            let images = cells
                .iter()
                .enumerate()
                .map(|(k, &c)| BufferImageCopy { buffer_offset: (k * 4) as u64, image_offset: self.world.cell_at(c), image_extent: [1, 1, 1], ..Default::default() })
                .collect();
            uploads.push(Upload { words: grid.len(), piece: host(grid), target: Target::Grid, images, buffers: Vec::new(), pooled: false });
            let (mut words, mut buffers) = (Vec::new(), Vec::new());
            for &c in &cells {
                if let Some((index, brick)) = self.world.brick_words(c) {
                    buffers.push(BufferCopy {
                        src_offset: (words.len() * 4) as u64,
                        dst_offset: (index * worldgen::BRICK_WORDS * 4) as u64,
                        size: (worldgen::BRICK_WORDS * 4) as u64,
                        ..Default::default()
                    });
                    words.extend_from_slice(brick);
                }
            }
            if !words.is_empty() {
                uploads.push(Upload { words: words.len(), piece: host(words), target: Target::Bricks, images: Vec::new(), buffers, pooled: false });
            }
        }
        if let Some((lo, hi)) = ground {
            for (at, extent, words) in self.water.take_ground(&self.world, lo, hi) {
                let images = vec![BufferImageCopy { image_offset: [at[0], at[1], 0], image_extent: [extent[0], extent[1], 1], ..Default::default() }];
                uploads.push(Upload { words: words.len(), piece: host(words), target: Target::WaterGround, images, buffers: Vec::new(), pooled: false });
            }
        }
        if uploads.is_empty() {
            return;
        }
        // Room for the bricks blown into being, made with the frame too
        let pages = self.world.page_pool().map_or(0, |p| p.lock().unwrap_or_else(|e| e.into_inner()).count()) as usize;
        self.room_for_pages(pages);
        // Ahead of anything else waiting, so they're in with the frame that first draws the strip
        for upload in uploads.into_iter().rev() {
            stream.uploads.push_front(upload);
        }
    }

    /// Whether the world's being built afresh round the camera behind a fade, see Teleport, so
    /// neither the camera nor the player can go anywhere till it's there
    pub(super) fn held_still(&self) -> bool {
        self.stream.as_ref().and_then(|s| s.teleport.as_ref()).is_some_and(|t| !t.lighter)
    }

    /// Moves the world on a strip along `axis`, the way `forward` says, onto the strip that's just
    /// been put in, `arrived` with where it is, or that was there already. Everything in the world
    /// moves with it, and what's left behind is kept in the slot, in case it's come back to.
    fn scroll(&mut self, stream: &mut Stream, axis: usize, forward: bool, arrived: Option<Arrived>) {
        let started = Instant::now();
        let strip = stream.strip as i64;
        let old = [self.world.origin[0], self.world.origin[1]];
        let w = self.world.size[axis] as i64;
        // What's left behind
        let mut leaving = old;
        leaving[axis] = if forward { old[axis] } else { old[axis] + w - strip };
        let mut leaving_size = [self.world.size[0], self.world.size[1]];
        leaving_size[axis] = strip as u32;
        let mut origin = old;
        origin[axis] += if forward { strip } else { -strip };
        self.world.move_to(origin);
        let mut shift = [0i64; 2];
        shift[axis] = origin[axis] - old[axis];
        self.water.moved(shift, self.world.wrap);
        // What's still to go to the GPU of holes blown is where it was in the room, but where the
        // ground changed is counted from the world's first column
        if let Some((lo, hi)) = &mut self.edited_ground {
            for i in 0..2 {
                lo[i] -= shift[i];
                hi[i] -= shift[i];
            }
        }

        // The slot it moved into is the world now, and what's left behind is in its place; the
        // other way's slot no longer reaches all along the world
        stream.slots[axis] = Slot { holds: Some((leaving, leaving_size)), content: Some((leaving, leaving_size)), free_since: self.frame };
        stream.slots[1 - axis].holds = None;

        let springs = match arrived {
            Some(Arrived { origin: lo, size, features, springs, craters, stains }) => {
                // Its water's surface is worked out to be drawn
                let local = [0, 1].map(|i| (lo[i] - origin[i]).clamp(0, self.world.size[i] as i64) as u32);
                let end = [0, 1].map(|i| (local[i] + size[i]).min(self.world.size[i]));
                self.water.resurface(local, end);
                // Its empty bricks, and those of the world beside it, skip no further than where it
                // joins the world till they're worked out again
                stream.skips.joined(local, end, self.world.size, self.world.wrap);
                stream.pieces.push(features);
                // The holes blown and stains splashed while it was being built, blown in it alone
                // now it's in, as they're in the rest already, and sent over with the frame
                let (lo, hi) = (local.map(i64::from), end.map(i64::from));
                let vpm = self.world.voxels_per_meter as f64;
                let corner = Vector3::new(origin[0] as f64, origin[1] as f64, 0.0);
                let mut cells = Vec::new();
                let mut ground: Option<([i64; 2], [i64; 2])> = None;
                // Only those reaching into it: most are where the fighting is, nowhere near
                let reaches = |at: Vector3<f64>, r: f64| (0..2).all(|i| at[i] + r + 2.0 >= lo[i] as f64 && at[i] - r - 2.0 < hi[i] as f64);
                for k in craters..self.craters.len() {
                    let (center, radius) = self.craters[k];
                    let at = Vector3::from(center) * vpm - corner;
                    if !reaches(at, radius * vpm) {
                        continue;
                    }
                    let (changed, _) = self.world.carve_in(at.into(), radius * vpm, Some((lo, hi)));
                    cells.extend(changed);
                    // Where the ground changed, in the strip
                    let r = radius * vpm + 2.0;
                    let a = [0, 1].map(|i| ((at[i] - r).floor() as i64).max(lo[i]));
                    let b = [0, 1].map(|i| ((at[i] + r).ceil() as i64 + 1).min(hi[i]));
                    if a[0] < b[0] && a[1] < b[1] {
                        ground = Some(ground.map_or((a, b), |(p, q)| ([p[0].min(a[0]), p[1].min(a[1])], [q[0].max(b[0]), q[1].max(b[1])])));
                    }
                }
                for k in stains..self.stains.len() {
                    let (center, radius, material, burn) = self.stains[k];
                    let at = Vector3::from(center) * vpm - corner;
                    if reaches(at, radius * vpm) {
                        cells.extend(self.world.paint_in(at.into(), radius * vpm, material, burn, Some((lo, hi))));
                    }
                }
                self.send_edits(stream, cells, ground);
                springs
            }
            None => {
                let vpm = self.world.voxels_per_meter as f32;
                let lo = [origin[0] as f32 / vpm, origin[1] as f32 / vpm];
                let hi = [lo[0] + self.world.size[0] as f32 / vpm, lo[1] + self.world.size[1] as f32 / vpm];
                worldgen::window_springs(&self.terrain_settings(), &self.rivers, lo, hi)
            }
        };
        stream.river_springs = springs;
        // Only the strips the world reaches into
        let vpm = self.world.voxels_per_meter as f32;
        let (lo, hi) = ([origin[0] as f32 / vpm, origin[1] as f32 / vpm], [(origin[0] + self.world.size[0] as i64) as f32 / vpm, (origin[1] + self.world.size[1] as i64) as f32 / vpm]);
        stream.pieces.retain(|p| p.hi[0] > lo[0] && p.hi[1] > lo[1] && p.lo[0] < hi[0] && p.lo[1] < hi[1]);
        self.water.set_springs(&stream.springs(), origin);
        let watered = started.elapsed();

        self.shift_everything(Vector3::new(shift[0] as f64, shift[1] as f64, 0.0));
        let shifted = started.elapsed();
        let pieces = std::mem::take(&mut stream.pieces);
        self.adopt_pieces(&pieces);
        stream.pieces = pieces;
        let took = started.elapsed().as_secs_f32() * 1000.0;
        stream.stats.frame_ms_max = stream.stats.frame_ms_max.max(took);
        if let Some(capture) = &self.capture {
            let ms = |d: Duration| d.as_secs_f32() * 1000.0;
            println!(
                "Moved on a strip {}{} at frame {} in {took:.1} ms: holes, water and springs {:.1} ms, what's about {:.1} ms, what's in it {:.1} ms",
                if forward { "+" } else { "-" },
                ["x", "y"][axis],
                capture.frame_times.len() + 1,
                ms(watered),
                ms(shifted - watered),
                took - ms(shifted)
            );
        }
    }

    /// Starts building the whole world afresh round where the camera is, as it's got too near the
    /// edge, going faster than strips can be built: what was being built or put in is let be, and
    /// the screen goes dark till it's ready, see update_teleport()
    fn start_teleport(&mut self, stream: &mut Stream) {
        let camera = self.player.as_ref().map_or(camera_to_voxels(self.camera.position, self.world.size), |p| p.position);
        let strip = stream.strip as i64;
        let origin = [0, 1].map(|i| (self.world.origin[i] + camera[i] as i64 - self.world.size[i] as i64 / 2).div_euclid(strip) * strip);
        // Into pages of its own, from the first, as nothing of the world there is now is kept
        let pool = Arc::new(Mutex::new(PagePool::default()));
        let input = self.job_input(stream, origin, [self.world.size[0], self.world.size[1]], origin, pool.clone());
        let (to_main, from_job) = channel();
        let (to_job, from_main) = channel();
        std::thread::Builder::new().name("teleport".into()).spawn(move || run_job(input, to_main, from_main)).unwrap();
        // Whatever was under way is let go: the threads stop once they find no one's listening, and
        // what they'd sent over of it isn't
        stream.job = None;
        stream.installing = None;
        stream.releasing = None;
        stream.skips.forget();
        let dropped: Vec<Subbuffer<[u32]>> = stream.uploads.drain(..).filter(|u| u.pooled).map(|u| u.piece).collect();
        stream.staging.give_back(dropped);
        stream.stats.teleports += 1;
        println!("The world moves on all at once to {origin:?}, too far to keep up a strip at a time");
        let job = Job { axis: 0, forward: true, base: origin, from_job, to_job: Some(to_job), built: None, started: Instant::now(), craters: self.craters.len(), stains: self.stains.len() };
        // Dark at once if the camera's been taken out of the world, as there's nothing to see
        let outside = (0..2).any(|i| camera[i] < 0.0 || camera[i] >= self.world.size[i] as f64);
        stream.teleport = Some(Teleport { job, pool, origin, arrival: None, dark: if outside { 1.0 } else { 0.0 }, lighter: false, last: Instant::now() });
    }

    /// Moving the world on all at once, see start_teleport(): the screen goes dark, the world
    /// built round where the camera is is sent over once nothing can be seen, put in, moved on to,
    /// and the screen comes back
    fn update_teleport(&mut self, stream: &mut Stream) {
        let teleport = stream.teleport.as_mut().unwrap();
        let dt = teleport.last.elapsed().as_secs_f32();
        teleport.last = Instant::now();
        if teleport.lighter {
            teleport.dark -= dt / FADE_SECONDS;
            if teleport.dark <= 0.0 {
                stream.teleport = None;
            }
            return;
        }
        teleport.dark = (teleport.dark + dt / FADE_SECONDS).min(1.0);
        // Listened to till it's all been sent: it's done then, and gone, which isn't its having
        // gone wrong. Taken for that, with pieces of it still to go over, it was all started again,
        // and again, every time.
        if teleport.arrival.is_none() {
            let from_job = std::mem::replace(&mut teleport.job.from_job, channel().1);
            let heard = stream.hear(&from_job);
            let teleport = stream.teleport.as_mut().unwrap();
            teleport.job.from_job = from_job;
            match heard {
                Ok(Some(FromJob::Built { pages })) => teleport.job.built = Some(pages),
                Ok(Some(FromJob::Sent(arrival))) => teleport.arrival = Some(arrival),
                Ok(_) => {}
                Err(()) => {
                    println!("The world couldn't be built afresh");
                    stream.teleport = None;
                    return;
                }
            }
        }
        let teleport = stream.teleport.as_mut().unwrap();
        // Sent over once it's built and nothing can be seen
        if teleport.dark >= 1.0
            && teleport.job.built.is_some()
            && let Some(to_job) = teleport.job.to_job.take()
        {
            let pages = teleport.pool.lock().unwrap_or_else(|e| e.into_inner()).count() as usize;
            self.room_for_pages(pages);
            let _ = to_job.send(());
        }
        // Moved on to once it's all there
        let teleport = stream.teleport.as_mut().unwrap();
        if teleport.arrival.is_none() || !stream.uploads.is_empty() {
            return;
        }
        let Arrival { strip, features, springs, .. } = *teleport.arrival.take().unwrap();
        let origin = teleport.origin;
        let pool = teleport.pool.clone();
        let started = teleport.job.started;
        teleport.lighter = true;
        // A world of its own, all of it put in at once while nothing can be seen
        let mut world = World::window(self.world.size, stream.strip, origin, self.world.voxels_per_meter, self.world.sea_level);
        if let Some(pages) = &mut world.pages {
            pages.pool = pool;
        }
        let mut installing = World::install(strip);
        while !world.install_step(&mut installing, u32::MAX) {}
        world.forget_water_surface();
        let strip = installing.strip;
        let old = [self.world.origin[0], self.world.origin[1]];
        let old_world = std::mem::replace(&mut self.world, world);
        std::thread::spawn(move || drop(old_world));
        // What was still to go over of holes blown was of the world there was, and they're all in
        // this one already
        self.edited.clear();
        self.edited_ground = None;
        let shift = [origin[0] - old[0], origin[1] - old[1]];
        self.water.moved(shift, self.world.wrap);
        self.water.take_strip(strip.origin, strip.size, &strip.ground, &strip.surface);
        self.water.resurface([0, 0], [self.world.size[0], self.world.size[1]]);
        stream.slots = Default::default();
        stream.quarantine.clear();
        stream.skips.forget();
        stream.pieces = vec![features];
        stream.river_springs = springs;
        self.water.set_springs(&stream.springs(), origin);
        self.shift_everything(Vector3::new(shift[0] as f64, shift[1] as f64, 0.0));
        // Up out of anything solid there, as wherever they were taken, the world wasn't there yet
        // to stand them on
        if let Some(player) = &self.player {
            let lifted = Player::new(player.eye(&self.world), &self.world);
            if lifted.position.z > player.position.z + 0.5 {
                self.camera.position = voxels_to_camera(lifted.eye(&self.world), self.world.size);
                self.player = Some(lifted);
            }
        }
        let pieces = std::mem::take(&mut stream.pieces);
        self.adopt_pieces(&pieces);
        stream.pieces = pieces;
        self.reset_accum = true;
        self.light_cache_for = None;
        stream.stats.total_ms.push(started.elapsed().as_secs_f32() * 1000.0);
        std::thread::spawn(move || drop(strip));
    }

    /// Takes the lights, props, buildings, camps and dungeons of the strips the world's made of,
    /// those in the world, in its voxels, see adopt_features()
    pub(super) fn adopt_pieces(&mut self, pieces: &[PieceFeatures]) {
        let vpm = self.world.voxels_per_meter as f32;
        let (lo, hi) = ([self.world.origin[0] as f32 / vpm, self.world.origin[1] as f32 / vpm], [(self.world.origin[0] + self.world.size[0] as i64) as f32 / vpm, (self.world.origin[1] + self.world.size[1] as i64) as f32 / vpm]);
        let inside = |p: &[f32]| p[0] >= lo[0] && p[1] >= lo[1] && p[0] < hi[0] && p[1] < hi[1];
        let mut all = PieceFeatures::default();
        for piece in pieces {
            all.props.extend(piece.props.iter().filter(|p| inside(&p.position)));
            all.lights.extend(piece.lights.iter().filter(|l| inside(&l.position)));
            all.sites.extend(piece.sites.iter().filter(|s| inside(&s.position)));
            all.camps.extend(piece.camps.iter().filter(|c| inside(&c.fire)));
            all.waterfalls.extend(piece.waterfalls.iter().filter(|w| inside(&w.0)));
        }
        self.adopt_features(&all);
    }

    /// The settings the landscape's being built with now
    pub(super) fn terrain_settings(&self) -> TerrainSettings {
        TerrainSettings {
            width_m: self.world.size[0] / self.world.voxels_per_meter,
            height_m: self.world.size[2] / self.world.voxels_per_meter,
            voxels_per_meter: self.world.voxels_per_meter,
            ..self.terrain
        }
    }

    /// The world's corner moved on `shift` voxels over the landscape: everything in it moves with it,
    /// as it's counted from there, and what's now outside it goes
    fn shift_everything(&mut self, shift: Vector3<f64>) {
        let camera = camera_to_voxels(self.camera.position, self.world.size) - shift;
        self.camera.position = voxels_to_camera(camera, self.world.size);
        if let Some(player) = &mut self.player {
            player.position -= shift;
        }
        if let Some((at, when)) = self.last_camera {
            self.last_camera = Some((at - shift, when));
        }
        self.last_feet = self.last_feet.map(|f| f - shift);
        self.wake = self.wake.map(|(p, v)| ([p[0] - shift.x, p[1] - shift.y], v));
        if let Some(near) = &mut self.near {
            near.origin -= shift;
            near.center -= shift;
        }
        if let Some(build) = &mut self.near_build {
            build.center -= shift;
        }
        if let Some((at, when)) = self.play.check {
            self.play.check = Some((at - shift, when));
        }
        // What was drawn last frame is still where it was, so the frames gathered go on
        let shift32 = shift.cast::<f32>();
        if let Some(last) = &mut self.last_view {
            let mut moved = last.pixel_to_ray;
            moved.m14 -= shift32.x;
            moved.m24 -= shift32.y;
            last.pixel_to_ray = moved;
        }
        if let Some(previous) = &mut self.last_world_to_window {
            *previous *= Matrix4::new_translation(&shift);
        }
        // The map of where sunlight reaches is of the world where it was
        self.sun_map_for = None;
        let water = &self.water;
        let near = self.player.as_ref().map_or(camera, |p| p.position);
        self.game.recenter(&self.world, &|x, y| water.surface_at(x, y), shift, self.terrain.seed, near);
    }
}
