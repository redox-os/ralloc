//! Traits for leakable types.
//!
//! In the context of writing a memory allocator, leaks are never ideal. To avoid these, we have a
//! trait for types which are "leakable".

use prelude::*;

/// Types that have no destructor.
///
/// This trait act as a simple static assertions catching dumb logic errors and memory leaks.
///
/// Since one cannot define mutually exclusive traits, we have this as a temporary hack.
pub trait Leak {}

impl Leak for () {}
impl Leak for Block {}
impl Leak for u8 {}
impl Leak for u16 {}
impl Leak for i32 {}
