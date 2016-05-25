//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.

#![cfg_attr(feature = "allocator", allocator)]
#![no_std]

#![feature(allocator, const_fn, core_intrinsics, stmt_expr_attributes, drop_types_in_const,
           nonzero)]

#![warn(missing_docs)]

#[cfg(target_os = "redox")]
extern crate system;
#[cfg(not(target_os = "redox"))]
#[macro_use]
extern crate syscall;

mod allocator;
mod block;
mod bookkeeper;
mod ptr;
mod sync;
mod sys;
mod vec;
pub mod fail;

pub use allocator::{free, alloc, realloc, realloc_inplace};

/// Rust allocation symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    alloc(size, align)
}

/// Rust deallocation symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub unsafe extern fn __rust_deallocate(ptr: *mut u8, size: usize, _align: usize) {
    free(ptr, size);
}

/// Rust reallocation symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub unsafe extern fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    realloc(ptr, old_size, size, align)
}

/// Rust reallocation inplace symbol.
#[no_mangle]
#[inline]
#[cfg(feature = "allocator")]
pub unsafe extern fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize, _align: usize) -> usize {
    if realloc_inplace(ptr, old_size, size).is_ok() {
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
