//! Memory bookkeeping.

use prelude::*;

use vec::Vec;

use core::{ptr, cmp, mem, intrinsics};

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
    /// The BRK multiplier.
    ///
    /// The factor determining the linear dependence between the minimum segment, and the acquired
    /// segment.
    const BRK_MULTIPLIER: usize = 2;
    /// The minimum size to be BRK'd.
    const BRK_MIN: usize = 65536;
    /// The maximal amount of _extra_ elements.
    const BRK_MAX_EXTRA: usize = 4 * 65536;

    let res = cmp::max(BRK_MIN, min.saturating_add(cmp::min(BRK_MULTIPLIER * min, BRK_MAX_EXTRA)));

    // Make some handy assertions.
    debug_assert!(res >= min, "Canonicalized BRK space is smaller than the one requested.");

    res
}

/// The default OOM handler.
///
/// This will simply abort the process.
fn default_oom_handler() -> ! {
    unsafe {
        intrinsics::abort();
    }
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
    /// 2. No two consecutive or empty block delimited blocks are adjacent, except if the right
    ///    block is empty.
    /// 3. There are no trailing empty blocks.
    ///
    /// These are invariants assuming that only the public methods are used.
    pool: Vec<Block>,
    /// The inner OOM handler.
    oom_handler: fn() -> !,
    /// The number of bytes currently allocated.
    #[cfg(feature = "debug_tools")]
    allocated: usize,
}

impl Bookkeeper {
    /// Create a new, empty block pool.
    ///
    /// This will make no allocations or BRKs.
    #[inline]
    #[cfg(feature = "debug_tools")]
    pub const fn new() -> Bookkeeper {
        Bookkeeper {
            pool: Vec::new(),
            oom_handler: default_oom_handler,
            allocated: 0,
        }

    }

