//! The global allocator.
//!
//! This contains primitives for the cross-thread allocator. Furthermore, it provides symbols for
//! allocation, deallocation, and reallocation for Rust.

use core::intrinsics;
use core::ptr::Unique;
use core::sync::atomic;

use bookkeeper::Bookkeeper;
use block::Block;

/// The bookkeeper lock.
///
/// This atomic boolean is false whenever the lock is free.
static mut BOOKKEEPER_LOCK: atomic::AtomicBool = atomic::AtomicBool::new(false);
/// The bookkeeper.
///
/// This is the associated bookkeeper of this allocator.
static mut BOOKKEEPER: Option<Bookkeeper> = None;

/// Unlock the associated mutex.
///
/// This is unsafe, since it will make future use of the acquired bookkeeper reference invalid,
/// until it is reacquired through [the `get_bookkeeper` method](./fn.get_bookkeeper.html).
unsafe fn unlock_bookkeeper() {
    BOOKKEEPER_LOCK.store(false, atomic::Ordering::SeqCst);
}

/// Lock and possibly initialize the bookkeeper.
///
/// Note that the mutex should be unlocked manually, through the [`unlock_bookkeeper`
/// method](./fn.unlock_bookkeeper.html).
// TODO use condvar.
fn get_bookkeeper() -> &'static mut Bookkeeper {
    unsafe {
        // Lock the mutex.
        while BOOKKEEPER_LOCK.load(atomic::Ordering::SeqCst) {}
        BOOKKEEPER_LOCK.store(true, atomic::Ordering::SeqCst);

        if let Some(ref mut x) = BOOKKEEPER {
            x
        } else {
            BOOKKEEPER = Some(Bookkeeper::new());

            BOOKKEEPER.as_mut().unwrap_or_else(|| intrinsics::unreachable())
        }
    }
}

/// Allocate memory.
#[no_mangle]
pub extern fn __rust_allocate(size: usize, align: usize) -> *mut u8 {
    let res = *get_bookkeeper().alloc(size, align);
    unsafe { unlock_bookkeeper() }

    res
}

/// Deallocate memory.
#[no_mangle]
pub extern fn __rust_deallocate(ptr: *mut u8, size: usize, _align: usize) {
    let res = get_bookkeeper().free(Block {
        size: size,
        ptr: unsafe { Unique::new(ptr) },
    });
    unsafe { unlock_bookkeeper() }

    res
}

/// Reallocate memory.
#[no_mangle]
pub extern fn __rust_reallocate(ptr: *mut u8, old_size: usize, size: usize, align: usize) -> *mut u8 {
    let res = *get_bookkeeper().realloc(Block {
        size: old_size,
        ptr: unsafe { Unique::new(ptr) },
    }, size, align);
    unsafe { unlock_bookkeeper() }

    res
}

/// Return the maximal amount of inplace reallocation that can be done.
#[no_mangle]
pub extern fn __rust_reallocate_inplace(_ptr: *mut u8, old_size: usize, _size: usize, _align: usize) -> usize {
    old_size // TODO
}

/// Get the usable size of the some number of bytes of allocated memory.
#[no_mangle]
pub extern fn __rust_usable_size(size: usize, _align: usize) -> usize {
    size
}
