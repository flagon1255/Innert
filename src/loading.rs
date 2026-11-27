//! The loading screen. The window's up with it as soon as Innert starts, while what the game needs
//! is made on other threads, see Loaded::load(). Once it's all there the game takes over the same
//! window, with what's drawn to it, the swapchain and the GUI, and the loading screen fades away
//! over the menu.
//!
//! It's black and white as the rest of the interface, see theme.rs: the name in blackletter, and
//! under it the very land the player will stand on rising as the world's built, see land.rs, then
//! the bar, what's being made and a tip. The same screen's shown over the game while the world's
//! built afresh round where it goes on, a game started or a realm gone to, see Board.

use std::f32::consts::TAU;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::rc::Rc;
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::{Duration, Instant};

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, LayerId, Mesh, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};
use egui_winit_vulkano::{Gui, GuiConfig};
use std::ops::Range;

use vulkano::{
    DeviceSize, Validated, VulkanError,
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    command_buffer::{AutoCommandBufferBuilder, ClearColorImageInfo, CommandBufferUsage, CopyImageToBufferInfo},
    device::{Device, DeviceOwned, Queue},
    image::{
        Image, ImageLayout,
        view::{ImageView, ImageViewCreateInfo},
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    swapchain::{Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo, acquire_next_image},
    sync::{
        self, GpuFuture,
        future::{AccessCheckError, SemaphoreSignalFuture, SubmitAnyBuilder},
    },
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

use crate::worldgen::{self, TerrainSettings};
use crate::{App, Capture, Gpu, Loaded, get_swapchain_images, save_screen, settings_from_args, startup, window_attributes};

mod land;

use land::{Land, Phase};

/// The loading screen's drawn this often
const FRAME_TIME: Duration = Duration::from_micros(16_667);
/// How long the loading screen takes to fade in over the black the window's filled with
const FADE_IN_SECONDS: f32 = 0.35;
/// How long the loading screen takes to fade away over the menu
const FADE_SECONDS: f32 = 0.8;

/// What's made while loading, in the order it's shown
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    Graphics,
    Shaders,
    Ground,
    Rivers,
    Land,
    Voxels,
    Grass,
    Upload,
    Water,
    Creatures,
}

const STEPS: usize = 10;

/// The steps of building the world, in the order generate_terrain() goes through them
const WORLD: [Step; 5] = [Step::Ground, Step::Rivers, Step::Land, Step::Voxels, Step::Grass];

impl Step {
    const ALL: [Step; STEPS] = [
        Step::Graphics,
        Step::Shaders,
        Step::Ground,
        Step::Rivers,
        Step::Land,
        Step::Voxels,
        Step::Grass,
        Step::Upload,
        Step::Water,
        Step::Creatures,
    ];

    fn label(self) -> &'static str {
        match self {
            Step::Graphics => "Starting the graphics card",
            Step::Shaders => "Preparing the shaders",
            Step::Ground => "Raising the land",
            Step::Rivers => "Carving the rivers",
            Step::Land => "Founding towns and castles",
            Step::Voxels => "Laying the land voxel by voxel",
            Step::Grass => "Growing the grass and the forests",
            Step::Upload => "Sending the world to the graphics card",
            Step::Water => "Filling the seas",
            Step::Creatures => "Waking the beasts",
        }
    }

    /// About how long it takes in milliseconds, as on the laptop with an RTX 4050 it was measured
    /// on. Only how long each takes next to the others matters, for how far along the bar is.
    fn expected(self) -> f32 {
        match self {
            Step::Graphics => 200.0,
            Step::Shaders => 100.0,
            Step::Ground => 360.0,
            Step::Rivers => 85.0,
            Step::Land => 210.0,
            Step::Voxels => 3700.0,
            Step::Grass => 650.0,
            Step::Upload => 300.0,
            Step::Water => 60.0,
            Step::Creatures => 400.0,
        }
    }

    /// The graphics card and the world are made side by side, then the rest one at a time
    fn track(self) -> usize {
        match self {
            Step::Graphics | Step::Shaders => 0,
            Step::Ground | Step::Rivers | Step::Land | Step::Voxels | Step::Grass => 1,
            Step::Upload | Step::Water | Step::Creatures => 2,
        }
    }
}

/// How long the shaders take when they're compiled, rather than kept from before, see
/// hot_reload::is_cached()
const COMPILING_SHADERS_MS: u32 = 15_000;

/// How far loading's got, shared by the threads making the game and the loading screen
pub struct Progress {
    /// When each step started and finished
    times: Mutex<[(Option<Instant>, Option<Instant>); STEPS]>,
    /// About how long each step should take, in milliseconds, see Step::expected()
    expected: [f32; STEPS],
    /// How long the shaders should take
    shaders_ms: AtomicU32,
    /// Set once the window's closed while loading, to stop where it can
    cancelled: AtomicBool,
    /// The land round where the player's to stand, to show it being built, once it's been read
    /// off the landscape, see show_land()
    land: Mutex<Option<Arc<Land>>>,
    /// Whether it's the game starting, whose steps are timed, see startup.rs
    starting: bool,
}

impl Progress {
    fn new() -> Self {
        Progress {
            times: Mutex::new([(None, None); STEPS]),
            expected: Step::ALL.map(Step::expected),
            shaders_ms: AtomicU32::new(Step::Shaders.expected() as u32),
            cancelled: AtomicBool::new(false),
            land: Mutex::new(None),
            starting: true,
        }
    }

    /// For the world being built afresh round where a game goes on, see stream.rs: the graphics
    /// card and the creatures are there already, and what's built goes over to it with the frames
    pub fn arriving() -> Arc<Self> {
        let mut progress = Progress { starting: false, ..Progress::new() };
        progress.expected[Step::Upload as usize] = 600.0;
        progress.expected[Step::Water as usize] = 150.0;
        let now = Instant::now();
        for step in [Step::Graphics, Step::Shaders, Step::Creatures] {
            progress.expected[step as usize] = 0.0;
            progress.times()[step as usize] = (Some(now), Some(now));
        }
        progress.shaders_ms.store(0, Ordering::Relaxed);
        Arc::new(progress)
    }

    fn times(&self) -> std::sync::MutexGuard<'_, [(Option<Instant>, Option<Instant>); STEPS]> {
        self.times.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn begin(&self, step: Step) {
        self.times()[step as usize].0.get_or_insert_with(Instant::now);
    }

    pub fn end(&self, step: Step) {
        let mut times = self.times();
        let (started, done) = &mut times[step as usize];
        if done.is_none() {
            let now = Instant::now();
            *done = Some(now);
            let took = now - *started.get_or_insert(now);
            if self.starting {
                startup::finished(step.label(), took);
            }
        }
    }

