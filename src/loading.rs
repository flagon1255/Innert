//! The loading screen. The window's up with it as soon as Innert starts, while what the game needs
//! is made on other threads, see Loaded::load(). Once it's all there the game takes over the same
//! window, with what's drawn to it, the swapchain and the GUI, and the loading screen fades away
//! over the menu.

use std::f32::consts::{PI, TAU};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::rc::Rc;
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::{Duration, Instant};

use egui_winit_vulkano::egui::{self, Align2, Color32, FontId, LayerId, Mesh, Pos2, Rect, Shape, Stroke, pos2, vec2};
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

use crate::{App, Capture, Gpu, Loaded, get_swapchain_images, save_screen, settings_from_args, startup, window_attributes, worldgen};

/// The loading screen's drawn this often
const FRAME_TIME: Duration = Duration::from_micros(16_667);
/// How long the loading screen takes to fade in over the night sky's color the window's filled with
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
            Step::Ground => "Shaping the land",
            Step::Rivers => "Carving rivers",
            Step::Land => "Raising towns and castles",
            Step::Voxels => "Laying down voxels",
            Step::Grass => "Growing grass",
            Step::Upload => "Sending the world to the graphics card",
            Step::Water => "Filling the water",
            Step::Creatures => "Carving creatures",
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
    /// How long the shaders should take
    shaders_ms: AtomicU32,
    /// Set once the window's closed while loading, to stop where it can
    cancelled: AtomicBool,
}

