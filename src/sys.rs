//! System primitives.

#[cfg(not(feature = "unsafe_no_brk_lock"))]
use sync;

mod symbols {
    extern {
        pub fn sched_yield() -> isize;
        pub fn sbrk(diff: isize) -> *mut u8;
    }
}

/// The BRK mutex.
///
/// This is used for avoiding data races in multiple allocator.
#[cfg(not(feature = "unsafe_no_brk_lock"))]
static BRK_MUTEX: sync::Mutex<()> = sync::Mutex::new(());

/// Increment data segment of this process by some, _n_, return a pointer to the new data segment
/// start.
///
/// This uses the system call BRK as backend.
#[inline]
pub fn sbrk(n: isize) -> Result<*mut u8, ()> {
    // Lock the BRK mutex.
    #[cfg(not(feature = "unsafe_no_brk_lock"))]
    let _guard = BRK_MUTEX.lock();

    unsafe {
        let brk = symbols::sbrk(n as isize);
        if brk as usize == !0 {
            Err(())
        } else {
            Ok(brk)
        }
    }
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    assert_eq!(unsafe { symbols::sched_yield() }, 0);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_oom() {
        assert!(sbrk(9999999999999).is_err());
    }

    #[test]
    fn test_overflow() {
        assert!(sbrk(!0).is_err());
        assert!(sbrk(!0 - 2000).is_err());
    }
}