    /// What `make` makes, counted as `step`
    pub fn run<T>(&self, step: Step, make: impl FnOnce() -> T) -> T {
        self.begin(step);
        let made = make();
        self.end(step);
        made
    }

    /// The shaders have to be compiled this time, which takes far longer
    pub fn compiling_shaders(&self) {
        self.shaders_ms.store(COMPILING_SHADERS_MS, Ordering::Relaxed);
    }

    fn shaders_compiling(&self) -> bool {
        self.shaders_ms.load(Ordering::Relaxed) == COMPILING_SHADERS_MS
    }

    /// What `build` builds, with each step generate_terrain() goes through on the way counted, see
    /// worldgen::on_step()
    pub fn building_world<T>(self: &Arc<Self>, build: impl FnOnce() -> T) -> T {
        self.watch_world();
        let built = build();
        self.world_built();
        built
    }

    /// Has each step generate_terrain() goes through on this thread from here on counted, until
    /// world_built()
    pub fn watch_world(self: &Arc<Self>) {
        let progress = self.clone();
        worldgen::on_step(Some(Box::new(move |n| {
            if let Some(&before) = n.checked_sub(1).and_then(|n| WORLD.get(n)) {
                progress.end(before);
            }
            if let Some(&step) = WORLD.get(n) {
                progress.begin(step);
            }
        })));
    }

    /// The world's built, see watch_world(), with all it takes besides on this thread
    pub fn world_built(&self) {
        worldgen::on_step(None);
        for step in WORLD {
            self.end(step);
        }
    }

    /// Reads the land round `middle`, in meters from the corner of the whole landscape, off the
    /// landscape `settings` build, on a thread of its own, to show it being built
    pub fn show_land(self: &Arc<Self>, settings: TerrainSettings, middle: [f32; 2]) {
        let progress = self.clone();
        let _ = std::thread::Builder::new().name("land shown".into()).spawn(move || {
            let land = Land::around(&settings, middle);
            *progress.land.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(land));
        });
    }

    fn land(&self) -> Option<Arc<Land>> {
        self.land.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    fn expected(&self, step: Step) -> f32 {
        if step == Step::Shaders { self.shaders_ms.load(Ordering::Relaxed) as f32 } else { self.expected[step as usize] }
    }

    /// About how many milliseconds `step` has still to go, at `times`: what's expected less what's
    /// gone, then less and less of what's left the longer it takes past that, so it always seems
    /// to be getting on
    fn left(&self, times: &[(Option<Instant>, Option<Instant>); STEPS], step: Step) -> f32 {
        let expected = self.expected(step);
        match times[step as usize] {
            (_, Some(_)) => 0.0,
            (Some(started), None) => expected * still_to_go(started.elapsed().as_secs_f32() * 1000.0 / expected.max(1.0)),
            (None, None) => expected,
        }
    }

    /// How far along it all is, from 0 to 1, by how long what's left should take, and what's being
    /// made meanwhile
    fn estimate(&self) -> (f32, Step) {
        let times = self.times();
        let (mut total, mut left, mut current) = ([0.0f32; 3], [0.0f32; 3], [None; 3]);
        for step in Step::ALL {
            let track = step.track();
            total[track] += self.expected(step);
            left[track] += self.left(&times, step);
            if times[step as usize].1.is_none() && current[track].is_none() {
                current[track] = Some(step);
            }
        }
        let fraction = 1.0 - (left[0].max(left[1]) + left[2]) / (total[0].max(total[1]) + total[2]).max(1.0);
        // Whichever of the graphics card and the world has longer to go, then the rest in turn
        let step = match current {
            [None, None, rest] => rest,
            [card, world, _] if left[0] > left[1] => card.or(world),
            [card, world, _] => world.or(card),
        };
        (fraction.clamp(0.0, 1.0), step.unwrap_or(Step::Creatures))
    }

    /// How far the land's built, see land.rs: risen as the ground, the rivers, what's on it and
    /// its voxels are made, its woods grown with the grass, and the seas filling once that's done
    fn phase(&self) -> Phase {
        let times = self.times();
        let rising = [Step::Ground, Step::Rivers, Step::Land, Step::Voxels];
        let total: f32 = rising.iter().map(|&s| self.expected(s)).sum();
        let left: f32 = rising.iter().map(|&s| self.left(&times, s)).sum();
        Phase {
            rise: 1.0 - left / total.max(1.0),
            grow: 1.0 - self.left(&times, Step::Grass) / self.expected(Step::Grass).max(1.0),
            fill: if times[Step::Grass as usize].1.is_some() { 1.0 } else { 0.0 },
        }
    }
}

/// How much of a step's expected time it has still to go, `x` times its expected time in: in step
/// with the time till then, then ever less of the last part, never quite none
fn still_to_go(x: f32) -> f32 {
    const LAST: f32 = 0.15;
    if x < 1.0 { 1.0 - (1.0 - LAST) * x.max(0.0) } else { LAST * (-(x - 1.0) * 1.5).exp() }
}

/// `--set loading_preview=1` in screenshot runs: the loading screen's drawn as it is in play,
/// though the window's never shown, and shots of it are saved next to the screenshot. The menu's
/// up after, as in play, to see it fade away over it with `--shots` and `--set gui=1`.
pub fn previewing() -> bool {
    Capture::from_args().is_some() && settings_from_args().iter().any(|(k, v)| k == "loading_preview" && (v == "1" || v == "true"))
}

/// The window while it's the loading screen's, to put away should starting fail, see put_away()
static WINDOW: Mutex<Option<Weak<Window>>> = Mutex::new(None);

/// Hides the loading screen's window, from any thread, so a box saying why the game can't start
/// is all there is on the screen, see fail()
pub fn put_away() {
    let window = WINDOW.lock().ok().and_then(|w| w.as_ref()?.upgrade());
    if let Some(window) = window {
        window.set_visible(false);
    }
}

/// The window and what's drawn to it, made for the loading screen and handed over to the game, see
/// App::attach()
pub struct Screen {
    pub window: Arc<Window>,
    pub swapchain: Arc<Swapchain>,
    pub image_views: Vec<Arc<ImageView>>,
    pub gui: Gui,
    pub previous_frame_end: Option<Box<dyn GpuFuture>>,
    pub recreate_swapchain: bool,
    pub presented: Presented,
}

/// The semaphore each of the window's images was last shown once it said it was drawn, see Drawn,
/// kept till that image comes round again: showing it may wait on the semaphore till then, so it
/// mustn't go back to vulkano's pool for another frame to signal before, see
/// https://docs.vulkan.org/guide/latest/swapchain_semaphore_reuse.html
#[derive(Default)]
pub struct Presented(Vec<Option<Signal>>);

