//! Allocator logging.
//!
//! This allows for detailed logging for `ralloc`.

/// Log to the appropriate source.
///
/// The first argument defines the log level, the rest of the arguments are just `write!`-like
/// formatters.
#[macro_export]
macro_rules! log {
    (INTERNAL, $( $x:tt )*) => {
        log!(@["INTERNAL: ", 1], $( $x )*);
    };
    (DEBUG, $( $x:tt )*) => {
        log!(@["DEBUG:    ", 2], $( $x )*);
    };
    (CALL, $( $x:tt )*) => {
        log!(@["CALL:     ", 3], $( $x )*);
    };
    (NOTE, $( $x:tt )*) => {
        log!(@["NOTE:     ", 5], $( $x )*);
    };
    (WARNING, $( $x:tt )*) => {
        log!(@["WARNING:  ", 5], $( $x )*);
    };
    (ERROR, $( $x:tt )*) => {
        log!(@["ERROR:    ", 6], $( $x )*);
    };
    (@[$kind:expr, $lv:expr], $( $arg:expr ),*) => {
        #[cfg(feature = "log")]
        {
            use core::fmt::Write;

            use log::internal::{LogWriter, level};

            // Set the level.
            if level($lv) {
                // Print the pool state.
                let mut log = LogWriter::new();
                // Print the log message.
                let _ = write!(log, $kind);
                let _ = write!(log, $( $arg ),*);
                let _ = writeln!(log, " (at {}:{})", file!(), line!());
            }
        }
    };
}

/// Log with bookkeeper data to the appropriate source.
///
/// The first argument this takes is of the form `pool;cursor`, which is used to print the
/// block pools state. `cursor` is what the operation "revolves around" to give a sense of
/// position.
///
/// If the `;cursor` part is left out, no cursor will be printed.
///
/// The rest of the arguments are just normal formatters.
///
/// This logs to level 2.
#[macro_export]
macro_rules! bk_log {
    ($pool:expr, $( $arg:expr ),*) => {
        bk_log!($pool;(), $( $arg ),*);
    };
    ($bk:expr;$cur:expr, $( $arg:expr ),*) => {
        #[cfg(feature = "log")]
        {
            use log::internal::{IntoCursor, BlockLogger};

            log!(INTERNAL, "({:2}) {:10?} : {}", $bk.id, BlockLogger {
                cur: $cur.clone().into_cursor(),
                blocks: &$bk.pool,
            }, format_args!($( $arg ),*));
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
    ($left:expr, $right:expr) => {{
        // We evaluate _once_.
        let left = &$left;
        let right = &$right;

        assert!(left == right, "(left: '{:?}', right: '{:?}')", left, right)
    }};
}

/// Top-secret module.
#[cfg(feature = "log")]
pub mod internal {
    use prelude::*;

    use core::cell::Cell;
    use core::fmt;
    use core::ops::Range;

    use shim::config;

    use sync;

    /// The log lock.
    ///
    /// This lock is used to avoid bungling and intertwining the log.
    #[cfg(not(feature = "no_log_lock"))]
    pub static LOG_LOCK: Mutex<()> = Mutex::new(());

    /// A log writer.
    ///
    /// This writes to the shim logger.
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
            if config::log(s) == !0 {
                Err(fmt::Error)
            } else {
                Ok(())
            }
        }
    }

    /// A "cursor".
    ///
    /// Cursors represents a block or an interval in the log output. This trait is implemented for
    /// various types that can represent a cursor.
    pub trait Cursor {
        /// Iteration at n.
        ///
        /// This is called in the logging loop. The cursor should then write, what it needs, to the
        /// formatter if the underlying condition is true.
        ///
        /// For example, a plain position cursor will write `"|"` when `n == self.pos`.
        // TODO: Use an iterator instead.
        fn at(&self, f: &mut fmt::Formatter, n: usize) -> fmt::Result;

        /// The after hook.
        ///
        /// This is runned when the loop is over. The aim is to e.g. catch up if the cursor wasn't
        /// printed (i.e. is out of range).
        fn after(&self, f: &mut fmt::Formatter) -> fmt::Result;
    }

    /// Types that can be converted into a cursor.
    pub trait IntoCursor {
        /// The end result.
        type Cursor: Cursor;

        /// Convert this value into its equivalent cursor.
        fn into_cursor(self) -> Self::Cursor;
    }

    /// A single-point cursor.
    pub struct UniCursor {
        /// The position where this cursor will be placed.
        pos: usize,
        /// Is this cursor printed?
        ///
        /// This is used for the after hook.
        is_printed: Cell<bool>,
    }

    impl Cursor for UniCursor {
        fn at(&self, f: &mut fmt::Formatter, n: usize) -> fmt::Result {
            if self.pos == n {
                self.is_printed.set(true);
                write!(f, "|")?;
            }

            Ok(())
        }

        fn after(&self, f: &mut fmt::Formatter) -> fmt::Result {
            if !self.is_printed.get() {
                write!(f, "…|")?;
            }

            Ok(())
        }
    }

    impl IntoCursor for usize {
        type Cursor = UniCursor;

        fn into_cursor(self) -> UniCursor {
            UniCursor {
                pos: self,
                is_printed: Cell::new(false),
            }
        }
    }

    impl Cursor for () {
        fn at(&self, _: &mut fmt::Formatter, _: usize) -> fmt::Result {
            Ok(())
        }

        fn after(&self, _: &mut fmt::Formatter) -> fmt::Result {
            Ok(())
        }
    }

    impl IntoCursor for () {
        type Cursor = ();

        fn into_cursor(self) -> () {
            ()
        }
    }

    /// A interval/range cursor.
    ///
    /// The start of the range is marked by `[` and the end by `]`.
    pub struct RangeCursor {
        /// The range of this cursor.
        range: Range<usize>,
    }

    impl Cursor for RangeCursor {
        fn at(&self, f: &mut fmt::Formatter, n: usize) -> fmt::Result {
            if self.range.start == n {
                write!(f, "[")?;
            } else if self.range.end == n {
                write!(f, "]")?;
            }

            Ok(())
        }

        fn after(&self, _: &mut fmt::Formatter) -> fmt::Result {
            Ok(())
        }
    }

    impl IntoCursor for Range<usize> {
        type Cursor = RangeCursor;

        fn into_cursor(self) -> RangeCursor {
            RangeCursor { range: self }
        }
    }

    /// A "block logger".
    ///
    /// This intend to show the structure of a block pool. The syntax used is like:
    ///
    /// ```
    /// xxx__|xx_
    /// ```
    ///
    /// where `x` denotes an non-empty block. `_` denotes an empty block, with `|` representing the
    /// cursor.
    pub struct BlockLogger<'a, T> {
        /// The cursor.
        ///
        /// This is where the `|` will be printed.
        pub cur: T,
        /// The blocks.
        pub blocks: &'a [Block],
    }

    impl<'a, T: Cursor> fmt::Debug for BlockLogger<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            // TODO: Handle alignment etc.

            for (n, i) in self.blocks.iter().enumerate() {
                self.cur.at(f, n)?;

                if i.is_empty() {
                    // Empty block.
                    write!(f, "_")?;
                } else {
                    // Non-empty block.
                    write!(f, "x")?;
                }
            }

            self.cur.after(f)?;

            Ok(())
        }
    }

    /// Check if this log level is enabled.
    #[inline]
    pub fn level(lv: u8) -> bool {
        lv >= config::MIN_LOG_LEVEL
    }
}
