//! Allocator logging.
//!
//! This allows for detailed logging for `ralloc`.

/// Log to the appropriate source.
///
/// The first argument this takes is of the form `pool;cursor`, which is used to print the
/// block pools state. `cursor` is what the operation "revolves around" to give a sense of
/// position.
///
/// If the `;cursor` part is left out, no cursor will be printed.
///
/// The rest of the arguments are just normal formatters.
#[macro_export]
macro_rules! log {
    ($pool:expr, $( $arg:expr ),*) => {
        log!($pool;(), $( $arg ),*);
    };
    ($bk:expr;$cur:expr, $( $arg:expr ),*) => {
        #[cfg(feature = "log")]
        {
            use core::fmt::Write;

            use {write, log};
            use log::internal::IntoCursor;

            // Print the pool state.
            let mut log = write::LogWriter::new();
            let _ = write!(log, "({:2})   {:10?} : ", $bk.id, log::internal::BlockLogger {
                cur: $cur.clone().into_cursor(),
                blocks: &$bk.pool,
            });

            // Print the log message.
            let _ = write!(log, $( $arg ),*);
            let _ = writeln!(log, " (at {}:{})", file!(), line!());
        }
    };
}

/// Top secret place-holding module.
#[macro_use]
#[cfg(feature = "log")]
pub mod internal {
    use prelude::*;

    use core::fmt;

    use core::cell::Cell;
    use core::ops::Range;

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
        // TODO use an iterator instead.
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
        fn at(&self, _: &mut fmt::Formatter, _: usize) -> fmt::Result { Ok(()) }

        fn after(&self, _: &mut fmt::Formatter) -> fmt::Result { Ok(()) }
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

        fn after(&self, _: &mut fmt::Formatter) -> fmt::Result { Ok(()) }
    }

    impl IntoCursor for Range<usize> {
        type Cursor = RangeCursor;

        fn into_cursor(self) -> RangeCursor {
            RangeCursor {
                range: self,
            }
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
            // TODO handle alignment etc.

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
}