impl Presented {
    /// `image` has come round again: what it was last shown with can go
    pub fn acquired(&mut self, image: u32) {
        if let Some(slot) = self.0.get_mut(image as usize) {
            *slot = None;
        }
    }

    /// `image` is being shown once `drawn` is
    pub fn showing(&mut self, image: u32, drawn: Signal) {
        let i = image as usize;
        if self.0.len() <= i {
            self.0.resize_with(i + 1, || None);
        }
        self.0[i] = Some(drawn);
    }
}

/// A frame, then a semaphore signaled once it's drawn, which showing it waits on. Without, vulkano
/// shows it straight after it's sent, which Vulkan doesn't promise waits for it to be drawn, and
/// which the validation layers' synchronization checks call a hazard
/// (SYNC-HAZARD-PRESENT-AFTER-WRITE). Shared, so the semaphore can be kept, see Presented: just as
/// vulkano's own otherwise.
pub struct Drawn(Signal);

/// A frame's semaphore that it's drawn, as kept, see Drawn
pub type Signal = Rc<SemaphoreSignalFuture<Box<dyn GpuFuture>>>;

impl Drawn {
    pub fn after(frame: Box<dyn GpuFuture>) -> Self {
        Drawn(Rc::new(frame.then_signal_semaphore()))
    }

    /// To keep, see Presented
    pub fn semaphore(&self) -> Signal {
        self.0.clone()
    }
}

unsafe impl DeviceOwned for Drawn {
    fn device(&self) -> &Arc<Device> {
        self.0.device()
    }
}

// SAFETY: all just as the semaphore's future, but for cleaning up after it while it's kept
unsafe impl GpuFuture for Drawn {
    fn cleanup_finished(&mut self) {
        if let Some(future) = Rc::get_mut(&mut self.0) {
            future.cleanup_finished();
        }
    }

    unsafe fn build_submission(&self) -> Result<SubmitAnyBuilder, Validated<VulkanError>> {
        unsafe { self.0.build_submission() }
    }

    fn flush(&self) -> Result<(), Validated<VulkanError>> {
        self.0.flush()
    }

    unsafe fn signal_finished(&self) {
        unsafe { self.0.signal_finished() }
    }

    fn queue_change_allowed(&self) -> bool {
        self.0.queue_change_allowed()
    }

    fn queue(&self) -> Option<Arc<Queue>> {
        self.0.queue()
    }

    fn check_buffer_access(&self, buffer: &Buffer, range: Range<DeviceSize>, exclusive: bool, queue: &Queue) -> Result<(), AccessCheckError> {
        self.0.check_buffer_access(buffer, range, exclusive, queue)
    }

    fn check_image_access(&self, image: &Image, range: Range<DeviceSize>, exclusive: bool, expected_layout: ImageLayout, queue: &Queue) -> Result<(), AccessCheckError> {
        self.0.check_image_access(image, range, exclusive, expected_layout, queue)
    }

    fn check_swapchain_image_acquired(&self, swapchain: &Swapchain, image_index: u32, before: bool) -> Result<(), AccessCheckError> {
        self.0.check_swapchain_image_acquired(swapchain, image_index, before)
    }
}

/// What the loader thread tells the loading screen
enum Message {
    /// The graphics card's going, so the loading screen can be drawn
    Started(Gpu),
    Loaded(Box<Loaded>),
}

/// Runs the event loop: the loading screen until the game's loaded, then the game
pub struct Launcher {
    progress: Arc<Progress>,
    messages: mpsc::Receiver<Message>,
    loader: Option<std::thread::JoinHandle<()>>,
    state: State,
    /// Whether the game's first frame has been timed, see startup.rs
    timed: bool,
}

enum State {
    Loading(Box<Loading>),
    Playing(Box<App>),
    /// The window was closed while loading, or loading went wrong, which has been said
    Stopped { failed: bool },
}

impl Launcher {
    /// Starts loading the game on another thread, while the event loop gets the window up
    pub fn start(event_loop: &EventLoop<()>) -> Self {
        let progress = Arc::new(Progress::new());
        let (sender, messages) = mpsc::channel();
        let wake = event_loop.create_proxy();
        let display = event_loop.owned_display_handle();
        let loader = {
            let progress = progress.clone();
            std::thread::Builder::new()
                .name("loader".into())
                .spawn(move || {
                    // Threads it starts, like the world's, start level with everything else
                    hurry(true);
                    let tell = |message| {
                        let _ = sender.send(message);
                        let _ = wake.send_event(());
                    };
                    if let Some(loaded) = Loaded::load(&display, &progress, |gpu| tell(Message::Started(gpu.clone()))) {
                        tell(Message::Loaded(Box::new(loaded)));
                    }
                    // Woken either way, to hear it's stopped
                    let _ = wake.send_event(());
                })
                .unwrap()
        };
        // Until the game takes over, see take_over()
        hurry(true);
        let out_of_sight = Capture::from_args().is_some();
        let preview = previewing().then(|| Preview::new(Capture::from_args().unwrap().path));
        let now = Instant::now();
        let loading = Loading {
            out_of_sight,
            drawn: !out_of_sight || preview.is_some(),
            window: None,
            drawn_once: false,
            gpu: None,
            screen: None,
            look: Look::new(),
            next_frame: now,
            preview,
            loaded: None,
            cost: Cost::default(),
        };
        Launcher { progress, messages, loader: Some(loader), state: State::Loading(Box::new(loading)), timed: false }
    }

    /// Once the event loop's done. False if loading went wrong, which has been said.
    pub fn finish(&mut self) -> bool {
        if self.progress.cancelled() {
            // Given a moment to stop where it can, rather than the process ending with it partway
            // through sending something to the graphics card
            let waiting = Instant::now();
            while self.loader.as_ref().is_some_and(|l| !l.is_finished()) && waiting.elapsed() < Duration::from_secs(3) {
                std::thread::sleep(Duration::from_millis(10));
            }
        } else if let Some(loader) = self.loader.take() {
            let _ = loader.join();
        }
        !matches!(self.state, State::Stopped { failed: true })
    }

    /// Takes what the loader thread's sent
    fn hear(&mut self, event_loop: &ActiveEventLoop) {
        let State::Loading(loading) = &mut self.state else {
            return;
        };
        let mut failed = false;
        loop {
            match self.messages.try_recv() {
                Ok(Message::Started(gpu)) => {
                    startup::moment("Graphics card ready");
                    loading.gpu = Some(gpu);
                    loading.connect(event_loop, &self.progress);
                }
                Ok(Message::Loaded(loaded)) => {
                    loading.loaded = Some(loaded);
                    // Drawn once more as the game's put together, which holds up this thread a
                    // moment, see take_over()
                    self.progress.begin(Step::Creatures);
                    loading.connect(event_loop, &self.progress);
                    if loading.drawn {
                        loading.draw(&self.progress, true);
                    }
                    event_loop.set_control_flow(ControlFlow::Poll);
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    // It stopped without the game, having said why
                    failed = loading.loaded.is_none() && !self.progress.cancelled();
                    break;
                }
            }
        }
        if failed {
            self.state = State::Stopped { failed: true };
            event_loop.exit();
        }
    }

