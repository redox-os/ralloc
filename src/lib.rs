//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.

#![cfg_attr(feature = "allocator", allocator)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

#![no_std]

#![feature(allocator, const_fn, core_intrinsics, stmt_expr_attributes, drop_types_in_const,
           nonzero, optin_builtin_traits, type_ascription, question_mark, try_from)]
#![warn(missing_docs, cast_precision_loss, cast_sign_loss, cast_possible_wrap,
        cast_possible_truncation, filter_map, if_not_else, items_after_statements,
        invalid_upcast_comparisons, mutex_integer, nonminimal_bool, shadow_same, shadow_unrelated,
        single_match_else, string_add, string_add_assign, wrong_pub_self_convention)]

#[cfg(feature = "libc_write")]
#[macro_use]
mod write;
#[macro_use]
mod log;

mod allocator;
mod block;
mod bookkeeper;
mod fail;
mod leak;
mod prelude;
mod ptr;
mod sync;
mod sys;
mod vec;

pub use allocator::{lock, Allocator};
pub use fail::set_oom_handler;
pub use sys::sbrk;

/// Rust allocation symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    lock().alloc(size, align)
}

/// Rust deallocation symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub unsafe extern fn __rust_deallocate(ptr: *mut u8, size: usize, _align: usize) {
    lock().free(ptr, size);
}

/// Rust reallocation symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub unsafe extern fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    lock().realloc(ptr, old_size, size, align)
}

/// Rust reallocation inplace symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub unsafe extern fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize, _align: usize) -> usize {
    if lock().realloc_inplace(ptr, old_size, size).is_ok() {
        size
    } else {
        old_size
    }
}

/// Get the usable size of the some number of bytes of allocated memory.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    // Yay! It matches exactly.
    size
}
