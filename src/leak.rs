//! Traits for leakable types.
//!
//! In the context of writing a memory allocator, leaks are never ideal. To avoid these, we have a
//! trait for types which are "leakable".

use prelude::*;

/// Types that have no destructor.
///
/// This trait holds the invariant that our type carries no destructor.
///
/// Since one cannot define mutually exclusive traits, we have this as a temporary hack.
pub unsafe trait Leak {}

unsafe impl Leak for Block {}
unsafe impl<T: Copy> Leak for T {}
