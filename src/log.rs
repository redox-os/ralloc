//! Allocator logging.
//!
//! This allows for detailed logging for `ralloc`.

/// NO-OP.
#[macro_export]
#[cfg(not(feature = "log"))]
macro_rules! log {
    ($( $arg:tt )*) => {};
}

/// Top secret place-holding module.
#[cfg(feature = "log")]
#[macro_use]
pub mod internal {
    use prelude::*;

    use core::fmt;

    /// Log to the appropriate source.
    ///
    /// The first argument this takes is of the form `pool;number`, which is used to print the
    /// block pools state. `number` is what the operation "revolves around" to give a sense of
    /// position.
    ///
    /// The rest of the arguments are just normal formatters.
    #[macro_export]
    macro_rules! log {
        ($pool:expr;$n:expr, $( $arg:expr ),*) => {{
            use {write, log};

            use core::fmt::Write;

            // Print the pool state.
            let mut stderr = write::Writer::stderr();
            let _ = write!(stderr, "{:10?} : ", log::internal::BlockLogger {
                cur: $n,
                blocks: &$pool,
            });

            // Print the log message.
            let _ = write!(stderr, $( $arg ),*);
            let _ = writeln!(stderr, " (at {}:{})", file!(), line!());
        }};
    }

    /// A "block logger".
    ///
    /// This intend to show the structure of a block pool. The syntax used is like:
    ///
    /// ```
    /// xxx__|xx_
    /// ```
    ///
    /// where `x` denotes an non-empty block. `_` denotes an empty block, and `|` is placed on the
    /// "current block".
    pub struct BlockLogger<'a> {
        /// The cursor.
        ///
        /// This is where the `|` will be printed.
        pub cur: usize,
        /// The blocks.
        pub blocks: &'a [Block],
    }

    impl<'a> fmt::Debug for BlockLogger<'a> {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            // TODO handle alignment etc.

            let mut cursor_set = false;

            for (n, i) in self.blocks.iter().enumerate() {
                if n == self.cur {
                    // Write the cursor.
                    write!(f, "|")?;
                    cursor_set = true;
                }

                if i.is_empty() {
                    // Empty block.
                    write!(f, "_")?;
                } else {
                    // Non-empty block.
                    write!(f, "x")?;
                }
            }

            if !cursor_set {
                // The cursor isn't set yet, so we place it in the end.
                write!(f, "|")?;
            }

            Ok(())
        }
    }
}
