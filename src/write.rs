//! Direct shim-based write for internal debugging.
//!
//! This will replace the assertion macros to avoid deadlocks in panics, by utilizing a
//! non-allocating writing primitive.

use prelude::*;

use core::fmt;

use {sys, sync};

/// The log lock.
///
/// This lock is used to avoid bungling and intertwining the log.
#[cfg(not(feature = "no_log_lock"))]
pub static LOG_LOCK: Mutex<()> = Mutex::new(());

/// A log writer.
///
/// This writes to  `sys::log`.
pub struct LogWriter {
    /// The inner lock.
    #[cfg(not(feature = "no_log_lock"))]
    _lock: sync::MutexGuard<'static, ()>,
}

impl LogWriter {
    /// Standard error output.
    pub fn new() -> LogWriter {
        #[cfg(feature = "no_log_lock")]
        {
            LogWriter {}
        }

        #[cfg(not(feature = "no_log_lock"))]
        LogWriter {
            _lock: LOG_LOCK.lock(),
        }
    }
}

impl fmt::Write for LogWriter {
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
            let mut log = write::LogWriter::new();
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
