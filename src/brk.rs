//! BRK abstractions.
//!
//! This module provides safe abstractions over BRK.

use prelude::*;

use core::{cmp, ptr};
use core::convert::TryInto;

use {sync, sys, fail};

/// The BRK mutex.
///
/// This is used for avoiding data races in multiple allocator.
static BRK_MUTEX: Mutex<BrkState> = Mutex::new(BrkState {
    brk_end: None,
});

/// A cache of the BRK state.
///
/// To avoid keeping asking the OS for information whenever needed, we cache it.
struct BrkState {
    /// The program break's end
    brk_end: Option<Pointer<u8>>,
}

/// A BRK lock.
pub struct BrkLock {
    /// The inner lock.
    guard: sync::MutexGuard<'static, BrkState>,
}

impl BrkLock {
    /// BRK new space.
    ///
    /// The first block represents the aligner segment (that is the precursor aligning the middle
    /// block to `align`), the second one is the result and is of exactly size `size`. The last
    /// block is the excessive space.
    ///
    /// # Failure
    ///
    /// This method calls the OOM handler if it is unable to acquire the needed space.
    // TODO: This method is possibly unsafe.
    pub fn canonical_brk(&mut self, size: usize, align: usize) -> (Block, Block, Block) {
        // Calculate the canonical size (extra space is allocated to limit the number of system calls).
        let brk_size = canonicalize_space(size) + align;

        // Use SBRK to allocate extra data segment. The alignment is used as precursor for our
        // allocated block. This ensures that it is properly memory aligned to the requested value.
        // TODO: Audit the casts.
        let (alignment_block, rest) = unsafe {
            Block::from_raw_parts(
                self.sbrk(brk_size.try_into().unwrap()).unwrap_or_else(|()| fail::oom()),
                brk_size,
            )
        }.align(align).unwrap();

        // Split the block to leave the excessive space.
        let (res, excessive) = rest.split(size);

        // Make some assertions.
        debug_assert!(res.aligned_to(align), "Alignment failed.");
        debug_assert!(res.size() + alignment_block.size() + excessive.size() == brk_size, "BRK memory leak.");

        (alignment_block, res, excessive)
    }

    /// Extend the program break.
    ///
    /// # Safety
    ///
    /// Due to being able shrink the program break, this method is unsafe.
    unsafe fn sbrk(&mut self, size: isize) -> Result<Pointer<u8>, ()> {
        // Calculate the new program break. To avoid making multiple syscalls, we make use of the
        // state cache.
        let new_brk = self.guard.brk_end
            .clone()
            .unwrap_or_else(current_brk)
            .offset(size);

        // Break it to me, babe!
        let old_brk = Pointer::new(sys::brk(*new_brk as *const u8) as *mut u8);

        if new_brk == old_brk && size != 0 {
            // BRK failed. This syscall is rather weird, but whenever it fails (e.g. OOM) it
            // returns the old (unchanged) break.
            Err(())
        } else {
            // Update the program break cache.
            self.guard.brk_end = Some(old_brk.clone());

            // Return the old break.
            Ok(old_brk)
        }
    }
}

/// Lock the BRK lock to allow manipulating the program break.
pub fn lock() -> BrkLock {
    BrkLock {
        guard: BRK_MUTEX.lock(),
    }
}

/// `SBRK` symbol which can coexist with the allocator.
///
/// `SBRK`-ing directly (from the `BRK` syscall or libc) might make the state inconsistent. This
/// function makes sure that's not happening.
///
/// With the exception of being able to coexist, it follows the same rules. Refer to the relevant
/// documentation.
///
/// # Failure
///
/// On failure the maximum pointer (`!0 as *mut u8`) is returned.
pub unsafe extern fn sbrk(size: isize) -> *mut u8 {
    *lock().sbrk(size).unwrap_or_else(|()| Pointer::new(!0 as *mut u8))
}

/// Get the current program break.
fn current_brk() -> Pointer<u8> {
    unsafe { Pointer::new(sys::brk(ptr::null()) as *mut u8) }
}

/// Canonicalize a BRK request.
///
/// Syscalls can be expensive, which is why we would rather accquire more memory than necessary,
/// than having many syscalls acquiring memory stubs. Memory stubs are small blocks of memory,
/// which are essentially useless until merge with another block.
///
/// To avoid many syscalls and accumulating memory stubs, we BRK a little more memory than
/// necessary. This function calculate the memory to be BRK'd based on the necessary memory.
///
/// The return value is always greater than or equals to the argument.
#[inline]
fn canonicalize_space(min: usize) -> usize {
    // TODO: Tweak this.
    /// The BRK multiplier.
    ///
    /// The factor determining the linear dependence between the minimum segment, and the acquired
    /// segment.
    const BRK_MULTIPLIER: usize = 2;
    /// The minimum size to be BRK'd.
    const BRK_MIN: usize = 1024;
    /// The maximal amount of _extra_ elements.
    const BRK_MAX_EXTRA: usize = 65536;

    let res = cmp::max(BRK_MIN, min + cmp::min(BRK_MULTIPLIER * min, BRK_MAX_EXTRA));

    // Make some handy assertions.
    debug_assert!(res >= min, "Canonicalized BRK space is smaller than the one requested.");

    res
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_ordered() {
        let brk = lock().canonical_brk(20, 1);

        assert!(brk.0 <= brk.1);
        assert!(brk.1 <= brk.2);
    }

    #[test]
    fn test_brk_grow_up() {
        unsafe {
            let brk1 = lock().sbrk(5).unwrap();
            let brk2 = lock().sbrk(100).unwrap();

            assert!(*brk1 < *brk2);
        }
    }
}
