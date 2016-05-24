//! Memory bookkeeping.

use block::Block;
use vec::Vec;
use fail;

use core::{ptr, cmp, mem};
use core::mem::{align_of, size_of};

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
fn canonicalize_brk(min: usize) -> usize {
    const BRK_MULTIPLIER: usize = 1;
    const BRK_MIN: usize = 200;
    /// The maximal amount of _extra_ elements.
    const BRK_MAX_EXTRA: usize = 10000;

    let res = cmp::max(BRK_MIN, min.saturating_add(cmp::min(BRK_MULTIPLIER * min, BRK_MAX_EXTRA)));

    // Make some handy assertions.
    debug_assert!(res >= min, "Canonicalized BRK space is smaller than the one requested.");

    res
}

/// The memory bookkeeper.
///
/// This is the main component of ralloc. Its job is to keep track of the free blocks in a
/// structured manner, such that allocation, reallocation, and deallocation are all efficient.
/// Particularly, it keeps a list of blocks, commonly called the "block pool". This list is kept.
/// Entries in the block pool can be "empty", meaning that you can overwrite the entry without
/// breaking consistency.
///
/// Only making use of only [`alloc`](#method.alloc), [`free`](#method.free),
/// [`realloc`](#method.realloc) (and following their respective assumptions) guarantee that no
/// buffer overrun, arithmetic overflow, panic, or otherwise unexpected crash will happen.
pub struct Bookkeeper {
    /// The internal block pool.
    ///
    /// Guarantees
    /// ==========
    ///
    /// Certain guarantees are made:
    ///
    /// 1. The list is always sorted with respect to the block's pointers.
    /// 2. No two blocks are adjacent.
    pool: Vec<Block>,
}

impl Bookkeeper {
    /// Create a new, empty block pool.
    ///
    /// This will make no allocations or BRKs.
    #[inline]
    pub const fn new() -> Bookkeeper {
        Bookkeeper {
            pool: Vec::new(),
        }
    }


    /// Allocate a chunk of memory.
    ///
    /// This function takes a size and an alignment. From these a fitting block is found, to which
    /// a pointer is returned. The block returned is guaranteed to be aligned to `align`.
    ///
    /// # Example
    ///
    /// We start with our initial segment.
    ///
    /// ```notrust
    ///    Address space
    ///   I---------------------------------I
    /// B
    /// l
    /// k
    /// s
    /// ```
    ///
    /// We then split it at the aligner, which is used for making sure that
    /// the pointer is aligned properly.
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B   ^    I--------------------------I
    /// l  al
    /// k
    /// s
    /// ```
    ///
    /// We then use the remaining block, but leave the excessive space.
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B                           I--------I
    /// l        \_________________/
    /// k        our allocated block.
    /// s
    /// ```
    ///
    /// A block representing the marked area is then returned.
    pub fn alloc(&mut self, size: usize, align: usize) -> Block {
        // TODO: scan more intelligently.
        if let Some((n, b)) = self.pool.iter_mut().enumerate().filter_map(|(n, i)| {
            // Try to split at the aligner.
            i.align(align).map(|(a, b)| {
                *i = a;
                (n, b)
            })
        }).next() {
            let (res, excessive) = b.split(size);

            // Mark the excessive space as free.
            self.free_ind(n, excessive);
            //   ^^^^ Important note to self: Do not replace the old block, it is already replaced
            //        by the alignment block. Better let `free_ind` handle that.

            // Check consistency.
            self.check();
            debug_assert!(res.aligned_to(align), "Alignment failed.");
            debug_assert!(res.size() == size, "Requested space does not match with the returned \
                          block.");

            res
        } else {
            // No fitting block found. Allocate a new block.
            self.alloc_fresh(size, align)
        }
    }

    /// Free a memory block.
    ///
    /// After this have been called, no guarantees are made about the passed pointer. If it want
    /// to, it could begin shooting laser beams.
    ///
    /// Freeing an invalid block will drop all future guarantees about this bookkeeper.
    ///
    /// # Example
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B                                  I--------I
    /// l        \_________________/
    /// k     the used block we want to deallocate.
    /// s
    /// ```
    ///
    /// If the blocks are adjacent, we merge them:
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B        I-----------------I
    /// l                                  I--------I
    /// k
    /// s
    /// ```
    ///
    /// This gives us:
    ///
    /// ```notrust
    ///    Address space
    ///   I------------------------I
    /// B                                  I--------I
    /// l
    /// k
    /// s
    /// ```
    ///
    /// And we're done. If it cannot be done, we insert the block, while keeping the list sorted.
    /// See [`insert`](#method.insert) for details.
    pub fn free(&mut self, block: Block) {
        let ind = self.find(&block);
        self.free_ind(ind, block);
    }

