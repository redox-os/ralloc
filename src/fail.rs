//! General error handling.

use prelude::*;

use core::sync::atomic::{self, AtomicPtr};
use core::{mem, intrinsics};

use tls;

/// The global OOM handler.
static OOM_HANDLER: AtomicPtr<()> = AtomicPtr::new(default_oom_handler as *mut ());
tls! {
    /// The thread-local OOM handler.
    static THREAD_OOM_HANDLER: MoveCell<Option<fn() -> !>> = MoveCell::new(None);
}

/// The default OOM handler.
///
/// This will simply abort the process.
#[cold]
fn default_oom_handler() -> ! {
    unsafe {
        intrinsics::abort();
    }
}

/// Call the OOM handler.
///
/// This is used one out-of-memory errors, and will never return. Usually, it simply consists
/// of aborting the process.
///
/// # An important note
///
/// This is for OOM-conditions, not malformed or too big allocations, but when the system is unable
/// to gather memory for the allocation (SBRK fails).
///
/// The rule of thumb is that this should be called, if and only if unwinding (which allocates)
/// will hit the same error.
pub fn oom() -> ! {
    if let Some(handler) = THREAD_OOM_HANDLER.with(|x| x.replace(None)) {
        // There is a local allocator available.
        handler();
    } else {
        unsafe {
            // Transmute the atomic pointer to a function pointer and call it.
            (mem::transmute::<_, fn() -> !>(OOM_HANDLER.load(atomic::Ordering::SeqCst)))()
        }
    }
}

/// Set the OOM handler.
///
/// This is called when the process is out-of-memory.
#[inline]
pub fn set_oom_handler(handler: fn() -> !) {
    OOM_HANDLER.store(handler as *mut (), atomic::Ordering::SeqCst);
}

/// Override the OOM handler for the current thread.
///
/// # Panics
///
/// This might panic if a thread OOM handler already exists.
#[inline]
pub fn set_thread_oom_handler(handler: fn() -> !) {
    THREAD_OOM_HANDLER.with(|thread_oom| {
        // Replace it with the new handler.
        let res = thread_oom.replace(Some(handler));

        // Make sure that it doesn't override another handler.
        // TODO: Make this a warning.
        debug_assert!(res.is_none(), "Overriding the old handler. Is this intentional?");
    });
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic]
    fn test_panic_oom() {
        fn panic() -> ! {
            panic!("cats are not cute.");
        }

        set_oom_handler(panic);
        oom();
    }

    #[test]
    #[should_panic]
    fn test_panic_thread_oom() {
        fn infinite() -> ! {
            #[allow(empty_loop)]
            loop {}
        }
        fn panic() -> ! {
            panic!("cats are not cute.");
        }

        set_oom_handler(infinite);
        set_thread_oom_handler(panic);
        oom();
    }
}
