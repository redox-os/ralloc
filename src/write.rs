//! Direct libc-based write for internal debugging.
//!
//! This will replace the assertion macros to avoid deadlocks in panics, by utilizing a
//! non-allocating writing primitive.

use prelude::*;

use core::fmt;

use sys;

/// The line lock.
///
/// This lock is used to avoid bungling and intertwining lines.
pub static LINE_LOCK: Mutex<()> = Mutex::new(());

/// A log writer.
///
/// This writes to  `sys::log`.
pub struct Writer;

impl Writer {
    /// Standard error output.
    pub fn new() -> Writer {
        Writer
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if sys::log(s).is_err() {
            Err(fmt::Error)
        } else { Ok(()) }
    }
}

/// Make a runtime assertion.
///
/// The only way it differs from the one provided by `libcore` is the panicking strategy, which
/// allows for aborting, non-allocating panics when running the tests.
#[macro_export]
macro_rules! assert {
    ($e:expr) => {
        assert!($e, "No description.");
    };
    ($e:expr, $( $arg:expr ),*) => {{
        use write;

        use core::intrinsics;
        use core::fmt::Write;

        if !$e {
            // To avoid cluttering the lines, we acquire a lock.
            let _lock = write::LINE_LOCK.lock();

            let mut log = write::Writer::new();
            let _ = write!(log, "assertion failed at {}:{}: `{}` - ", file!(),
                           line!(), stringify!($e));
            let _ = writeln!(log, $( $arg ),*);

            #[allow(unused_unsafe)]
            unsafe { intrinsics::abort() }
        }
    }}
}

/// Make a runtime assertion in debug mode.
///
/// The only way it differs from the one provided by `libcore` is the panicking strategy, which
/// allows for aborting, non-allocating panics when running the tests.
#[macro_export]
macro_rules! debug_assert {
    // We force the programmer to provide explanation of their assertion.
    ($first:expr, $( $arg:tt )*) => {{
        if cfg!(debug_assertions) {
            assert!($first, $( $arg )*);
        }
    }}
}

/// Make a runtime equality assertion in debug mode.
///
/// The only way it differs from the one provided by `libcore` is the panicking strategy, which
/// allows for aborting, non-allocating panics when running the tests.
#[macro_export]
macro_rules! assert_eq {
    ($left:expr, $right:expr) => ({
        // We evaluate _once_.
        let left = &$left;
        let right = &$right;

        assert!(left == right, "(left: `{:?}`, right: `{:?}`)", left, right)
    })
}
