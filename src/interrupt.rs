//! SIGINT (Ctrl-C) ends the run with exit 130 and one line on standard error.
//!
//! This is unsafe code of the binary, like `stdout_probe` (the library forbids
//! unsafe). The handler only touches atomics and calls `write` and `_exit`,
//! which are safe to call from a signal handler. The report is written in one
//! step at the very end; while that step runs SIGINT is held back, so a
//! report or a file is never left half written. Outside that step nothing has
//! been written yet, so the handler can end the process at once.

use std::sync::atomic::{AtomicBool, Ordering};

/// Exit code of a run ended by SIGINT: 128 + 2, as a shell reports it.
#[cfg(unix)]
pub const EXIT_SIGINT: u8 = 130;

#[cfg(unix)]
const MESSAGE: &[u8] = b"s3-inventory-bill: interrupted by SIGINT\n";

static WRITING: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
mod sys {
    use super::{EXIT_SIGINT, MESSAGE, PENDING, WRITING};
    use std::sync::atomic::Ordering;

    extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
        fn write(fd: i32, buf: *const std::ffi::c_void, count: usize) -> isize;
        fn _exit(status: i32) -> !;
    }
    const SIGINT: i32 = 2;
    const SIG_ERR: usize = usize::MAX;

    extern "C" fn on_sigint(_: i32) {
        if WRITING.load(Ordering::SeqCst) {
            PENDING.store(true, Ordering::SeqCst);
            return;
        }
        end();
    }

    pub fn end() -> ! {
        // SAFETY: `write` and `_exit` are async-signal-safe; MESSAGE is a
        // static buffer of the length given.
        unsafe {
            let _ = write(2, MESSAGE.as_ptr().cast(), MESSAGE.len());
            _exit(i32::from(EXIT_SIGINT))
        }
    }

    pub fn install() -> bool {
        let handler: extern "C" fn(i32) = on_sigint;
        // SAFETY: `on_sigint` has the C signature a handler needs and only
        // does async-signal-safe work.
        unsafe { signal(SIGINT, handler as usize) != SIG_ERR }
    }
}

/// Install the handler. Where there is none (not Unix) a Ctrl-C ends the
/// process the way the platform does.
pub fn install() {
    #[cfg(unix)]
    {
        let _ = sys::install();
    }
}

/// Has a SIGINT arrived while a guarded step was running?
pub fn pending() -> bool {
    PENDING.load(Ordering::SeqCst)
}

/// Run `step` with SIGINT held back. A SIGINT that came meanwhile ends the
/// process with exit 130 after the step is complete.
pub fn guarded<T>(step: impl FnOnce() -> T) -> T {
    WRITING.store(true, Ordering::SeqCst);
    let value = step();
    WRITING.store(false, Ordering::SeqCst);
    if PENDING.load(Ordering::SeqCst) {
        #[cfg(unix)]
        sys::end();
    }
    value
}
