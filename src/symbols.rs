//! Rust allocation symbols.

// TODO: Remove this, this is a false positive.
#![allow(private_no_mangle_fns)]

use allocator;

/// Rust allocation symbol.
#[linkage = "external"]
#[no_mangle]
#[inline]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    allocator::alloc(size, align)
}

/// Rust deallocation symbol.
#[linkage = "external"]
#[no_mangle]
#[inline]
pub unsafe extern fn __rust_deallocate(ptr: *mut u8, size: usize, _align: usize) {
    allocator::free(ptr, size);
}

/// Rust reallocation symbol.
#[linkage = "external"]
#[no_mangle]
#[inline]
pub unsafe extern fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    allocator::realloc(ptr, old_size, size, align)
}

/// Rust reallocation inplace symbol.
#[linkage = "external"]
#[no_mangle]
#[inline]
pub unsafe extern fn __rust_reallocate_inplace(ptr: *mut u8, old_size: usize, size: usize, _align: usize) -> usize {
    if allocator::realloc_inplace(ptr, old_size, size).is_ok() {
        size
    } else {
        old_size
    }
}

/// Get the usable size of the some number of bytes of allocated memory.
#[linkage = "external"]
#[no_mangle]
#[inline]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    // Yay! It matches exactly.
    size
}