impl Progress {
    fn new() -> Self {
        Progress {
            times: Mutex::new([(None, None); STEPS]),
            shaders_ms: AtomicU32::new(Step::Shaders.expected() as u32),
            cancelled: AtomicBool::new(false),
        }
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
            startup::finished(step.label(), now - started.unwrap_or(now));
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
        const WORLD: [Step; 5] = [Step::Ground, Step::Rivers, Step::Land, Step::Voxels, Step::Grass];
        let progress = self.clone();
        worldgen::on_step(Some(Box::new(move |n| {
            if let Some(&before) = n.checked_sub(1).and_then(|n| WORLD.get(n)) {
                progress.end(before);
            }
            if let Some(&step) = WORLD.get(n) {
                progress.begin(step);
            }
        })));
        let built = build();
        worldgen::on_step(None);
        for step in WORLD {
            self.end(step);
        }
        built
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// How far along it all is, from 0 to 1, by how long what's left should take, and what's being
    /// made meanwhile
    fn estimate(&self) -> (f32, Step) {
        let times = self.times();
        let shaders = self.shaders_ms.load(Ordering::Relaxed) as f32;
        let (mut total, mut left, mut current) = ([0.0f32; 3], [0.0f32; 3], [None; 3]);
        for step in Step::ALL {
            let expected = if step == Step::Shaders { shaders } else { step.expected() };
            let track = step.track();
            total[track] += expected;
            left[track] += match times[step as usize] {
                (_, Some(_)) => 0.0,
                // Taking longer than expected, it's still not done
                (Some(started), None) => (expected - started.elapsed().as_secs_f32() * 1000.0).max(expected * 0.15),
                (None, None) => expected,
            };
            if times[step as usize].1.is_none() && current[track].is_none() {
                current[track] = Some(step);
            }
        }
        let fraction = 1.0 - (left[0].max(left[1]) + left[2]) / (total[0].max(total[1]) + total[2]);
        // Whichever of the graphics card and the world has longer to go, then the rest in turn
        let step = match current {
            [None, None, rest] => rest,
            [card, world, _] if left[0] > left[1] => card.or(world),
            [card, world, _] => world.or(card),
        };
        (fraction.clamp(0.0, 1.0), step.unwrap_or(Step::Creatures))
    }
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
        let preview = previewing().then(|| Preview { path: Capture::from_args().unwrap().path, taken: 0 });
        let now = Instant::now();
        let loading = Loading {
            out_of_sight,
            drawn: !out_of_sight || preview.is_some(),
            window: None,
            drawn_once: false,
            gpu: None,
            screen: None,
            look: Look { start: now, last: now, fraction: 0.0, step: Step::Graphics, compiling: false },
            next_frame: now,
            preview,
            loaded: None,
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
        let Loading { loaded: Some(loaded), screen: Some(screen), look, drawn, .. } = *loading else {
            event_loop.exit();
            return;
        };
        let mut app = Box::new(App::new(*loaded));
        self.progress.end(Step::Creatures);
        // The game's from here on
        *WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = None;
        // The game runs as it always has
        hurry(false);
        if drawn {
            fade_away(&screen.gui.context(), look);
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
}

impl Loading {
    /// Makes the window and shows it straight away, filled with the night sky's color until the
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
            // Everything moving in it starts from now, fading in over the night sky's color
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
        self.look.advance(fraction, step, progress.shaders_compiling());
        let look = self.look.clone();
        let appearing = smoothstep(look.start.elapsed().as_secs_f32() / FADE_IN_SECONDS);
        screen.gui.immediate_ui(|gui| paint(&gui.context(), &look, appearing, LayerId::background(), None));

        let view = screen.image_views[image_index as usize].clone();
        let image = view.image().clone();
        // The GUI's drawn over what's there
        let mut builder = AutoCommandBufferBuilder::primary(gpu.command_buffer_allocator.clone(), gpu.queue.queue_family_index(), CommandBufferUsage::OneTimeSubmit).unwrap();
        builder
            .clear_color_image(ClearColorImageInfo {
                clear_value: [NIGHT[0] as f32 / 255.0, NIGHT[1] as f32 / 255.0, NIGHT[2] as f32 / 255.0, 1.0].into(),
                ..ClearColorImageInfo::image(image.clone())
            })
            .unwrap();
        let cleared = screen.previous_frame_end.take().unwrap().join(acquired).then_execute(gpu.queue.clone(), builder.build().unwrap()).unwrap();
        let drawn = screen.gui.draw_on_image(cleared, view);

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

/// Shots of the loading screen, see previewing()
struct Preview {
    /// The screenshot's, which they're named after
    path: PathBuf,
    taken: usize,
}

impl Preview {
    /// Where the next shot goes if it's time for one: once it's faded in, once loading's halfway
    /// and on the `last`, before the game takes over
    fn next(&mut self, fraction: f32, appeared: bool, last: bool) -> Option<PathBuf> {
        let name = match self.taken {
            0 if appeared && !last => "early",
            0..3 if last => "loaded",
            1 if fraction >= 0.45 => "middle",
            _ => return None,
        };
        self.taken = if last { 3 } else { self.taken + 1 };
        let stem = self.path.file_stem().map_or("innert".into(), |s| s.to_string_lossy());
        Some(self.path.with_file_name(format!("{stem}-loading-{name}.png")))
    }
}

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
}

impl Look {
    fn advance(&mut self, target: f32, step: Step, compiling: bool) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        self.fraction = (self.fraction + (target - self.fraction) * (1.0 - (-dt * 4.0).exp())).max(self.fraction);
        self.step = step;
        self.compiling = compiling;
    }
}

/// Fades the loading screen away over the game's first frames: it's drawn over them by the GUI
/// the game goes on drawing with, the bar filling up as it goes, and the name gliding over to
/// where the menu has it
fn fade_away(ctx: &egui::Context, look: Look) {
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
            look.fraction += (1.0 - look.fraction) * smoothstep(t / (FADE_SECONDS * 0.3));
            let alpha = 1.0 - smoothstep(t / FADE_SECONDS);
            // Where the menu is, see App::render(), if it's up
            let menu = ctx.memory(|m| m.area_rect(egui::Id::new("menu")));
            let glide = menu.map(|menu| (menu, t / (FADE_SECONDS * 0.55)));
            paint(ctx, &look, alpha, LayerId::new(egui::Order::Foreground, egui::Id::new("loading screen")), glide);
        }),
    );
}

/// Puts this thread ahead of the world being built while loading, which keeps every core busy,
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

/// The color the window's filled with until the loading screen's drawn in it, its night sky's at
/// the top, which it fades in over
const NIGHT: [u8; 3] = [7, 10, 26];

/// Fills the window with the night sky's color, before anything can be drawn in it with the
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
        let brush = CreateSolidBrush(NIGHT[0] as u32 | (NIGHT[1] as u32) << 8 | (NIGHT[2] as u32) << 16);
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

