//! BRK abstractions.
//!
//! This module provides safe abstractions over BRK.

use prelude::*;

use core::convert::TryInto;
use core::ptr;

use shim::{config, syscalls};

use {fail, sync};

/// The BRK mutex.
///
/// This is used for avoiding data races in multiple allocator.
static BRK_MUTEX: Mutex<BrkState> = Mutex::new(BrkState { current_brk: None });

/// A cache of the BRK state.
///
/// To avoid keeping asking the OS for information whenever needed, we cache it.
struct BrkState {
    /// The program break's end
    current_brk: Option<Pointer<u8>>,
}

/// A BRK lock.
pub struct BrkLock {
    /// The inner lock.
    state: sync::MutexGuard<'static, BrkState>,
}

impl BrkLock {
    /// Extend the program break, and return the old one.
    ///
    /// # Safety
    ///
    /// Due to being able shrink the program break, this method is unsafe.
    unsafe fn sbrk(&mut self, size: isize) -> Result<Pointer<u8>, ()> {
        log!(NOTE, "Incrementing the program break by {} bytes.", size);

        // Calculate the new program break. To avoid making multiple syscalls, we make use of the
        // state cache.
        let old_brk = self.current_brk();
        let expected_brk = old_brk.clone().offset(size);

        // Break it to me, babe!
        let new_brk = Pointer::new(syscalls::brk(expected_brk.get() as *const u8) as *mut u8);

        /// AAAARGH WAY TOO MUCH LOGGING
        ///
        /// No, sweetie. Never too much logging.
        ///
        /// REEEEEEEEEEEEEEEEEEEEEE
        log!(INTERNAL, "Program break set.");

        if expected_brk == new_brk {
            // Update the program break cache.
            self.state.current_brk = Some(expected_brk.clone());

            // Return the old break.
            Ok(old_brk)
        } else {
            // BRK failed. This syscall is rather weird, but whenever it fails (e.g. OOM) it
            // returns the old (unchanged) break.
            assert_eq!(old_brk, new_brk);
            Err(())
        }
    }

    /// Safely release memory to the OS.
    ///
    /// If failed, we return the memory.
    pub fn release(&mut self, block: Block) -> Result<(), Block> {
        // Check if we are actually next to the program break.
        if self.current_brk() == Pointer::from(block.empty_right()) {
            // Logging...
            log!(DEBUG, "Releasing {:?} to the OS.", block);

            // We are. Now, sbrk the memory back. Do to the condition above, this is safe.
            let res = unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // Note that the end of the block is addressable, making the size as well. For this
                // reason the first bit is unset and the cast will never wrap.
                self.sbrk(-(block.size() as isize))
            };

            // In debug mode, we want to check for WTF-worthy scenarios.
            debug_assert!(res.is_ok(), "Failed to set the program break back.");

            Ok(())
        } else {
            // Logging...
            log!(DEBUG, "Unable to release {:?} to the OS.", block);

            // Return the block back.
            Err(block)
        }
    }

    /// Get the current program break.
    ///
    /// If not available in the cache, requested it from the OS.
    fn current_brk(&mut self) -> Pointer<u8> {
        if let Some(ref cur) = self.state.current_brk {
            let res = cur.clone();
            // Make sure that the break is set properly (i.e. there is no libc interference).
            debug_assert!(
                res == current_brk(),
                "The cached program break is out of sync with the \
                 actual program break. Are you interfering with BRK? If so, prefer the \
                 provided 'sbrk' instead, then."
            );

            return res;
        }

        // TODO: Damn it, borrowck.
        // Get the current break.
        let cur = current_brk();
        self.state.current_brk = Some(cur.clone());

        cur
    }

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
        let brk_size = size + config::extra_brk(size) + align;

        // Use SBRK to allocate extra data segment. The alignment is used as precursor for our
        // allocated block. This ensures that it is properly memory aligned to the requested value.
        // TODO: Audit the casts.
        let (alignment_block, rest) = unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            Block::from_raw_parts(
                // Important! The conversion is failable to avoid arithmetic overflow-based
                // attacks.
                self.sbrk(brk_size.try_into().unwrap())
                    .unwrap_or_else(|()| fail::oom()),
                brk_size,
            )
        }.align(align)
            .unwrap();

        // Split the block to leave the excessive space.
        let (res, excessive) = rest.split(size);

        // Make some assertions.
        debug_assert!(res.aligned_to(align), "Alignment failed.");
        debug_assert!(
            res.size() + alignment_block.size() + excessive.size() == brk_size,
            "BRK memory leak."
        );

        (alignment_block, res, excessive)
    }
}

/// Lock the BRK lock to allow manipulating the program break.
pub fn lock() -> BrkLock {
    BrkLock {
        state: BRK_MUTEX.lock(),
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
pub unsafe extern "C" fn sbrk(size: isize) -> *mut u8 {
    lock()
        .sbrk(size)
        .unwrap_or_else(|()| Pointer::new(!0 as *mut u8))
        .get()
}

/// Get the current program break.
fn current_brk() -> Pointer<u8> {
    unsafe {
        // LAST AUDIT: 2016-08-21 (Ticki).

        Pointer::new(syscalls::brk(ptr::null()) as *mut u8)
    }
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

            assert!(brk1.get() < brk2.get());
        }
    }
}
