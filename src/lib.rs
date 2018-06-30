//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.
//!
//! # Ralloc seems to reimplement everything. Why?
//!
//! Memory allocators cannot depend on libraries or primitives, which allocates. This is a
//! relatively strong condition, which means that you are forced to rewrite primitives and make
//! sure no allocation ever happens.

#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]
#![no_std]
#![feature(
    allocator_api, const_fn, core_intrinsics, stmt_expr_attributes, optin_builtin_traits,
    type_ascription, thread_local, linkage, try_from, const_unsafe_cell_new, const_atomic_bool_new,
    const_nonzero_new, const_atomic_ptr_new
)]
#![warn(missing_docs)]

extern crate ralloc_shim as shim;

#[macro_use]
mod log;
#[macro_use]
#[cfg(feature = "tls")]
mod tls;

#[macro_use]
mod unborrow;

mod allocator;
mod block;
mod bookkeeper;
mod brk;
mod cell;
mod fail;
mod lazy_init;
mod leak;
mod prelude;
mod ptr;
mod sync;
mod vec;

use core::alloc::GlobalAlloc;
use core::alloc::{Alloc, AllocErr, CannotReallocInPlace, Layout};
use core::ptr::NonNull;

pub use allocator::{alloc, free, realloc, realloc_inplace};
pub use brk::sbrk;
pub use fail::set_oom_handler;
#[cfg(feature = "tls")]
pub use fail::set_thread_oom_handler;

/// The rallocator
pub struct Allocator;

unsafe impl<'a> Alloc for &'a Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {
        let ptr = allocator::alloc(layout.size(), layout.align());
        if ptr.is_null() {
            Err(AllocErr)
        } else {
            Ok(NonNull::new_unchecked(ptr))
        }
    }

    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        allocator::free(ptr.as_ptr(), layout.size());
    }

    unsafe fn realloc(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<u8>, AllocErr> {
        let ptr = allocator::realloc(ptr.as_ptr(), layout.size(), new_size, layout.align());
        if ptr.is_null() {
            Err(AllocErr)
        } else {
            Ok(NonNull::new_unchecked(ptr))
        }
    }

    unsafe fn grow_in_place(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<(), CannotReallocInPlace> {
        if allocator::realloc_inplace(ptr.as_ptr(), layout.size(), new_size).is_ok() {
            Ok(())
        } else {
            Err(CannotReallocInPlace)
        }
    }

    unsafe fn shrink_in_place(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<(), CannotReallocInPlace> {
        if allocator::realloc_inplace(ptr.as_ptr(), layout.size(), new_size).is_ok() {
            Ok(())
        } else {
            Err(CannotReallocInPlace)
        }
    }

    fn usable_size(&self, layout: &Layout) -> (usize, usize) {
        // Yay! It matches exactly.
        (layout.size(), layout.size())
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocator::alloc(layout.size(), layout.align())
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        allocator::free(ptr, layout.size());
    }
}
