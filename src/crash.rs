//! When something goes wrong past putting right: what went wrong is said, and the game ends there,
//! rather than the panic unwinding through what's left of it, or, off its own thread, leaving it
//! going on without, say, the strips being built. Screenshot runs hung after a panic, till stopped
//! from outside minutes later, in what said it: see background_printing::drain() in main.rs.
//!
//! A thread other than the game's own going wrong, as a strip being built, the sounds or the map,
//! the game stops going on and saves first, as it would every minute anyway, see App::emergency_save()
//! in session.rs: it's in a fit state for that between frames. Not when the game's own thread went
//! wrong, which may have been half way through changing what would be saved: then the last save
//! stays as it was, whole, see save::write_file().
//!
//! `--set crash_at=N` in a screenshot run panics on the game's own thread at frame N, and `--set
//! crash_thread=N` in the next thread building the world in the background from frame N on, or in a
//! thread of its own if none comes by soon, to see either ends the game, said why.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Something's panicked: the first panic ends the game, the rest are only said
static PANICKED: AtomicBool = AtomicBool::new(false);
/// It wasn't on the game's own thread, so the game's to save, and stop
static ELSEWHERE: AtomicBool = AtomicBool::new(false);
/// The game's taken that up, and done it
static SAVING: AtomicBool = AtomicBool::new(false);
static SAVED: AtomicBool = AtomicBool::new(false);
/// When the game last went round its loop, in milliseconds from START, 0 for not yet
static LAST_ROUND: AtomicU64 = AtomicU64::new(0);
static START: OnceLock<Instant> = OnceLock::new();
/// `--set crash_thread`: the next thread building the world in the background to come by panics
static WORKER_CRASH: AtomicBool = AtomicBool::new(false);

/// How long the game is waited for to save, at most, when another thread went wrong
const SAVE_WAIT: Duration = Duration::from_secs(5);
/// How long a save being written already is waited for, at most
const WRITE_WAIT: Duration = Duration::from_secs(5);
/// The game's loop counts as going round if it's been round this recently
const GOING_ROUND: Duration = Duration::from_millis(500);

fn millis() -> u64 {
    START.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

/// What's printed and shown, from where it went wrong and with RUST_BACKTRACE set how it got there
fn describe(info: &std::panic::PanicHookInfo) -> String {
    let place = info.location().map_or(String::new(), |l| format!(" ({}:{})", l.file(), l.line()));
    let what = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_default();
    let thread = match std::thread::current().name() {
        Some(name) => format!(" on the {name} thread"),
        None => " on a thread of its own".to_owned(),
    };
    let trace = std::backtrace::Backtrace::capture();
    let trace = if trace.status() == std::backtrace::BacktraceStatus::Captured { format!("\n\n{trace}") } else { String::new() };
    format!("Innert ran into a problem and has to close.\n\n{what}{place}{thread}{trace}")
}

/// From here on, a panic anywhere is said, in a box or printed, and ends the game
pub fn install() {
    START.get_or_init(Instant::now);
    std::panic::set_hook(Box::new(|info| {
        let text = describe(info);
        let main = std::thread::current().name() == Some("main");
        // Another panic's ending the game already, perhaps one this follows from: this one's only
        // printed, and its thread goes no further meanwhile
        if PANICKED.swap(true, Ordering::SeqCst) {
            eprintln!("\nAnd then:\n{text}");
            loop {
                std::thread::park();
            }
        }
        if !main {
            ELSEWHERE.store(true, Ordering::SeqCst);
        }
        crate::show_error(&text);
        // The game's been going round its loop just now, so it'll see to saving, if it hasn't
        // while the box was up
        let round = LAST_ROUND.load(Ordering::SeqCst);
        let going = round > 0 && millis().saturating_sub(round) < GOING_ROUND.as_millis() as u64;
        if !main && going {
            wait_until(|| SAVED.load(Ordering::SeqCst), SAVE_WAIT);
        }
        // A save going to its file already gets there: cut short, it'd leave only the save before
        crate::save::wait_for_writing(WRITE_WAIT);
        end();
    }));
}

/// Ends the game there and then, once what's been printed is out. Not through ExitProcess, which
/// stops the other threads wherever they are and then has each library let go of what it holds:
/// one stopped holding a lock of the graphics driver's, as the game's thread could be, sending a
/// frame, and the driver letting go would wait for it for ever.
fn end() -> ! {
    use std::io::Write;
    // What's been printed but not yet sent on, from a thread of its own in case another holds on
    // to stdout
    let flushing = std::thread::spawn(|| {
        let _ = std::io::stdout().flush();
    });
    wait_until(|| flushing.is_finished(), Duration::from_millis(500));
    crate::background_printing::drain();
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
            fn TerminateProcess(process: *mut std::ffi::c_void, code: u32) -> i32;
        }
        unsafe {
            TerminateProcess(GetCurrentProcess(), 101);
        }
    }
    std::process::exit(101)
}

fn wait_until(done: impl Fn() -> bool, limit: Duration) {
    let since = Instant::now();
    while !done() && since.elapsed() < limit {
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Called each time the game goes round its loop: whether a thread other than its own has panicked,
/// so the game's to stop. The first time it says so, `save` is the game's to save first.
pub fn round(save: impl FnOnce()) -> bool {
    LAST_ROUND.store(millis(), Ordering::SeqCst);
    if !ELSEWHERE.load(Ordering::SeqCst) {
        return false;
    }
    if !SAVING.swap(true, Ordering::SeqCst) {
        save();
        SAVED.store(true, Ordering::SeqCst);
    }
    true
}

/// `--set crash_at` and `--set crash_thread`, read once
fn tests() -> (Option<usize>, Option<usize>) {
    static TESTS: OnceLock<(Option<usize>, Option<usize>)> = OnceLock::new();
    *TESTS.get_or_init(|| {
        let settings = crate::settings_from_args();
        let frame = |key: &str| settings.iter().find(|(k, _)| k == key).and_then(|(_, v)| v.parse().ok());
        (frame("crash_at"), frame("crash_thread"))
    })
}

/// For screenshot runs, at each `frame`: panics if it's the one `--set crash_at` says, or sets the
/// next thread building the world to, if `--set crash_thread` says this one, see worker()
pub fn test(frame: usize) {
    let (at, thread) = tests();
    if at == Some(frame) {
        println!("CRASH TEST on the game's own thread at frame {frame}");
        panic!("a crash for testing, at frame {frame}, as `--set crash_at={frame}` asks");
    }
    let Some(thread) = thread else {
        return;
    };
    if frame == thread {
        println!("CRASH TEST: the next thread building the world panics, from frame {frame}");
        WORKER_CRASH.store(true, Ordering::SeqCst);
    }
    // None came by, as nothing's being built: one of its own, then
    if frame == thread + 300 && WORKER_CRASH.swap(false, Ordering::SeqCst) {
        println!("CRASH TEST: nothing was being built, so on a thread of its own, at frame {frame}");
        let _ = std::thread::Builder::new().name("crash test".into()).spawn(|| panic!("a crash for testing, as `--set crash_thread` asks"));
    }
}

/// Called by the threads building the world in the background now and then, see
/// worldgen::give_way(): panics if `--set crash_thread` has set one to. Only a load of an atomic.
pub fn worker() {
    if WORKER_CRASH.load(Ordering::Relaxed) && WORKER_CRASH.swap(false, Ordering::SeqCst) {
        panic!("a crash for testing, as `--set crash_thread` asks");
    }
}
