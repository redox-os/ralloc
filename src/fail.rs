//! General error handling.

use prelude::*;

use core::mem;
use core::sync::atomic::{self, AtomicPtr};

use shim::config;

#[cfg(feature = "tls")]
use tls;

/// The global OOM handler.
static OOM_HANDLER: AtomicPtr<()> = AtomicPtr::new(config::default_oom_handler as *mut ());
#[cfg(feature = "tls")]
tls! {
    /// The thread-local OOM handler.
    static THREAD_OOM_HANDLER: MoveCell<Option<fn() -> !>> = MoveCell::new(None);
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
    // If TLS is enabled, we will use the thread-local OOM.
    #[cfg(feature = "tls")]
    {
        if let Some(handler) = THREAD_OOM_HANDLER.with(|x| x.replace(None)) {
            log!(DEBUG, "Calling the local OOM handler.");

            handler();
        }
    }

    log!(DEBUG, "Calling the global OOM handler.");

    unsafe {
        // LAST AUDIT: 2016-08-21 (Ticki).

        // Transmute the atomic pointer to a function pointer and call it.
        (mem::transmute::<_, fn() -> !>(OOM_HANDLER.load(atomic::Ordering::SeqCst)))()
    }
}

/// Set the OOM handler.
///
/// This is called when the process is out-of-memory.
#[inline]
pub fn set_oom_handler(handler: fn() -> !) {
    // Logging...
    log!(NOTE, "Setting the global OOM handler.");

    OOM_HANDLER.store(handler as *mut (), atomic::Ordering::SeqCst);
}

/// Override the OOM handler for the current thread.
///
/// # Panics
///
/// This might panic if a thread OOM handler already exists.
#[inline]
#[cfg(feature = "tls")]
pub fn set_thread_oom_handler(handler: fn() -> !) {
    // Logging...
    log!(NOTE, "Setting the thread OOM handler.");

    THREAD_OOM_HANDLER.with(|thread_oom| {
        // Replace it with the new handler.
        let res = thread_oom.replace(Some(handler));

        // Throw a warning if it overrides another handler.
        if res.is_some() {
            log!(WARNING, "An old thread OOM handler was overriden.");
        }
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
    #[cfg(feature = "tls")]
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
