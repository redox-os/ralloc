//! Traits for leakable types.
//!
//! In the context of writing a memory allocator, leaks are never ideal. To avoid these, we have a
//! trait for types which are "leakable".

use prelude::*;

/// Types that have no (or a diverging) destructor.
///
/// This trait holds the invariant that our type is one of the following:
///
/// 1. carries a diverging destructor.
/// 2. carries a destructor which diverges if a (effectless) condition is true.
/// 3. carries no destructor.
pub unsafe trait Leak {}

unsafe impl Leak for Block {}
unsafe impl<T> Leak for Jar<T> {}
unsafe impl<T> Leak for Uninit<T> {}
unsafe impl<T> Leak for T where T: Copy {}
