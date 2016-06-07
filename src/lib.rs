//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.

#![cfg_attr(feature = "allocator", allocator)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

#![no_std]

#![feature(allocator, const_fn, core_intrinsics, stmt_expr_attributes, drop_types_in_const,
           nonzero, optin_builtin_traits, type_ascription)]
#![warn(missing_docs)]

#[cfg(feature = "internals")]
#[macro_use]
mod debug;

mod block;
mod bookkeeper;
mod leak;
mod prelude;
mod ptr;
mod sync;
mod sys;
mod vec;
mod allocator;

pub use allocator::{lock, Allocator};
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
