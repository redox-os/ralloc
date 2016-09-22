//! Allocator logging.
//!
//! This allows for detailed logging for `ralloc`.

/// Log to the appropriate source.
///
/// The first argument defines the log level, the rest of the arguments are just `write!`-like
/// formatters.
///
/// # Log levels
///
/// 1. `INTERNAL`: For things that are used in debugging of `ralloc`, but rarely of relevance or
///    usage when debugging.
/// 2. `DEBUG`: For messages which helps debugging `ralloc` (not the program, but `ralloc`).
/// 3. `CALL`: For calls into `ralloc`. This is only inteded for use when entering `ralloc`.
/// 4. `NOTE`: Information which is not necessarily a red flag, but interesting for the user.
/// 5. `WARNING`: For indicating that something might have went wrong.
/// 6. `ERROR`: For error messages, both fatal and non-fatal ones.
#[macro_export]
macro_rules! log {
    (INTERNAL, $( $x:tt )*) => {
        log!(@["INTERNAL", 1], $( $x )*);
    };
    (DEBUG, $( $x:tt )*) => {
        log!(@["DEBUG", 2], $( $x )*);
    };
    (CALL, $( $x:tt )*) => {
        log!(@["CALL", 3], $( $x )*);
    };
    (NOTE, $( $x:tt )*) => {
        log!(@["NOTE", 5], $( $x )*);
    };
    (WARNING, $( $x:tt )*) => {
        log!(@["WARNING", 5], $( $x )*);
    };
    (ERROR, $( $x:tt )*) => {
        log!(@["ERROR", 6], $( $x )*);
    };
    (@[$kind:expr, $lv:expr], $( $arg:expr ),*) => {
        // Sneks gunna snek
        //
        //     ()
        //   ()  ()
        //  ()  ()     ()
        //   ()  ()  ()
        //  ()     ()
        //  \/
        // [**]
        //  |
        //  ^

        #[cfg(feature = "log")]
        {
            use core::fmt::Write;

            use log::__internal::{LogWriter, level};

            shim::log::write($lv, $kind, format_args!($( $arg ),*), file!(), line!());
        }
    };
}

/// Make a runtime assertion.
///
/// The only way it differs from the one provided by `libcore` is the panicking strategy, which
/// allows for aborting, non-allocating panics when running the tests.
#[macro_export]
#[cfg(feature = "write")]
macro_rules! assert {
    ($e:expr) => {
        assert!($e, "No description.");
    };
    ($e:expr, $( $arg:expr ),*) => {{
        use core::intrinsics;

        if !$e {
            log!(ERROR, $( $arg ),*);

            #[allow(unused_unsafe)]
            unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // Right now there is no safe interface exposed for this, but it is safe no matter
                // what.
                intrinsics::abort();
            }
        }
    }}
}

/// Make a runtime assertion in debug mode.
///
/// The only way it differs from the one provided by `libcore` is the panicking strategy, which
/// allows for aborting, non-allocating panics when running the tests.
#[cfg(feature = "write")]
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
#[cfg(feature = "write")]
#[macro_export]
macro_rules! assert_eq {
    ($left:expr, $right:expr) => ({
        // We evaluate _once_.
        let left = &$left;
        let right = &$right;

        assert!(left == right, "(left: '{:?}', right: '{:?}')", left, right)
    })
}
