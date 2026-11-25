//! The only unsafe code of the binary (the library forbids unsafe).
//!
//! Rust replaces a closed file descriptor 0, 1 or 2 with `/dev/null`
//! (read-write) before `main`. After that a really closed stdout cannot be
//! told apart from `1<>/dev/null`. A constructor in `.init_array` runs before
//! that replacement and records whether fd 1 was already closed.

use std::sync::atomic::{AtomicBool, Ordering};

static FD1_WAS_CLOSED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
mod probe {
    use super::{AtomicBool, Ordering, FD1_WAS_CLOSED};

    extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }
    const F_GETFD: i32 = 1;

    extern "C" fn record() {
        // SAFETY: `fcntl(1, F_GETFD)` only reads a flag and changes nothing.
        let closed = unsafe { fcntl(1, F_GETFD) } == -1;
        let flag: &AtomicBool = &FD1_WAS_CLOSED;
        flag.store(closed, Ordering::Relaxed);
    }

    #[used]
    #[link_section = ".init_array"]
    static INIT: extern "C" fn() = record;
}

/// Was fd 1 already closed when the process started? Reliable on Linux only.
pub fn stdout_was_closed_at_start() -> bool {
    FD1_WAS_CLOSED.load(Ordering::Relaxed)
}
