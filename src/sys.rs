//! System primitives.

extern crate ralloc_shim as shim;

use prelude::*;

use core::mem;

/// The BRK mutex.
///
/// This is used for avoiding data races in multiple allocator.
static BRK_MUTEX: Mutex<()> = Mutex::new(());

/// Increment data segment of this process by some, _n_, return a pointer to the new data segment
/// start.
///
/// This uses the system call BRK as backend.
///
/// # Safety
///
/// This is safe unless you have negative or overflowing `n`.
#[inline]
pub unsafe fn sbrk(n: isize) -> Result<*mut u8, ()> {
    // Lock the BRK mutex.
    #[cfg(not(feature = "unsafe_no_brk_lock"))]
    let _guard = BRK_MUTEX.lock();

    let brk = shim::sbrk(n);
    if brk as usize == !0 {
        Err(())
    } else {
        Ok(brk as *mut u8)
    }
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    assert_eq!(unsafe { shim::sched_yield() }, 0);
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
#[cfg(feature = "tls")]
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
    if shim::log(s) == -1 { Err(()) } else { Ok(()) }
}

/// Tell the debugger that this segment is free.
///
/// If the `debugger` feature is disabled, this is a NOOP.
pub fn mark_free(_ptr: *const u8, _size: usize) {
    #[cfg(feature = "debugger")]
    shim::debug::mark_free(_ptr as *const _, _size);
}

/// Tell the debugger that this segment is unaccessible.
///
/// If the `debugger` feature is disabled, this is a NOOP.
pub fn mark_uninitialized(_ptr: *const u8, _size: usize) {
    #[cfg(feature = "debugger")]
    shim::debug::mark_free(_ptr as *const _, _size);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_oom() {
        unsafe {
            assert!(sbrk(9999999999999).is_err());
        }
    }
}