    /// The game takes over the window, at the start of a turn of the event loop so it hears all
    /// of what happens in it
    fn take_over(&mut self, event_loop: &ActiveEventLoop) {
        let State::Loading(loading) = std::mem::replace(&mut self.state, State::Stopped { failed: true }) else {
            return;
        };
        // The window and the graphics card are there by the time the game's loaded
        let Loading { loaded: Some(loaded), screen: Some(screen), look, drawn, cost, out_of_sight, .. } = *loading else {
            event_loop.exit();
            return;
        };
        if out_of_sight {
            cost.say();
        }
        let mut app = Box::new(App::new(*loaded));
        self.progress.end(Step::Creatures);
        // The game's from here on
        *WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = None;
        // The game runs as it always has
        hurry(false);
        if drawn {
            fade_away(&screen.gui.context(), look, self.progress.land());
        }
        app.attach(screen, event_loop);
        self.state = State::Playing(app);
    }

    /// The window was closed while loading
    fn stop(&mut self, event_loop: &ActiveEventLoop) {
        if let State::Loading(loading) = &self.state
            && let Some(window) = &loading.window
        {
            window.set_visible(false);
        }
        self.progress.cancel();
        self.state = State::Stopped { failed: false };
        event_loop.exit();
    }

    fn time_first_frame(&mut self) {
        if let State::Playing(app) = &self.state
            && !self.timed
            && app.frame > 0
        {
            self.timed = true;
            startup::moment("First frame of the game drawn");
        }
    }
}