    /// Reallocate memory.
    ///
    /// If necessary (inplace reallocation is not possible or feasible) it will allocate a new
    /// buffer, fill it with the contents of the old buffer, and deallocate the replaced buffer.
    ///
    /// The following guarantees are made:
    ///
    /// 1. The returned block is valid and aligned to `align`.
    /// 2. The returned block contains the same data byte-for-byte as the original buffer.
    ///
    /// The data will be truncated if `new_size` is smaller than `block`'s size.
    ///
    /// Example
    /// =======
    ///
    /// We will first try to perform an in-place reallocation, and if that fails, we will use
    /// memmove.
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B \~~~~~~~~~~~~~~~~~~~~~/
    /// l     needed
    /// k
    /// s
    /// ```
    ///
    /// We simply find the block next to our initial block. If this block is free and have
    /// sufficient size, we will simply merge it into our initial block, and leave the excessive
    /// space as free. If these conditions are not met, we have to allocate a new list, and then
    /// deallocate the old one, after which we use memmove to copy the data over to the newly
    /// allocated list.
    pub fn realloc(&mut self, block: Block, new_size: usize, align: usize) -> Block {
        // Find the index.
        let ind = self.find(&block);

        // Try to do an inplace reallocation.
        match self.realloc_inplace_ind(ind, block, new_size) {
            Ok(block) => block,
            Err(block) => {
                // Reallocation cannot be done inplace.

                // Allocate a new block with the same size.
                let mut res = self.alloc(new_size, align);

                // Copy the old data to the new location.
                block.copy_to(&mut res);

                // Free the old block.
                self.free_ind(ind, block);

                // Check consistency.
                self.check();
                debug_assert!(res.aligned_to(align), "Alignment failed.");
                debug_assert!(res.size() == new_size, "Requested space does not match with the \
                              returned block.");

                res
            },
        }
    }

    /// Extend/shrink the buffer inplace.
    ///
    /// This will try to extend the buffer without copying, if the new size is larger than the old
    /// one. If not, truncate the block and place it back to the pool.
    ///
    /// On failure, return `Err(Block)` with the old _intact_ block. Shrinking cannot fail.
    ///
    /// This shouldn't be used when the index of insertion is known, since this performs an binary
    /// search to find the blocks index. When you know the index use
    /// [`realloc_inplace_ind`](#method.realloc_inplace_ind.html).
    pub fn realloc_inplace(&mut self, block: Block, new_size: usize) -> Result<Block, Block> {
        let ind = self.find(&block);
        let res = self.realloc_inplace_ind(ind, block, new_size);

        // Check consistency.
        debug_assert!(res.as_ref().ok().map_or(true, |x| x.size() == new_size), "Requested space \
                      does not match with the returned block.");

        res
    }

    /// Allocate _fresh_ space.
    ///
    /// "Fresh" means that the space is allocated through a BRK call to the kernel.
    ///
    /// The returned pointer is guaranteed to be aligned to `align`.
    fn alloc_fresh(&mut self, size: usize, align: usize) -> Block {
        // Calculate the canonical size (extra space is allocated to limit the number of system calls).
        let brk_size = canonicalize_brk(size).checked_add(align).unwrap_or_else(|| fail::oom());

        // Use SYSBRK to allocate extra data segment. The alignment is used as precursor for our
        // allocated block. This ensures that it is properly memory aligned to the requested value.
        let (alignment_block, rest) = unsafe {
            Block::brk(brk_size)
        }.align(align).unwrap();

        // Split the block to leave the excessive space.
        let (res, excessive) = rest.split(size);

        // Make some assertions.
        debug_assert!(res.aligned_to(align), "Alignment failed.");
        debug_assert!(res.size() + alignment_block.size() + excessive.size() == brk_size, "BRK memory \
                      leak in fresh allocation.");
        // Add it to the list. This will not change the order, since the pointer is higher than all
        // the previous blocks.
        self.push(alignment_block);

        // Push the excessive space to the end of the block pool.
        self.push(excessive);

        // Check consistency.
        self.check();

        res
    }