/// Between `a` and `b`, `t` of the way
fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

fn shade(c: [u8; 3], by: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * by).round().clamp(0.0, 255.0) as u8)
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

/// A rectangle shading from `top` to `bottom`
fn gradient(mesh: &mut Mesh, rect: Rect, top: Color32, bottom: Color32) {
    let first = mesh.vertices.len() as u32;
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), bottom);
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
}

/// Draws the loading screen over the whole window in `layer`, `alpha` of the way opaque. Night
/// gives way to dawn over a blocky landscape as loading goes on, with a little cube turning over
/// the name. With `glide`, the menu and how far from 0 to 1 the name's moved over to where it has
/// it, staying solid until it's there.
fn paint(ctx: &egui::Context, look: &Look, alpha: f32, layer: LayerId, glide: Option<(Rect, f32)>) {
    let painter = ctx.layer_painter(layer);
    let screen = ctx.screen_rect();
    let (w, h) = (screen.width(), screen.height());
    let t = look.start.elapsed().as_secs_f32();
    let dawn = smoothstep(look.fraction);
    let fade = |c: Color32| c.gamma_multiply(alpha);
    let mut mesh = Mesh::default();

    // The sky, night giving way to dawn
    let horizon = h * 0.64;
    let sky = [
        (0.0, mix(NIGHT, [30, 46, 92], dawn)),
        (0.42, mix([17, 24, 52], [84, 104, 158], dawn)),
        (0.64, mix([46, 38, 74], [236, 168, 116], dawn)),
        (1.0, mix([24, 20, 40], [120, 84, 80], dawn)),
    ];
    for pair in sky.windows(2) {
        let rect = Rect::from_min_max(pos2(0.0, pair[0].0 * h), pos2(w, pair[1].0 * h));
        gradient(&mut mesh, rect, fade(rgb(pair[0].1)), fade(rgb(pair[1].1)));
    }

    // Stars, fading as it gets light
    let starlight = (1.0 - look.fraction * 1.4).max(0.0);
    for i in 0..110 {
        let at = pos2(hash(i, 1) * w, hash(i, 2).powf(1.5) * horizon * 0.95);
        let twinkle = 0.55 + 0.45 * (t * (0.6 + hash(i, 3) * 2.2) + hash(i, 4) * TAU).sin();
        let brightness = starlight * twinkle * (0.3 + 0.7 * hash(i, 5));
        let size = 1.0 + (hash(i, 6) * 1.6).floor();
        mesh.add_colored_rect(Rect::from_center_size(at, vec2(size, size)), fade(Color32::from_white_alpha((brightness * 230.0) as u8)));
    }

    // The sun, coming up off to one side, just peeking over the hills by the end
    let sun = pos2(w * 0.8, horizon + h * (0.06 - 0.1 * dawn));
    let reach = w.max(h);
    glow(&mut mesh, sun, vec2(reach * 0.55, reach * 0.38), fade(Color32::from_rgba_unmultiplied(255, 180, 120, (20.0 + 70.0 * dawn) as u8)));
    glow(&mut mesh, sun, vec2(reach * 0.12, reach * 0.12), fade(Color32::from_rgba_unmultiplied(255, 214, 160, (30.0 + 120.0 * dawn) as u8)));
    let disc = (w.min(h) * 0.035).max(10.0);
    glow(&mut mesh, sun, vec2(disc * 2.2, disc * 2.2), fade(Color32::from_rgba_unmultiplied(255, 236, 200, (160.0 * dawn) as u8)));
    painter.add(Shape::mesh(std::mem::take(&mut mesh)));
    painter.circle_filled(sun, disc, fade(Color32::from_rgba_unmultiplied(255, 238, 206, (255.0 * dawn) as u8)));

    // Blocky hills drifting by, the farthest hazy with the sky's light, the nearest made of voxels
    // with grass on top
    struct Hills {
        block: f32,
        base: f32,
        height: f32,
        speed: f32,
        seed: f32,
        night: [u8; 3],
        day: [u8; 3],
    }
    let layers = [
        Hills { block: 9.0, base: 0.665, height: 0.075, speed: 4.0, seed: 1.3, night: [34, 30, 62], day: [166, 128, 140] },
        Hills { block: 14.0, base: 0.76, height: 0.07, speed: 9.0, seed: 4.1, night: [20, 21, 44], day: [86, 84, 104] },
        Hills { block: 22.0, base: 0.875, height: 0.06, speed: 17.0, seed: 7.7, night: [10, 12, 25], day: [36, 46, 48] },
    ];
    for (n, hills) in layers.iter().enumerate() {
        let front = n == layers.len() - 1;
        let color = mix(hills.night, hills.day, dawn);
        let grass = mix([22, 44, 34], [86, 140, 74], dawn);
        let scrolled = t * hills.speed;
        let first = (scrolled / hills.block).floor();
        let offset = scrolled - first * hills.block;
        for k in 0..(w / hills.block) as i64 + 2 {
            let column = first as i64 + k;
            let x = column as f32 * hills.block;
            let wave = 0.55 * (x * 0.0045 + hills.seed).sin() + 0.3 * (x * 0.0113 + hills.seed * 2.3).sin() + 0.15 * (x * 0.029 + hills.seed * 5.1).sin();
            let top = ((hills.base - hills.height * wave) * h / hills.block).round() * hills.block;
            let left = k as f32 * hills.block - offset;
            if !front {
                mesh.add_colored_rect(Rect::from_min_max(pos2(left, top), pos2(left + hills.block, h)), fade(rgb(color)));
                continue;
            }
            // Each voxel a little lighter or darker than the next
            let mut y = top;
            let mut row = 0;
            while y < h {
                let tone = if row == 0 { grass } else { shade(color, 0.85 + 0.3 * hash(column as u32, row)) };
                mesh.add_colored_rect(Rect::from_min_max(pos2(left, y), pos2(left + hills.block, y + hills.block)), fade(rgb(tone)));
                y += hills.block;
                row += 1;
            }
        }
    }

    // Fireflies drifting up out of the hills, fewer once it's light
    for i in 0..30 {
        let span = h * 0.42;
        let speed = 6.0 + hash(i, 7) * 14.0;
        let risen = (t * speed + hash(i, 8) * span) % span;
        let at = pos2(hash(i, 9) * w + (t * (0.4 + hash(i, 10)) + hash(i, 11) * TAU).sin() * 14.0, h - risen);
        let flicker = 0.6 + 0.4 * (t * (2.0 + hash(i, 12) * 3.0) + i as f32).sin();
        let a = (risen / span * PI).sin() * flicker * (1.0 - 0.6 * dawn);
        glow(&mut mesh, at, vec2(7.0, 7.0), fade(Color32::from_rgba_unmultiplied(255, 200, 110, (a * 60.0) as u8)));
        mesh.add_colored_rect(Rect::from_center_size(at, vec2(2.0, 2.0)), fade(Color32::from_rgba_unmultiplied(255, 226, 150, (a * 255.0) as u8)));
    }
    painter.add(Shape::mesh(std::mem::take(&mut mesh)));

    // The middle of the screen: the cube, the name, the bar and what's being made
    let middle = h * 0.42;
    let center = w * 0.5;
    cube(&painter, pos2(center, middle - 118.0 + (t * 1.7).sin() * 3.0), t, alpha);
    let (mut name, mut solid) = (pos2(center, middle - 18.0), alpha);
    if let Some((menu, along)) = glide {
        // As the menu lays them out, under its margin, see App::render()
        let row = |size: f32| ctx.fonts(|f| f.row_height(&FontId::proportional(size)));
        let to = pos2(menu.center().x, menu.top() + 36.0 + row(60.0) * 0.5);
        name = name.lerp(to, smoothstep(along));
        if along < 1.0 {
            solid = 1.0;
        }
    }
    painter.text(name, Align2::CENTER_CENTER, "INNERT", FontId::proportional(60.0), Color32::WHITE.gamma_multiply(solid));

    let width = (w * 0.56).min(360.0);
    let bar = Rect::from_min_size(pos2(center - width * 0.5, middle + 78.0), vec2(width, 4.0));
    painter.rect_filled(bar.expand(2.0), 4.0, fade(Color32::from_black_alpha(70)));
    painter.rect_filled(bar, 2.0, fade(Color32::from_white_alpha(34)));
    let filled = Rect::from_min_max(bar.min, pos2(bar.min.x + width * look.fraction, bar.max.y));
    let first = mesh.vertices.len() as u32;
    let (cool, warm) = (fade(Color32::from_rgb(236, 158, 96)), fade(Color32::from_rgb(255, 230, 176)));
    mesh.colored_vertex(filled.left_top(), cool);
    mesh.colored_vertex(filled.right_top(), warm);
    mesh.colored_vertex(filled.right_bottom(), warm);
    mesh.colored_vertex(filled.left_bottom(), cool);
    mesh.add_triangle(first, first + 1, first + 2);
    mesh.add_triangle(first, first + 2, first + 3);
    glow(&mut mesh, pos2(filled.max.x, filled.center().y), vec2(16.0, 9.0), fade(Color32::from_rgba_unmultiplied(255, 220, 160, 90)));
    painter.add(Shape::mesh(mesh));

    let below = bar.max.y + 12.0;
    painter.text(pos2(bar.min.x, below), Align2::LEFT_TOP, format!("{}…", look.step.label()), FontId::proportional(14.0), fade(Color32::from_gray(218)));
    painter.text(pos2(bar.max.x, below), Align2::RIGHT_TOP, format!("{:.0}%", look.fraction * 100.0), FontId::proportional(13.0), fade(Color32::from_gray(150)));
    if look.compiling && look.step == Step::Shaders {
        painter.text(pos2(center, below + 26.0), Align2::CENTER_TOP, "It only takes this long the first time", FontId::proportional(12.5), fade(Color32::from_gray(160)));
    }
}

