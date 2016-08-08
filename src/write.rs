//! Direct libc-based write for internal debugging.
//!
//! This will replace the assertion macros to avoid deadlocks in panics, by utilizing a
//! non-allocating writing primitive.

use prelude::*;

use core::fmt;

extern {
    /// Write a buffer to a file descriptor.
    fn write(fd: i32, buff: *const u8, size: usize) -> isize;
}

/// The line lock.
///
/// This lock is used to avoid bungling and intertwining lines.
pub static LINE_LOCK: Mutex<()> = Mutex::new(());

/// A direct writer.
///
/// This writes directly to some file descriptor through the `write` symbol.
pub struct Writer {
    /// The file descriptor.
    fd: i32,
}

impl Writer {
    /// Standard error output.
    pub fn stderr() -> Writer {
        Writer {
            fd: 2,
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if unsafe { write(self.fd, s.as_ptr(), s.len()) } == !0 {
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

            let _ = write!(write::Writer::stderr(), "assertion failed at {}:{}: `{}` - ", file!(),
                           line!(), stringify!($e));
            let _ = writeln!(write::Writer::stderr(), $( $arg ),*);

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
    ($( $arg:tt )*) => {{
        if cfg!(debug_assertions) {
            assert!($( $arg )*);
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