impl ApplicationHandler for Launcher {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match &mut self.state {
            State::Loading(loading) => loading.open(event_loop, &self.progress),
            State::Playing(app) => app.resumed(event_loop),
            State::Stopped { .. } => {}
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(&self.state, State::Loading(loading) if loading.loaded.is_some()) {
            self.take_over(event_loop);
        }
        if let State::Playing(app) = &mut self.state {
            app.new_events(event_loop, cause);
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, _: ()) {
        self.hear(event_loop);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        match &mut self.state {
            State::Playing(app) => {
                app.window_event(event_loop, window_id, event);
                self.time_first_frame();
            }
            State::Loading(_) if event == WindowEvent::CloseRequested => self.stop(event_loop),
            State::Loading(loading) => loading.window_event(&event, &self.progress),
            State::Stopped { .. } => {}
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
        if let State::Playing(app) = &mut self.state {
            app.device_event(event_loop, device_id, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match &mut self.state {
            State::Playing(app) => {
                app.about_to_wait(event_loop);
                self.time_first_frame();
            }
            State::Loading(loading) => loading.about_to_wait(event_loop, &self.progress),
            State::Stopped { .. } => {}
        }
    }
}

/// While the game's loading
struct Loading {
    /// Screenshot runs' window is never shown, see window_attributes()
    out_of_sight: bool,
    /// Whether the loading screen's drawn: not in screenshot runs, unless it's previewed
    drawn: bool,
    window: Option<Arc<Window>>,
    drawn_once: bool,
    gpu: Option<Gpu>,
    /// Once the graphics card's going
    screen: Option<Screen>,
    look: Look,
    next_frame: Instant,
    preview: Option<Preview>,
    /// Handed to the game at the start of the next turn of the event loop, see Launcher::take_over()
    loaded: Option<Box<Loaded>>,
    /// How long drawing it takes, said in screenshot runs
    cost: Cost,
}

/// How long the loading screen's taken to draw, with the land on it, over how many frames: painting
/// it with the GUI, and that and readying it for the graphics card
#[derive(Default)]
struct Cost {
    frames: u32,
    painting: Duration,
    drawing: Duration,
}

impl Cost {
    fn add(&mut self, painting: Duration, drawing: Duration) {
        self.frames += 1;
        self.painting += painting;
        self.drawing += drawing;
    }

    fn say(&self) {
        if self.frames > 0 {
            let each = |d: Duration| d.as_secs_f64() * 1000.0 / self.frames as f64;
            println!("Loading screen drawn {} times with the land, {:.3} ms painting it and {:.3} ms in all a frame", self.frames, each(self.painting), each(self.drawing));
        }
    }
}

impl Loading {
    /// Makes the window and shows it straight away, filled with the loading screen's black until the
    /// graphics card's going and the loading screen can be drawn in it
    fn open(&mut self, event_loop: &ActiveEventLoop, progress: &Progress) {
        if self.window.is_some() {
            return;
        }
        let window = Arc::new(event_loop.create_window(window_attributes(self.out_of_sight)).unwrap());
        *WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::downgrade(&window));
        if !self.out_of_sight {
            window.set_visible(true);
            fill(&window);
            startup::moment("Window shown");
        } else {
            startup::moment("Window made");
        }
        self.window = Some(window);
        self.connect(event_loop, progress);
    }

    /// Readies the window to be drawn to, once it's there and the graphics card's going
    fn connect(&mut self, event_loop: &ActiveEventLoop, progress: &Progress) {
        let (Some(window), Some(gpu), None) = (&self.window, &self.gpu, &self.screen) else {
            return;
        };
        let surface = Surface::from_window(gpu.instance.clone(), window.clone()).unwrap();
        let (swapchain, images) = get_swapchain_images(&gpu.device, &surface, window);
        let image_views = images
            .iter()
            .map(|i| ImageView::new(i.clone(), ImageViewCreateInfo::from_image(i)).unwrap())
            .collect::<Vec<_>>();
        let gui = Gui::new(
            event_loop,
            surface,
            gpu.queue.clone(),
            swapchain.image_format(),
            GuiConfig {
                is_overlay: true,
                ..Default::default()
            },
        );
        // Its look, the same in every window, see theme.rs
        crate::theme::apply(&gui.context());
        self.screen = Some(Screen {
            window: window.clone(),
            swapchain,
            image_views,
            gui,
            previous_frame_end: Some(sync::now(gpu.device.clone()).boxed()),
            recreate_swapchain: false,
            presented: Presented::default(),
        });
        if self.drawn {
            // Everything moving in it starts from now, fading in over the black
            let now = Instant::now();
            (self.look.start, self.look.last) = (now, now);
            self.draw(progress, false);
        }
    }

    fn window_event(&mut self, event: &WindowEvent, progress: &Progress) {
        if let Some(screen) = &mut self.screen {
            screen.gui.update(event);
            if matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }) {
                screen.recreate_swapchain = true;
            }
        }
        if *event == WindowEvent::RedrawRequested && self.loaded.is_none() {
            match (&self.screen, &self.window) {
                (Some(_), _) if self.drawn => self.draw(progress, false),
                // Resized, say, before the graphics card's going
                (None, Some(window)) if !self.out_of_sight => fill(window),
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop, progress: &Progress) {
        // Waiting for the game to take over, see Launcher::hear()
        if self.loaded.is_some() {
            return;
        }
        if self.screen.is_none() || !self.drawn {
            // Until the graphics card's going, or for good in screenshot runs
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        if now >= self.next_frame {
            self.next_frame = (self.next_frame + FRAME_TIME).max(now);
            // A window that's never shown is never asked to redraw
            if self.out_of_sight {
                self.draw(progress, false);
            } else if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
    }
    /// Draws the loading screen, as far as loading's got. The `last` time, before the game takes
    /// over.
    fn draw(&mut self, progress: &Progress, last: bool) {
        let (Some(screen), Some(gpu)) = (&mut self.screen, &self.gpu) else {
            return;
        };
        let size = screen.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return;
        }
        screen.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if screen.recreate_swapchain {
            let images;
            (screen.swapchain, images) = screen
                .swapchain
                .recreate(SwapchainCreateInfo {
                    image_extent: size.into(),
                    ..screen.swapchain.create_info()
                })
                .unwrap();
            screen.image_views = images
                .iter()
                .map(|i| ImageView::new(i.clone(), ImageViewCreateInfo::from_image(i)).unwrap())
                .collect();
            screen.recreate_swapchain = false;
        }

        let (image_index, suboptimal, acquired) = match acquire_next_image(screen.swapchain.clone(), None).map_err(Validated::unwrap) {
            Ok(r) => r,
            Err(VulkanError::OutOfDate) => {
                screen.recreate_swapchain = true;
                return;
            }
            Err(e) => panic!("failed to acquire next image: {e}"),
        };
        screen.presented.acquired(image_index);
        if suboptimal {
            screen.recreate_swapchain = true;
        }

        let (fraction, step) = progress.estimate();
        self.look.advance(fraction, step, progress.shaders_compiling(), progress.phase());
        let look = self.look.clone();
        let appearing = smoothstep(look.start.elapsed().as_secs_f32() / FADE_IN_SECONDS);
        let land = progress.land();
        let words = Words { title: "Innert", subtitle: None };
        let painting = Instant::now();
        screen.gui.immediate_ui(|gui| paint(&gui.context(), &look, appearing, LayerId::background(), None, land.as_deref(), &words));
        let painted = painting.elapsed();

        let view = screen.image_views[image_index as usize].clone();
        let image = view.image().clone();
        // The GUI's drawn over what's there
        let mut builder = AutoCommandBufferBuilder::primary(gpu.command_buffer_allocator.clone(), gpu.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
        builder
            .clear_color_image(ClearColorImageInfo {
                clear_value: [INK[0] as f32 / 255.0, INK[1] as f32 / 255.0, INK[2] as f32 / 255.0, 1.0].into(),
                ..ClearColorImageInfo::image(image.clone())
            })
            .unwrap();
        let cleared = screen.previous_frame_end.take().unwrap().join(acquired).then_execute(gpu.queue.clone(), builder.build().unwrap()).unwrap();
        let drawn = screen.gui.draw_on_image(cleared, view);
        if land.is_some() {
            self.cost.add(painted, painting.elapsed());
        }

        // Copied off for a shot of it, when previewed
        let shot = self.preview.as_mut().and_then(|p| p.next(look.fraction, appearing >= 1.0, last)).map(|path| {
            let buffer = Buffer::new_slice::<u8>(
                gpu.memory_allocator.clone(),
                BufferCreateInfo { usage: BufferUsage::TRANSFER_DST, ..Default::default() },
                AllocationCreateInfo { memory_type_filter: MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_RANDOM_ACCESS, ..Default::default() },
                image.extent()[0] as u64 * image.extent()[1] as u64 * 4,
            )
            .unwrap();
            (path, buffer)
        });
        let drawn = match &shot {
            Some((_, buffer)) => {
                let mut copy = AutoCommandBufferBuilder::primary(gpu.command_buffer_allocator.clone(), gpu.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
                copy.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(image.clone(), buffer.clone())).unwrap();
                drawn.then_execute(gpu.queue.clone(), copy.build().unwrap()).unwrap().boxed()
            }
            None => drawn,
        };

        // Shown once it's drawn, see Drawn
        let drawn = Drawn::after(drawn);
        let semaphore = drawn.semaphore();
        let frame_end = drawn
            .boxed()
            .then_swapchain_present(gpu.queue.clone(), SwapchainPresentInfo::swapchain_image_index(screen.swapchain.clone(), image_index))
            .then_signal_fence_and_flush();
        match frame_end.map_err(Validated::unwrap) {
            Ok(frame_end) => {
                if !self.drawn_once {
                    self.drawn_once = true;
                    startup::moment("Loading screen first drawn");
                }
                if let Some((path, buffer)) = &shot {
                    frame_end.wait(None).unwrap();
                    save_screen(buffer, image.extent(), image.format(), path);
                }
                screen.presented.showing(image_index, semaphore);
                screen.previous_frame_end = Some(frame_end.boxed());
            }
            Err(VulkanError::OutOfDate) => {
                screen.recreate_swapchain = true;
                screen.previous_frame_end = Some(sync::now(gpu.device.clone()).boxed());
            }
            Err(e) => panic!("failed to flush frame: {e}"),
        }
    }
}

//// Shots of the loading screen, see previewing()
struct Preview {
    /// The screenshot's, which they're named after
    path: PathBuf,
    taken: usize,
    /// With `--set loading_shots=12`, that many shots as the bar fills, one each time it's gone
    /// another twelfth of the way, as well as the last: for a sheet of it all through a load
    every: usize,
}

impl Preview {
    fn new(path: PathBuf) -> Self {
        let every = settings_from_args().iter().find(|(k, _)| k == "loading_shots").and_then(|(_, v)| v.parse().ok()).unwrap_or(0);
        Preview { path, taken: 0, every }
    }

    /// Where the next shot goes if it's time for one: once it's faded in, once loading's halfway
    /// and on the `last`, before the game takes over, or with `every` as the bar fills
    fn next(&mut self, fraction: f32, appeared: bool, last: bool) -> Option<PathBuf> {
        const DONE: usize = usize::MAX;
        let name = if self.taken == DONE {
            return None;
        } else if last {
            "loaded".to_owned()
        } else if self.every > 0 {
            if !appeared || fraction * (self.every as f32) < self.taken as f32 {
                return None;
            }
            format!("{:02}", self.taken)
        } else {
            match self.taken {
                0 if appeared => "early".to_owned(),
                1 if fraction >= 0.45 => "middle".to_owned(),
                _ => return None,
            }
        };
        self.taken = if last { DONE } else { self.taken + 1 };
        let stem = self.path.file_stem().map_or("innert".into(), |s| s.to_string_lossy());
        Some(self.path.with_file_name(format!("{stem}-loading-{name}.png")))
    }
}

/// How many loading screens have been shown, so each starts on another tip
static SHOWN: AtomicU32 = AtomicU32::new(0);

/// What the loading screen shows, changing as loading goes on
#[derive(Clone)]
struct Look {
    /// When it was first drawn, which everything moving in it is timed from
    start: Instant,
    last: Instant,
    /// How far along the bar is: it follows loading smoothly, never going back
    fraction: f32,
    step: Step,
    /// Whether the shaders are being compiled, which only takes so long the first time
    compiling: bool,
    /// How far the land's built, see land.rs, following loading at a pace that can be seen
    phase: Phase,
    /// The tip it starts on
    tip: usize,
}

impl Look {
    fn new() -> Self {
        let now = Instant::now();
        let tip = SHOWN.fetch_add(1, Ordering::Relaxed) as usize;
        Look { start: now, last: now, fraction: 0.0, step: Step::Graphics, compiling: false, phase: Phase::default(), tip }
    }

    fn advance(&mut self, target: f32, step: Step, compiling: bool, phase: Phase) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        self.fraction = (self.fraction + (target - self.fraction) * (1.0 - (-dt * 4.0).exp())).max(self.fraction);
        self.step = step;
        self.compiling = compiling;
        // The land comes up, the woods grow and the seas fill no faster than this many times a
        // second, so it's seen however quickly they're made
        let follow = |shown: f32, wanted: f32, most: f32| (shown + ((wanted - shown) * (1.0 - (-dt * 5.0).exp())).min(most * dt)).max(shown).min(1.0);
        self.phase = Phase { rise: follow(self.phase.rise, phase.rise, 0.8), grow: follow(self.phase.grow, phase.grow, 1.0), fill: follow(self.phase.fill, phase.fill, 0.7) };
    }
}

/// The words over the loading screen: its title, in blackletter, and under it what's being loaded
struct Words<'a> {
    title: &'a str,
    subtitle: Option<&'a str>,
}

/// The loading screen over the game while the world's built afresh round where it goes on, behind
/// the dark, see stream.rs: as the one Innert starts with, with the land round where the player's
/// to stand rising on it
pub struct Board {
    progress: Arc<Progress>,
    look: Look,
    title: String,
    subtitle: String,
}

impl Board {
    /// Showing how far `progress` has got, under `title` and `subtitle`
    pub fn new(progress: Arc<Progress>, title: String, subtitle: String) -> Self {
        Board { progress, look: Look::new(), title, subtitle }
    }

    pub fn progress(&self) -> &Arc<Progress> {
        &self.progress
    }

    /// Draws it over everything else in `ctx`, `alpha` of the way opaque, fading in as it's first
    /// shown
    pub fn paint(&mut self, ctx: &egui::Context, alpha: f32) {
        let (fraction, step) = self.progress.estimate();
        self.look.advance(fraction, step, false, self.progress.phase());
        let appearing = smoothstep(self.look.start.elapsed().as_secs_f32() / FADE_IN_SECONDS);
        let land = self.progress.land();
        let words = Words { title: &self.title, subtitle: Some(&self.subtitle) };
        paint(ctx, &self.look, alpha * appearing, LayerId::new(egui::Order::Tooltip, egui::Id::new("loading screen")), None, land.as_deref(), &words);
    }
}

/// Fades the loading screen away over the game's first frames: it's drawn over them by the GUI
/// the game goes on drawing with, the bar filling up and the land finishing as it goes, and the
/// name gliding over to where the menu has it
fn fade_away(ctx: &egui::Context, look: Look, land: Option<Arc<Land>>) {
    let began = Mutex::new(None::<Instant>);
    ctx.on_end_pass(
        "loading screen fading away",
        Arc::new(move |ctx| {
            // From the first frame, which can take a while to come
            let t = began.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(Instant::now).elapsed().as_secs_f32();
            if t >= FADE_SECONDS {
                return;
            }
            let mut look = look.clone();
            let done = smoothstep(t / (FADE_SECONDS * 0.3));
            look.fraction += (1.0 - look.fraction) * done;
            let p = look.phase;
            look.phase = Phase { rise: p.rise + (1.0 - p.rise) * done, grow: p.grow + (1.0 - p.grow) * done, fill: p.fill + (1.0 - p.fill) * done };
            let alpha = 1.0 - smoothstep(t / FADE_SECONDS);
            // Where the menu is, see App::render(), if it's up
            let menu = ctx.memory(|m| m.area_rect(egui::Id::new("menu")));
            let glide = menu.map(|menu| (menu, t / (FADE_SECONDS * 0.55)));
            let words = Words { title: "Innert", subtitle: None };
            paint(ctx, &look, alpha, LayerId::new(egui::Order::Foreground, egui::Id::new("loading screen")), glide, land.as_deref(), &words);
        }),
    );
}

// Puts this thread ahead of the world being built while loading, which keeps every core busy,
/// or back level with it. The graphics card started half again as slowly without, and the loading
/// screen would have to wait its turn to be drawn.
#[cfg(windows)]
fn hurry(ahead: bool) {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentThread() -> *mut std::ffi::c_void;
        fn SetThreadPriority(thread: *mut std::ffi::c_void, priority: i32) -> i32;
    }
    const THREAD_PRIORITY_NORMAL: i32 = 0;
    const THREAD_PRIORITY_ABOVE_NORMAL: i32 = 1;
    unsafe {
        SetThreadPriority(GetCurrentThread(), if ahead { THREAD_PRIORITY_ABOVE_NORMAL } else { THREAD_PRIORITY_NORMAL });
    }
}

#[cfg(not(windows))]
fn hurry(_ahead: bool) {}

/// The near black of the loading screen, which the window's filled with until it's drawn in it,
/// and it fades in over
const INK: [u8; 3] = [8, 8, 8];

/// Fills the window with the loading screen's black, before anything can be drawn in it with the
/// graphics card, so it can be shown straight away rather than with whatever's left in it
#[cfg(windows)]
fn fill(window: &Window) {
    use std::ffi::c_void;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    #[repr(C)]
    #[derive(Default)]
    struct WinRect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(window: *mut c_void) -> *mut c_void;
        fn ReleaseDC(window: *mut c_void, dc: *mut c_void) -> i32;
        fn GetClientRect(window: *mut c_void, rect: *mut WinRect) -> i32;
        fn FillRect(dc: *mut c_void, rect: *const WinRect, brush: *mut c_void) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateSolidBrush(color: u32) -> *mut c_void;
        fn DeleteObject(object: *mut c_void) -> i32;
    }
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
    let hwnd = handle.hwnd.get() as *mut c_void;
    unsafe {
        let dc = GetDC(hwnd);
        if dc.is_null() {
            return;
        }
        let mut rect = WinRect::default();
        GetClientRect(hwnd, &mut rect);
        let brush = CreateSolidBrush(INK[0] as u32 | (INK[1] as u32) << 8 | (INK[2] as u32) << 16);
        if !brush.is_null() {
            FillRect(dc, &rect, brush);
            DeleteObject(brush);
        }
        ReleaseDC(hwnd, dc);
    }
}

#[cfg(not(windows))]
fn fill(_window: &Window) {}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// A number from 0 to 1 that's always the same for `i` and `k`
fn hash(i: u32, k: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9e37_79b1) ^ k.wrapping_mul(0x85eb_ca77).rotate_left(13);
    x ^= x >> 15;
    x = x.wrapping_mul(0x2c1b_3c6d);
    x ^= x >> 12;
    x = x.wrapping_mul(0x297a_2d39);
    x ^= x >> 15;
    x as f32 / u32::MAX as f32
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// A glow, `color` in the middle fading out to nothing `radius` away
fn glow(mesh: &mut Mesh, center: Pos2, radius: egui::Vec2, color: Color32) {
    const SIDES: u32 = 40;
    let first = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);
    for k in 0..=SIDES {
        let a = k as f32 / SIDES as f32 * TAU;
        mesh.colored_vertex(center + vec2(a.cos() * radius.x, a.sin() * radius.y), Color32::TRANSPARENT);
    }
    for k in 0..SIDES {
        mesh.add_triangle(first, first + 1 + k, first + 2 + k);
    }
}

