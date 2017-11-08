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

#![feature(alloc, allocator_api, const_fn, core_intrinsics, stmt_expr_attributes, drop_types_in_const,
           nonzero, optin_builtin_traits, type_ascription, thread_local, linkage,
           try_from, const_unsafe_cell_new, const_atomic_bool_new, const_nonzero_new,
           const_atomic_ptr_new)]
#![warn(missing_docs, cast_precision_loss, cast_sign_loss, cast_possible_wrap,
        cast_possible_truncation, filter_map, if_not_else, items_after_statements,
        invalid_upcast_comparisons, mutex_integer, nonminimal_bool, shadow_same, shadow_unrelated,
        single_match_else, string_add, string_add_assign, wrong_pub_self_convention)]

extern crate alloc;
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

use alloc::heap::{Alloc, AllocErr, Layout, CannotReallocInPlace};

pub use allocator::{alloc, free, realloc, realloc_inplace};
pub use brk::sbrk;
pub use fail::set_oom_handler;
#[cfg(feature = "tls")]
pub use fail::set_thread_oom_handler;

pub struct Allocator;

unsafe impl<'a> Alloc for &'a Allocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<*mut u8, AllocErr> {
        Ok(allocator::alloc(layout.size(), layout.align()))
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        allocator::free(ptr, layout.size());
    }

    unsafe fn realloc(&mut self, ptr: *mut u8, layout: Layout, new_layout: Layout) -> Result<*mut u8, AllocErr> {
        Ok(allocator::realloc(ptr, layout.size(), new_layout.size(), new_layout.align()))
    }

    unsafe fn grow_in_place(&mut self, ptr: *mut u8, layout: Layout, new_layout: Layout) -> Result<(), CannotReallocInPlace> {
        if allocator::realloc_inplace(ptr, layout.size(), new_layout.size()).is_ok() {
            Ok(())
        } else {
            Err(CannotReallocInPlace)
        }
    }

    unsafe fn shrink_in_place(&mut self, ptr: *mut u8, layout: Layout, new_layout: Layout) -> Result<(), CannotReallocInPlace> {
        if allocator::realloc_inplace(ptr, layout.size(), new_layout.size()).is_ok() {
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