/// A cube turning slowly over the name, its sides the colors of the game's icon
fn cube(painter: &egui::Painter, center: Pos2, t: f32, alpha: f32) {
    let size = 24.0;
    let (turn, tilt) = (t * 0.55, 0.5f32);
    // Turned about the upright, then tipped towards the viewer, who's looking down +z
    let place = |[x, y, z]: [f32; 3]| {
        let (x, z) = (x * turn.cos() + z * turn.sin(), -x * turn.sin() + z * turn.cos());
        let (y, z) = (y * tilt.cos() - z * tilt.sin(), y * tilt.sin() + z * tilt.cos());
        (center + vec2(x, -y) * size, z)
    };
    let (red, green, blue) = ([224, 84, 72], [80, 190, 102], [78, 116, 226]);
    let faces: [([f32; 3], [[f32; 3]; 4], [u8; 3]); 5] = [
        ([0.0, 1.0, 0.0], [[-1.0, 1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]], blue),
        ([1.0, 0.0, 0.0], [[1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0], [1.0, -1.0, 1.0]], red),
        ([-1.0, 0.0, 0.0], [[-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, 1.0, 1.0], [-1.0, -1.0, 1.0]], red),
        ([0.0, 0.0, 1.0], [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]], green),
        ([0.0, 0.0, -1.0], [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0]], green),
    ];

    // Its shadow on the ground under it
    let mut mesh = Mesh::default();
    glow(&mut mesh, center + vec2(0.0, size * 2.3), vec2(size * 1.7, size * 0.45), Color32::from_black_alpha((90.0 * alpha) as u8));
    for (normal, corners, color) in faces {
        let (_, facing) = place(normal);
        // Only the sides facing the viewer
        if facing <= 0.0 {
            continue;
        }
        let color = rgb(shade(color, 0.55 + 0.45 * facing)).gamma_multiply(alpha);
        let points = corners.map(|c| place(c).0);
        let first = mesh.vertices.len() as u32;
        for p in points {
            mesh.colored_vertex(p, color);
        }
        mesh.add_triangle(first, first + 1, first + 2);
        mesh.add_triangle(first, first + 2, first + 3);
    }
    painter.add(Shape::mesh(mesh));
    // Edges, faintly
    for (normal, corners, _) in faces {
        if place(normal).1 > 0.0 {
            painter.add(Shape::closed_line(corners.map(|c| place(c).0).to_vec(), Stroke::new(1.0f32, Color32::from_black_alpha((60.0 * alpha) as u8))));
        }
    }
}