/// Tips shown along the foot of the loading screen, a few seconds each
const TIPS: [&str; 12] = [
    "Hold G at a tree, rock or clay to gather it. Tools gather two to four times as fast.",
    "Press B to build, then 1 to 9 and 0 to pick a piece, its cost beside it. R turns it.",
    "Once you have wings, jump, then hold Space in the air to glide.",
    "Merchants sell wings, and bosses and chests leave them.",
    "Press E to pick things up, open chests, trade and pet animals.",
    "Right click swings heavily, or eats what's in your hand.",
    "A legendary weapon has a special on R, and right click calls on its power.",
    "Crossbows and guns reload with Q or right click. T changes what they shoot.",
    "Press I for your inventory, and craft tools there. M opens the map.",
    "Draw a bow by holding right click, then click to loose the arrow.",
    "Shift runs and F flies. F1 opens the settings, Esc the menu.",
    "In Realms, beat a realm's three raid bosses for its Innert blocks, and set them in the gate.",
];
/// How long each tip's shown, in seconds
const TIP_SECONDS: f32 = 7.0;

/// Draws the loading screen over the whole window in `layer`, `alpha` of the way opaque: on near
/// black, framed as a printed page, the title in blackletter, under it `land` rising as it's
/// built, then the bar, what's being made and a tip. With `glide`, the menu and how far from 0 to
/// 1 the name's moved over to where it has it, staying solid until it's there.
fn paint(ctx: &egui::Context, look: &Look, alpha: f32, layer: LayerId, glide: Option<(Rect, f32)>, land: Option<&Land>, words: &Words) {
    use crate::theme;
    let painter = ctx.layer_painter(layer);
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let t = look.start.elapsed().as_secs_f32();
    let fade = |c: Color32| c.gamma_multiply(alpha);

    // Near black, lit a little behind the land, framed in two fine rules as a printed page
    painter.rect_filled(screen, 0.0, fade(rgb(INK)));
    let mut mesh = Mesh::default();
    glow(&mut mesh, pos2(w * 0.5, h * 0.5), vec2(w * 0.46, h * 0.42), fade(Color32::from_gray(18)));
    painter.add(Shape::mesh(mesh));
    let page = screen.shrink((w.min(h) * 0.025).clamp(10.0, 22.0));
    painter.rect_stroke(page, 0.0, Stroke::new(1.0f32, fade(Color32::from_white_alpha(40))), StrokeKind::Inside);
    painter.rect_stroke(page.shrink(4.0), 0.0, Stroke::new(1.0f32, fade(Color32::from_white_alpha(16))), StrokeKind::Inside);

    // The title, gliding over to the menu's own once that's up
    let center = w * 0.5;
    let innert = glide.is_some() || words.title == "Innert";
    let size = if innert { crate::session::TITLE_SIZE } else { (crate::session::TITLE_SIZE * 0.7).min(w / (words.title.len() as f32 * 0.42).max(1.0)) };
    let at = pos2(center, (h * 0.15).max(size * 0.5 + 24.0));
    let (mut name, mut solid) = (at, alpha);
    if let Some((menu, along)) = glide {
        // Where the menu has it, see session::menu()
        name = name.lerp(crate::session::title_at(menu), smoothstep(along));
        if along < 1.0 {
            solid = 1.0;
        }
    }
    // Shadowed, as the menu has it
    painter.text(name + vec2(2.0, 3.0), Align2::CENTER_CENTER, words.title, theme::blackletter(size), Color32::from_black_alpha((190.0 * solid) as u8));
    painter.text(name, Align2::CENTER_CENTER, words.title, theme::blackletter(size), theme::HEADING.gamma_multiply(solid));
    // Under its letters' tails
    let mut below = at.y + size * if innert { 0.5 } else { 0.62 } + 6.0;
    ornament(&painter, pos2(center, below), 120.0, fade(theme::ACCENT));
    if let Some(subtitle) = words.subtitle.filter(|s| !s.is_empty()) {
        below += 20.0;
        painter.text(pos2(center, below), Align2::CENTER_CENTER, subtitle, theme::display(16.0), fade(theme::WEAK));
    }

    // The bar, what's being made and how far along, and a tip under them
    let bar_y = h * 0.82;
    let tip_y = (h * 0.905).max(bar_y + 50.0).min(page.bottom() - 18.0);

    // The land being built, in what's left between
    if let Some(land) = land {
        let top = below + 16.0;
        let bottom = bar_y - 24.0;
        let across = (w * 0.64).min((bottom - top) * 1.9);
        // Coming later and gone sooner than the rest as it fades in and away, so it isn't seen
        // through the menu or the game, the more so over the menu
        let shown = if glide.is_some() { alpha.powi(3) } else { alpha * alpha };
        if bottom > top + 40.0 {
            land.paint(&painter, Rect::from_min_max(pos2(center - across * 0.5, top), pos2(center + across * 0.5, bottom)), t, look.phase, shown);
        }
    }

    let width = (w * 0.42).clamp(220.0, 440.0);
    let bar = Rect::from_min_size(pos2(center - width * 0.5, bar_y), vec2(width, 3.0));
    painter.rect_filled(bar.expand(2.0), 0.0, fade(Color32::from_black_alpha(200)));
    painter.rect_stroke(bar.expand(2.0), 0.0, Stroke::new(1.0f32, fade(theme::EDGE.gamma_multiply(0.7))), StrokeKind::Outside);
    let filled = Rect::from_min_max(bar.min, pos2(bar.min.x + width * look.fraction, bar.max.y));
    let mut mesh = Mesh::default();
    let first = mesh.vertices.len() as u32;
    // Grey to white, as the menu it gives way to
    let (cool, warm) = (fade(Color32::from_gray(120)), fade(Color32::from_gray(248)));
    mesh.colored_vertex(filled.left_top(), cool);
    mesh.colored_vertex(filled.right_top(), warm);
    mesh.colored_vertex(filled.right_bottom(), warm);
    mesh.colored_vertex(filled.left_bottom(), cool);
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
    // A glint running along it, so it's plainly going even while a step takes its time
    let glint = bar.min.x + filled.width() * ((t * 0.6) % 1.0);
    glow(&mut mesh, pos2(glint, bar.center().y), vec2(18.0, 4.0), fade(Color32::from_white_alpha(90)));
    glow(&mut mesh, pos2(filled.max.x, filled.center().y), vec2(16.0, 9.0), fade(Color32::from_white_alpha(70)));
    painter.add(Shape::mesh(mesh));
    theme::diamond(&painter, pos2(filled.max.x, bar.center().y), 3.5, fade(theme::ACCENT));

    let words_y = bar.max.y + 12.0;
    painter.text(pos2(bar.min.x, words_y), Align2::LEFT_TOP, format!("{}…", look.step.label()), theme::display(14.0), fade(theme::TEXT));
    painter.text(pos2(bar.max.x, words_y), Align2::RIGHT_TOP, format!("{:.0}%", look.fraction * 100.0), FontId::proportional(13.0), fade(theme::WEAK));
    if look.compiling && look.step == Step::Shaders {
        painter.text(pos2(center, words_y + 22.0), Align2::CENTER_TOP, "It only takes this long the first time", FontId::proportional(12.5), fade(theme::WEAK));
    }

    // A tip, each in turn, fading from one to the next
    let turn = t / TIP_SECONDS;
    let within = (turn.fract()) * TIP_SECONDS;
    let shown = smoothstep(within / 0.6) * smoothstep((TIP_SECONDS - within) / 0.6);
    let tip = TIPS[(look.tip + turn as usize) % TIPS.len()];
    if tip_y > words_y + 30.0 {
        painter.text(pos2(center, tip_y), Align2::CENTER_CENTER, tip, FontId::proportional(13.5), fade(theme::WEAK).gamma_multiply(shown));
    }
}