    /// Reallocate a block on a know index inplace.
    ///
    /// See [`realloc_inplace_ind`](#method.realloc_inplace.html) for more information.
    fn realloc_inplace_ind(&mut self, ind: usize, mut block: Block, new_size: usize) -> Result<Block, Block> {
        if new_size <= block.size() {
            // Shrink the block.

            // Split the block in two segments, the main segment and the excessive segment.
            let (block, excessive) = block.split(new_size);
            // Free the excessive segment.
            self.free_ind(ind, excessive);

            // Make some assertions to avoid dumb bugs.
            debug_assert!(block.size() == new_size, "Block wasn't shrinked properly.");

            // Run a consistency check.
            self.check();

            return Ok(block);

            // We check if `ind` is the end of the array.
        } else if let Some(entry) = self.pool.get_mut(ind + 1) {
            // Note that we are sure that no segments in the array are adjacent (unless they have size
            // 0). This way we know that we will, at maximum, need one and only one block for extending
            // the current block.
            if entry.size() + block.size() >= new_size && block.merge_right(entry).is_ok() {
                // Merge succeeded.

                // Place the excessive block back.
                let (res, excessive) = block.split(new_size);
                *entry = excessive;
                // Block will still not be adjacent, due to `excessive` being guaranteed to not be
                // adjacent to the next block.

                // TODO, damn you borrowck
                // Run a consistency check.
                // self.check();

                // TODO, drop excessive space
                return Ok(res);
            }
        }

        Err(block)
    }

    /// Free a block placed on some index.
    ///
    /// See [`free`](#method.free) for more information.
    fn free_ind(&mut self, ind: usize, mut block: Block) {
        // Do a lazy shortcut.
        if block.is_empty() { return; }

        // Try to merge left, and then right.
        if {
            // To avoid double bound checking and other shenanigans, we declare a variable holding our
            // entry's pointer.
            let entry = &mut self.pool[ind];

            // Make some handy assertions.
            debug_assert!(entry != &mut block, "Double free.");
            entry.merge_right(&mut block).is_err()
        } | (ind == 0 || self.pool[ind - 1].merge_right(&mut block).is_err()) {
            // Since merge failed, we will have to insert it in a normal manner.
            self.insert(ind, block);
        }

        // Check consistency.
        self.check();
    }

    /// Push to the block pool.
    ///
    /// This will append a block entry to the end of the block pool. Make sure that this entry has
    /// a value higher than any of the elements in the list, to keep it sorted.
    #[inline]
    fn push(&mut self, mut block: Block) {
        // First, we will do a shortcut in case that the block is empty.
        if block.is_empty() {
            return;
        }

        // We will try to simply merge it with the last block.
        if let Some(x) = self.pool.last_mut() {
            if x.merge_right(&mut block).is_ok() {
                return;
            }
        }

        // It failed, so we will need to add a new block to the end.
        let len = self.pool.len();

        // This is guaranteed not to overflow, since `len` is bounded by the address space, since
        // each entry represent at minimum one byte, meaning that `len` is bounded by the address
        // space.
        self.reserve(len + 1);

        let res = self.pool.push(block);

        // Make some assertions.
        debug_assert!(res.is_ok(), "Push failed (buffer filled).");
        self.check();
    }

