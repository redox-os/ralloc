//! General error handling.

use core::sync::atomic::{self, AtomicPtr};
use core::{mem, intrinsics};

/// The global OOM handler.
static OOM_HANDLER: AtomicPtr<()> = AtomicPtr::new(default_oom_handler as *mut ());
tls! {
    /// The thread-local OOM handler.
    static THREAD_OOM_HANDLER: Option<fn() -> !> = None;
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
    if let Some(handler) = THREAD_OOM_HANDLER.get().unwrap() {
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
#[inline]
pub fn set_thread_oom_handler(handler: fn() -> !) {
    *THREAD_OOM_HANDLER.get().unwrap() = handler;
}