/// A printer's ornament under a title, as theme::flourish() but in `color`, to fade with the rest
fn ornament(painter: &egui::Painter, at: Pos2, half: f32, color: Color32) {
    let mut mesh = Mesh::default();
    for side in [-1.0, 1.0f32] {
        let (a, b) = (at.x + side * 15.0, at.x + side * half);
        let first = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(a, at.y - 0.6), color);
        mesh.colored_vertex(pos2(a, at.y + 0.6), color);
        mesh.colored_vertex(pos2(b, at.y - 0.6), Color32::TRANSPARENT);
        mesh.colored_vertex(pos2(b, at.y + 0.6), Color32::TRANSPARENT);
        mesh.add_triangle(first, first + 1, first + 2);
        mesh.add_triangle(first + 1, first + 3, first + 2);
    }
    painter.add(Shape::mesh(mesh));
    for side in [-1.0, 1.0f32] {
        crate::theme::diamond(painter, at + vec2(side * 9.5, 0.0), 2.0, color);
    }
    crate::theme::diamond(painter, at, 4.5, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_taking_longer_still_seems_to_get_on() {
        // In step with the time at first, then ever slower, never stopping or going back
        assert!((still_to_go(0.0) - 1.0).abs() < 1e-6);
        assert!((still_to_go(0.5) - 0.575).abs() < 1e-3);
        let mut last = 2.0;
        for k in 0..400 {
            let left = still_to_go(k as f32 * 0.025);
            assert!(left < last && left > 0.0, "at {k}: {left} after {last}");
            last = left;
        }
    }

    #[test]
    fn the_bar_follows_the_steps() {
        let progress = Progress::arriving();
        let (fraction, step) = progress.estimate();
        assert!(fraction < 0.05, "nothing's built yet: {fraction}");
        assert_eq!(step, Step::Ground);
        for step in [Step::Ground, Step::Rivers, Step::Land, Step::Voxels] {
            progress.run(step, || ());
        }
        let (built, step) = progress.estimate();
        assert!(built > fraction && built < 1.0, "{built}");
        assert_eq!(step, Step::Grass);
        assert_eq!(progress.phase().rise, 1.0);
        assert_eq!(progress.phase().fill, 0.0);
        for step in [Step::Grass, Step::Upload, Step::Water] {
            progress.run(step, || ());
        }
        let (done, _) = progress.estimate();
        assert_eq!(done, 1.0);
        assert_eq!(progress.phase(), Phase::DONE);
    }

    #[test]
    fn the_land_is_built_at_a_pace_that_can_be_seen() {
        let mut look = Look::new();
        look.last -= Duration::from_millis(100);
        look.advance(1.0, Step::Grass, false, Phase::DONE);
        assert!(look.phase.rise > 0.0 && look.phase.rise <= 0.081, "{:?}", look.phase);
        assert!(look.fraction > 0.0 && look.fraction < 1.0);
    }

    #[test]
    fn every_tip_fits_a_small_window() {
        for tip in TIPS {
            assert!(tip.len() < 100, "{tip}");
        }
    }

    #[test]
    fn previews_are_named_in_turn() {
        let mut preview = Preview { path: PathBuf::from("shot.png"), taken: 0, every: 4 };
        assert!(preview.next(0.0, false, false).is_none());
        assert_eq!(preview.next(0.0, true, false).unwrap(), PathBuf::from("shot-loading-00.png"));
        assert!(preview.next(0.1, true, false).is_none());
        assert_eq!(preview.next(0.3, true, false).unwrap(), PathBuf::from("shot-loading-01.png"));
        assert_eq!(preview.next(0.4, true, true).unwrap(), PathBuf::from("shot-loading-loaded.png"));
        assert!(preview.next(1.0, true, true).is_none());
    }
}
