//! General error handling.

use core::sync::atomic::{self, AtomicPtr};
use core::{mem, intrinsics};

static OOM_HANDLER: AtomicPtr<()> = AtomicPtr::new(default_oom_handler as *mut ());

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
    unsafe {
        (mem::transmute::<_, fn() -> !>(OOM_HANDLER.load(atomic::Ordering::SeqCst)))()
    }
}

/// Set the OOM handler.
///
/// This is called when the process is out-of-memory.
#[inline]
pub fn set_oom_handler(handler: fn() -> !) {
    OOM_HANDLER.store(handler as *mut (), atomic::Ordering::SeqCst);
}
