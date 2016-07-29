//! System primitives.

extern crate ralloc_shim as shim;

#[cfg(not(feature = "unsafe_no_brk_lock"))]
use sync;

/// The BRK mutex.
///
/// This is used for avoiding data races in multiple allocator.
#[cfg(not(feature = "unsafe_no_brk_lock"))]
static BRK_MUTEX: sync::Mutex<()> = sync::Mutex::new(());

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
        Ok(brk)
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
// TODO I haven't figured out a safe general solution yet. Libstd relies on devirtualization,
// which, when missed, can make it quite expensive.
pub fn register_thread_destructor<T>(primitive: *mut T, dtor: fn(*mut T)) -> Result<(), ()> {
    if shim::thread_destructor::is_supported() {
        shim::thread_destructor::register(primitive, dtor);
        Ok(())
    } else {
        Err(())
    }
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