    /// Reserve space for the block pool.
    ///
    /// This will extend the capacity to a number greater than or equals to `needed`, potentially
    /// reallocating the block pool.
    #[inline]
    fn reserve(&mut self, needed: usize) {
        if needed > self.pool.capacity() {
            // Fool the borrowchecker.
            let len = self.pool.len();

            // Calculate the index.
            let ind = self.find(&Block::empty(&self.pool.ptr().duplicate().cast()));
            // Temporarily steal the block, placing an empty vector in its place.
            let block = Block::from(mem::replace(&mut self.pool, Vec::new()));
            // TODO allow BRK-free non-inplace reservations.

            // Reallocate the block pool.

            // We first try do it inplace.
            match self.realloc_inplace_ind(ind, block, needed * size_of::<Block>()) {
                Ok(succ) => {
                    // Set the extend block back.
                    self.pool = unsafe { Vec::from_raw_parts(succ, len) };
                },
                Err(block) => {
                    // Inplace alloc failed, so we have to BRK some new space.

                    // Reconstruct the vector.
                    self.pool = unsafe { Vec::from_raw_parts(block, len) };

                    // Make a fresh allocation.
                    let size = needed.saturating_add(
                        cmp::min(self.pool.capacity(), 200 + self.pool.capacity() / 2)
                    ) * size_of::<Block>();
                    let alloc = self.alloc_fresh(size, align_of::<Block>());

                    // Inplace reallocation suceeeded, place the block back as the pool.
                    let refilled = self.pool.refill(alloc);
                    self.free_ind(ind, refilled);
                },
            }

            // Check consistency.
            self.check();
        }
    }

    /// Perform a binary search to find the appropriate place where the block can be insert or is
    /// located.
    fn find(&self, block: &Block) -> usize {
        match self.pool.binary_search(block) {
            Ok(x) | Err(x) => x,
        }
    }

    /// Insert a block entry at some index.
    ///
    /// If the space is non-empty, the elements will be pushed filling out the empty gaps to the
    /// right. If all places to the right is occupied, it will reserve additional space to the
    /// block pool.
    ///
    /// # Example
    /// We want to insert the block denoted by the tildes into our list. Perform a binary search to
    /// find where insertion is appropriate.
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B < here                      I--------I
    /// l                                              I------------I
    /// k
    /// s                                                             I---I
    ///                  I~~~~~~~~~~I
    /// ```
    ///
    /// We keep pushing the blocks to the right to the next entry until a empty entry is reached:
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B < here                      I--------I <~ this one cannot move down, due to being blocked.
    /// l
    /// k                                              I------------I <~ thus we have moved this one down.
    /// s                                                             I---I
    ///              I~~~~~~~~~~I
    /// ```
    ///
    /// Repeating yields:
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B < here
    /// l                             I--------I <~ this one cannot move down, due to being blocked.
    /// k                                              I------------I <~ thus we have moved this one down.
    /// s                                                             I---I
    ///              I~~~~~~~~~~I
    /// ```
    ///
    /// Now an empty space is left out, meaning that we can insert the block:
    ///
    /// ```notrust
    ///    Address space
    ///   I------I
    /// B            I----------I
    /// l                             I--------I
    /// k                                              I------------I
    /// s                                                             I---I
    /// ```
    ///
    /// The insertion is now completed.
    fn insert(&mut self, ind: usize, block: Block) {
        // Some assertions...
        debug_assert!(block >= self.pool[ind + 1], "Inserting at {} will make the list unsorted.", ind);
        debug_assert!(self.find(&block) == ind, "Block is not inserted at the appropriate index.");

        // TODO consider moving right before searching left.

        // Find the next gap, where a used block were.
        if let Some((n, _)) = self.pool.iter().skip(ind).enumerate().filter(|&(_, x)| !x.is_empty()).next() {
            // Memmove the blocks to close in that gap.
            unsafe {
                ptr::copy(self.pool[ind..].as_ptr(), self.pool[ind + 1..].as_mut_ptr(), self.pool.len() - n);
            }

            // Place the block left to the moved line.
            self.pool[ind] = block;
        } else {
            self.push(block);
        }

        // Check consistency.
        self.check();
    }

    /// No-op in release mode.
    #[cfg(not(debug_assertions))]
    fn check(&self) {}

    /// Perform consistency checks.
    ///
    /// This will check for the following conditions:
    ///
    /// 1. The list is sorted.
    /// 2. No blocks are adjacent.
    #[cfg(debug_assertions)]
    fn check(&self) {
        if let Some(x) = self.pool.first() {
            let mut prev = x;
            for (n, i) in self.pool.iter().enumerate().skip(1) {
                // Check if sorted.
                assert!(i >= prev, "The block pool is not sorted at index, {} ({:?} < {:?})", n, i, prev);
                // Make sure no blocks are adjacent.
                assert!(!prev.left_to(i), "Adjacent blocks at index, {} ({:?} and {:?})", n, i, prev);

                // Set the variable tracking the previous block.
                prev = i;
            }
        }
    }
}
