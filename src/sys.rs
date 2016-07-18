//! System primitives.

extern crate ralloc_shim;

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

    let brk = ralloc_shim::sbrk(n);
    if brk as usize == !0 {
        Err(())
    } else {
        Ok(brk)
    }
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    assert_eq!(unsafe { ralloc_shim::sched_yield() }, 0);
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
