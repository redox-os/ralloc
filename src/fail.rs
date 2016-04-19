//! Primitives for allocator failures.

use core::sync::atomic::{self, AtomicPtr};
use core::{mem, intrinsics};

/// The OOM handler.
static OOM_HANDLER: AtomicPtr<()> = AtomicPtr::new(default_oom_handler as *mut ());

/// The default OOM handler.
///
/// This will simply abort the process with exit code, 1.
fn default_oom_handler() -> ! {
    unsafe {
        intrinsics::abort();
    }
}

/// Call the OOM handler.
#[cold]
#[inline(never)]
pub fn oom() -> ! {
    let value = OOM_HANDLER.load(atomic::Ordering::SeqCst);
    let handler: fn() -> ! = unsafe { mem::transmute(value) };
    handler();
}

/// Set the OOM handler.
///
/// This allows for overwriting the default OOM handler with a custom one.
pub fn set_oom_handler(handler: fn() -> !) {
    OOM_HANDLER.store(handler as *mut (), atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic]
    fn test_handler() {
        fn panic() -> ! {
            panic!("blame canada for the OOM.");
        }

        set_oom_handler(panic);
        oom();
    }
}
