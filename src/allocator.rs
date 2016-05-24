//! The global allocator.
//!
//! This contains primitives for the cross-thread allocator.
use block::Block;
use bookkeeper::Bookkeeper;
use ptr::Pointer;
use sync;

/// The bookkeeper.
///
/// This is the associated bookkeeper of this allocator.
static BOOKKEEPER: sync::Mutex<Bookkeeper> = sync::Mutex::new(Bookkeeper::new());

/// Allocate a block of memory.
pub fn alloc(size: usize, align: usize) -> *mut u8 {
    *BOOKKEEPER.lock().alloc(size, align).into_ptr()
}

/// Free a buffer.
///
/// Note that this do not have to be a buffer allocated through ralloc. The only requirement is
/// that it is not used after the free.
pub unsafe fn free(ptr: *mut u8, size: usize) {
    // Lock the bookkeeper, and do a `free`.
    BOOKKEEPER.lock().free(Block::from_raw_parts(Pointer::new(ptr), size));
}

/// Reallocate memory.
///
/// Reallocate the buffer starting at `ptr` with size `old_size`, to a buffer starting at the
/// returned pointer with size `size`.
pub unsafe fn realloc(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    // Lock the bookkeeper, and do a `realloc`.
    *BOOKKEEPER.lock().realloc(
        Block::from_raw_parts(Pointer::new(ptr), old_size),
        size,
        align
    ).into_ptr()
}

/// Try to reallocate the buffer _inplace_.
///
/// In case of success, return the new buffer's size. On failure, return the old size.
///
/// This can be used to shrink (truncate) a buffer as well.
pub unsafe fn realloc_inplace(ptr: *mut u8, old_size: usize, size: usize) -> Result<(), ()> {
    // Lock the bookkeeper, and do a `realloc_inplace`.
    if BOOKKEEPER.lock().realloc_inplace(
        Block::from_raw_parts(Pointer::new(ptr), old_size),
        size
    ).is_ok() {
        Ok(())
    } else {
        Err(())
    }
}
