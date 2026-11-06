//! How long starting up takes, counted from when Innert was launched rather than from when main()
//! began, so loading the program itself counts too. Printed in screenshot runs, which is where
//! startup is measured.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

static LAUNCH: OnceLock<Instant> = OnceLock::new();
static PRINTING: OnceLock<bool> = OnceLock::new();

/// Notes when the process was made. Called first thing in main().
pub fn init() {
    LAUNCH.get_or_init(|| Instant::now() - launched_ago());
}

/// How long ago Windows made this process
#[cfg(windows)]
fn launched_ago() -> Duration {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default, Clone, Copy)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessTimes(process: *mut c_void, creation: *mut FileTime, exit: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn GetSystemTimePreciseAsFileTime(now: *mut FileTime);
    }
    let (mut creation, mut exit, mut kernel, mut user, mut now) = Default::default();
    let ok = unsafe {
        GetSystemTimePreciseAsFileTime(&mut now);
        GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel, &mut user)
    };
    let ticks = |t: FileTime| ((t.high as u64) << 32) | t.low as u64;
    if ok == 0 || ticks(now) < ticks(creation) {
        return Duration::ZERO;
    }
    // Counted in hundreds of nanoseconds
    Duration::from_nanos((ticks(now) - ticks(creation)) * 100).min(Duration::from_secs(10))
}

#[cfg(not(windows))]
fn launched_ago() -> Duration {
    Duration::ZERO
}

fn printing() -> bool {
    *PRINTING.get_or_init(|| std::env::args().any(|a| a == "--screenshot"))
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn since_launch() -> Duration {
    LAUNCH.get_or_init(Instant::now).elapsed()
}

/// Says that `what`, which took `took`, is done
pub fn finished(what: &str, took: Duration) {
    if printing() {
        println!("Startup: {what} took {:.0} ms, done {:.0} ms after launch", ms(took), ms(since_launch()));
    }
}

/// Says that something happened, like the window showing, and how long after launch
pub fn moment(what: &str) {
    if printing() {
        println!("Startup: {what} {:.0} ms after launch", ms(since_launch()));
    }
}