    #[inline]
    #[cfg(not(feature = "debug_tools"))]
    pub const fn new() -> Bookkeeper {
        Bookkeeper {
            pool: Vec::new(),
            oom_handler: default_oom_handler,
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
            i.align(align).and_then(|(a, b)| {
                if b.size() >= size {
                    // Override the old block.
                    *i = a;
                    Some((n, b))
                } else { None }
            })
        }).next() {
            let (res, excessive) = b.split(size);

            // Mark the excessive space as free. Since `b` was split and we push `excessive`, not
            // `res`, we have index `n + 1` NOT `n`.
            self.free_ind(n + 1, excessive);
            //   ^^^^ Important note to self: Do not replace the old block, it is already replaced
            //        by the alignment block. Better let `free_ind` handle that.

            // Check consistency.
            self.check();
            debug_assert!(res.aligned_to(align), "Alignment failed.");
            debug_assert!(res.size() == size, "Requested space does not match with the returned \
                          block.");

            self.leave(res)
        } else {
            // No fitting block found. Allocate a new block.
            let res = self.alloc_fresh(size, align);
            // "Leave" the allocator.
            self.leave(res)
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
    #[inline]
    pub fn free(&mut self, block: Block) {
        // "Enter" the allocator.
        let block = self.enter(block);

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

        // Logging.
        log!(self.pool;ind, "Reallocating {:?} to size {} with align {}.", block, new_size, align);

        // "Leave" the allocator.
        let block = self.enter(block);
        // Try to do an inplace reallocation.
        match self.realloc_inplace_ind(ind, block, new_size) {
            Ok(block) => self.leave(block),
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
                debug_assert!(res.size() >= new_size, "Requested space does not match with the \
                              returned block.");

                self.leave(res)
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
    #[inline]
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
    #[inline]
    fn alloc_fresh(&mut self, size: usize, align: usize) -> Block {
        // To avoid shenanigans with unbounded recursion and other stuff, we pre-reserve the
        // buffer.
        self.reserve_more(2);

        // BRK what you need.
        let (alignment_block, res, excessive) = self.brk(size, align);

        // Add it to the list. This will not change the order, since the pointer is higher than all
        // the previous blocks.
        self.double_push(alignment_block, excessive);

        // Check consistency.
        self.check();

        res
    }

    /// Reallocate a block on a know index inplace.
    ///
    /// See [`realloc_inplace_ind`](#method.realloc_inplace.html) for more information.
    fn realloc_inplace_ind(&mut self, ind: usize, mut block: Block, new_size: usize) -> Result<Block, Block> {
        // Logging.
        log!(self.pool;ind, "Inplace reallocating {:?} to size {}.", block, new_size);

        /// Assertions...
        debug_assert!(self.find(&block) == ind, "Block is not inserted at the appropriate index.");

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
    /// This will at maximum insert one element.
    ///
    /// See [`free`](#method.free) for more information.
    #[inline]
    fn free_ind(&mut self, ind: usize, mut block: Block) {
        // Logging.
        log!(self.pool;ind, "Freeing {:?}.", block);

        // Short circuit in case of empty block.
        if block.is_empty() { return; }

        // Assertions...
        debug_assert!(self.find(&block) == ind, "Block is not inserted at the appropriate index.");

        {
            // So, we want to work around some borrowck edginess...
            let (before, after) = self.pool.split_at_mut(ind);
            // To avoid double bound checking and other shenanigans, we declare a variable holding our
            // entry's pointer.
            let entry = &mut after[0];

            // Try to merge it with the block to the right.
            if entry.merge_right(&mut block).is_ok() {
                // The merging succeeded. We proceed to try to close in the possible gap.
                if ind != 0 {
                    let _ = before[ind - 1].merge_right(entry);
                }

                // TODO fuck you rustc
                // // Check consistency.
                // self.check();

                return;

            // Dammit, let's try to merge left.
            } else if ind != 0 && before[ind - 1].merge_right(entry).is_ok() {
                // TODO fuck you rustc
                // // Check consistency.
                // self.check();

                return;
            }
        }

        // Well, it failed, so we insert it the old-fashioned way.
        self.insert(ind, block);

        // Check consistency.
        self.check();
    }

    /// Extend the data segment.
    #[inline]
    fn brk(&self, size: usize, align: usize) -> (Block, Block, Block) {
        // Logging.
        log!(self.pool;self.pool.len(), "BRK'ing a block of size, {}, and alignment {}.", size, align);

        // Calculate the canonical size (extra space is allocated to limit the number of system calls).
        let brk_size = canonicalize_brk(size).checked_add(align).unwrap_or_else(|| self.oom());

        // Use SBRK to allocate extra data segment. The alignment is used as precursor for our
        // allocated block. This ensures that it is properly memory aligned to the requested value.
        let (alignment_block, rest) = Block::brk(brk_size)
            .unwrap_or_else(|_| self.oom())
            .align(align)
            .unwrap();

        // Split the block to leave the excessive space.
        let (res, excessive) = rest.split(size);

        // Make some assertions.
        debug_assert!(res.aligned_to(align), "Alignment failed.");
        debug_assert!(res.size() + alignment_block.size() + excessive.size() == brk_size, "BRK memory leak");

        (alignment_block, res, excessive)
    }

    /// Push two blocks to the block pool.
    ///
    /// This will append the blocks to the end of the block pool (and merge if possible). Make sure
    /// that these blocks has a value higher than any of the elements in the list, to keep it
    /// sorted.
    ///
    /// This guarantees linearity so that the blocks will be adjacent.
    #[inline]
    fn double_push(&mut self, block_a: Block, block_b: Block) {
        // Logging.
        log!(self.pool;self.pool.len(), "Pushing {:?} and {:?}.", block_a, block_b);

        // Catch stupid bug...
        debug_assert!(block_a <= block_b, "The first pushed block is not lower or equal to the second.");

        // Reserve extra elements.
        self.reserve_more(2);

        self.push_no_reserve(block_a);
        self.push_no_reserve(block_b);
    }

    /// Push an element without reserving.
    fn push_no_reserve(&mut self, mut block: Block) {
        // Short-circuit in case on empty block.
        if !block.is_empty() {
            // We will try to simply merge it with the last block.
            if let Some(x) = self.pool.last_mut() {
                if x.merge_right(&mut block).is_ok() {
                    return;
                }
            }

            // Merging failed. Note that trailing empty blocks are not allowed, hence the last block is
            // the only non-empty candidate which may be adjacent to `block`.

            // We push.
            let res = self.pool.push(block);

            // Make some assertions.
            debug_assert!(res.is_ok(), "Push failed (buffer filled).");
        }
        self.check();
    }

    /// Reserve space for the block pool.
    ///
    /// This will ensure the capacity is at least `needed` greater than the current length,
    /// potentially reallocating the block pool.
    #[inline]
    fn reserve_more(&mut self, needed: usize) {
        let needed = self.pool.len() + needed;
        if needed > self.pool.capacity() {
            // TODO allow BRK-free non-inplace reservations.
            // TODO Enable inplace reallocation in this position.

            // Reallocate the block pool.

            // Make a fresh allocation.
            let size = needed.saturating_add(
                cmp::min(self.pool.capacity(), 200 + self.pool.capacity() / 2)
                // We add:
                + 1 // block for the alignment block.
                + 1 // block for the freed vector.
                + 1 // block for the excessive space.
            ) * mem::size_of::<Block>();
            let (alignment_block, alloc, excessive) = self.brk(size, mem::align_of::<Block>());

            // Refill the pool.
            let old = self.pool.refill(alloc);

            // Double push the alignment block and the excessive space linearly (note that it is in
            // fact in the end of the pool, due to BRK _extending_ the segment).
            self.double_push(alignment_block, excessive);

            // Free the old vector.
            self.free(old);

            // Check consistency.
            self.check();
        }
    }

    /// Perform a binary search to find the appropriate place where the block can be insert or is
    /// located.
    ///
    /// It is guaranteed that no block left to the returned value, satisfy the above condition.
    #[inline]
    fn find(&mut self, block: &Block) -> usize {
        // TODO optimize this function.

        let ind = match self.pool.binary_search(block) {
            Ok(x) | Err(x) => x,
        };
        let len = self.pool.len();

        // Move left.
        ind - self.pool.iter_mut()
            .rev()
            .skip(len - ind)
            .take_while(|x| x.is_empty())
            .map(|x| *x = block.empty_left())
            .count()
    }

    /// Insert a block entry at some index.
    ///
    /// If the space is non-empty, the elements will be pushed filling out the empty gaps to the
    /// right. If all places to the right is occupied, it will reserve additional space to the
    /// block pool.
    ///
    /// # Panics
    ///
    /// Panics on when `ind` is greater than the block pool's length.
    ///
    /// # Example
    ///
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
    #[inline]
    fn insert(&mut self, ind: usize, block: Block) {
        // Logging.
        log!(self.pool;ind, "Inserting block {:?}.", block);

        // Bound check.
        assert!(self.pool.len() >= ind, "Insertion out of bounds.");

        // Some assertions...
        debug_assert!(self.pool.is_empty() || block >= self.pool[ind + 1], "Inserting at {} will \
                      make the list unsorted.", ind);
        debug_assert!(self.find(&block) == ind, "Block is not inserted at the appropriate index.");
        debug_assert!(!block.is_empty(), "Inserting an empty block.");

        // Find the next gap, where a used block were.
        let n = {
            // The element we serach for.
            let elem = self.pool
                .iter()
                .skip(ind)
                .enumerate()
                .filter(|&(_, x)| !x.is_empty())
                .next()
                .map(|(n, _)| n);

            elem.unwrap_or_else(|| {
                // Reserve capacity.
                self.reserve_more(1);

                // We default to the end of the pool.
                self.pool.len()
            })
        };

        unsafe {
            // Memmove the elements.
            ptr::copy(self.pool.get_unchecked(ind) as *const Block,
                      self.pool.get_unchecked_mut(ind + 1) as *mut Block, self.pool.len() - n);

            // Set the element.
            *self.pool.get_unchecked_mut(ind) = block;
        }

        // Check consistency.
        self.check();
    }

    /// Call the OOM handler.
    ///
    /// This is used one out-of-memory errors, and will never return. Usually, it simply consists
    /// of aborting the process.
    fn oom(&self) -> ! {
        (self.oom_handler)()
    }

    /// Set the OOM handler.
    ///
    /// This is called when the process is out-of-memory.
    #[inline]
    pub fn set_oom_handler(&mut self, handler: fn() -> !) {
        self.oom_handler = handler;
    }

    /// Leave the allocator.
    ///
    /// A block should be "registered" through this function when it leaves the allocated (e.g., is
    /// returned), these are used to keep track of the current heap usage, and memory leaks.
    #[inline]
    fn leave(&mut self, block: Block) -> Block {
        // Update the number of bytes allocated.
        #[cfg(feature = "debug_tools")]
        {
            self.allocated += block.size();
        }

        block
    }

    /// Enter the allocator.
    ///
    /// A block should be "registered" through this function when it enters the allocated (e.g., is
    /// given as argument), these are used to keep track of the current heap usage, and memory
    /// leaks.
    #[inline]
    fn enter(&mut self, block: Block) -> Block {
        // Update the number of bytes allocated.
        #[cfg(feature = "debug_tools")]
        {
            self.allocated -= block.size();
        }

        block
    }

    /// No-op in release mode.
    #[cfg(not(debug_assertions))]
    #[inline]
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
                assert!(i >= prev, "The block pool is not sorted at index, {} ({:?} < {:?})", n, i,
                        prev);
                // Make sure no blocks are adjacent.
                assert!(!prev.left_to(i) || i.is_empty(), "Adjacent blocks at index, {} ({:?} and \
                        {:?})", n, i, prev);

                // Set the variable tracking the previous block.
                prev = i;
            }
        }
    }

    /// Check for memory leaks.
    ///
    /// This will ake sure that all the allocated blocks have been freed.
    #[cfg(feature = "debug_tools")]
    pub fn assert_no_leak(&self) {
        assert!(self.allocated == self.pool.capacity() * mem::size_of::<Block>(), "Not all blocks \
                freed. Total allocated space is {} ({} free blocks).", self.allocated,
                self.pool.len());
    }
}
