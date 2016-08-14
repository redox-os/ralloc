//! System primitives.
//!
//! This mostly wraps the `ralloc_shim` crate but provides some additional error handling.

extern crate ralloc_shim as shim;

use core::mem;

pub use self::shim::default_oom_handler;

/// Set the program break.
///
/// On success, the new program break is returned. On failure, the old program break is returned.
///
/// # Safety
///
/// This is due to being able to invalidate safe addresses as well as breaking invariants for the
/// [`brk`](../brk).
#[inline]
pub unsafe fn brk(ptr: *const u8) -> *const u8 {
    shim::brk(ptr)
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    assert_eq!(shim::sched_yield(), 0);
}

/// Register a thread destructor.
///
/// This will add a thread destructor to _the current thread_, which will be executed when the
/// thread exits.
///
/// The argument to the destructor is a pointer to the so-called "load", which is the data
/// shipped with the destructor.
// TODO: I haven't figured out a safe general solution yet. Libstd relies on devirtualization,
// which, when missed, can make it quite expensive.
pub fn register_thread_destructor<T>(load: *mut T, dtor: extern fn(*mut T)) -> Result<(), ()> {
    // Check if thread dtors are supported.
    if shim::thread_destructor::is_supported() {
        unsafe {
            // This is safe due to sharing memory layout.
            shim::thread_destructor::register(load as *mut u8, mem::transmute(dtor));
        }

        Ok(())
    } else {
        Err(())
    }
}

/// Write text to the log.
///
/// The log target is defined by the `shim` crate.
// TODO: Find a better way to silence the warning than this attribute.
#[allow(dead_code)]
pub fn log(s: &str) -> Result<(), ()> {
    if shim::log(s) == !0 { Err(()) } else { Ok(()) }
}

/// Tell the debugger that this segment is free.
///
/// If the `debugger` feature is disabled, this is a NOOP.
#[inline(always)]
pub fn mark_free(_ptr: *const u8, _size: usize) {
    #[cfg(feature = "debugger")]
    shim::debug::mark_free(_ptr, _size);
}

/// Tell the debugger that this segment is unaccessible.
///
/// If the `debugger` feature is disabled, this is a NOOP.
#[inline(always)]
pub fn mark_uninitialized(_ptr: *const u8, _size: usize) {
    #[cfg(feature = "debugger")]
    shim::debug::mark_free(_ptr, _size);
}
