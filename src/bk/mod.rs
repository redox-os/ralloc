//! Memory bookkeeping.
//!
//! This module is the core of `ralloc`, it contains efficient structures for storing and
//! organizing memory, as well as finding fitting blocks and so on.
//!
//! It is based around **a variant of skip lists**, which allow for very efficient searching. The
//! primary idea is to keep continuous segments maximal by merging adjacent blocks.
//!
//! Furthermore, every node keeps track of its "children's" (skipped nodes') largest block to
//! allow refinement search guided by that value as well.
//!
//! Many of these algorithms are super complex, so it is important that they're throughoutly
//! commented and documented to make sure the code is readable in the future.
//!
//! I've described some of the optimizations I use [here](http://ticki.github.io/blog/skip-lists-done-right/).
//!
//!     /------------------------------------------\
//!     | Welcome to the dark corner of `ralloc`,  |
//!     | have fun, and may the red ox be with you |
//!     \-------------v----------------------------/
//!                &   &
//!        _________&_&
//!      ~/  \  /  c  -\
//!     / |  /  \   \   \
//!       |    __    \__.
//!       |_!_|  |_!_|

mod lv;
mod node;
mod search;
mod seek;
mod shortcut;
