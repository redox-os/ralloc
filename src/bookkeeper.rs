//! Memory bookkeeping.

use prelude::*;

use core::ops::Range;
use core::{mem, ops, ptr};

use shim::config;

/// Elements required _more_ than the length as capacity.
///
/// This represents how many elements that are needed to conduct a `reserve` without the
/// stack overflowing, plus one (representing the new element):
///
/// 1. Aligner.
/// 2. Excessive space.
/// 3. The old buffer.
/// 4. The pushed or inserted block.
///
/// See assumption 4.
pub const EXTRA_ELEMENTS: usize = 4;

#[cfg(feature = "alloc_id")]
use core::sync::atomic::{self, AtomicUsize};
/// The bookkeeper ID count.
///
/// This is atomically incremented whenever a new `Bookkeeper` is created.
#[cfg(feature = "alloc_id")]
static BOOKKEEPER_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The memory bookkeeper.
///
/// This stores data about the state of the allocator, and in particular, the free memory.
///
/// The actual functionality is provided by [`Allocator`](./trait.Allocator.html).
pub struct Bookkeeper {
    /// The internal block pool.
    ///
    /// Entries in the block pool can be "empty", meaning that you can overwrite the entry without
    /// breaking consistency.
    ///
    /// # Assumptions
    ///
    /// Certain assumptions are made:
    ///
    /// 1. The list is always sorted with respect to the block's pointers.
    /// 2. No two consecutive or empty block delimited blocks are adjacent, except if the right
    ///    block is empty.
    /// 3. There are no trailing empty blocks.
    /// 4. The capacity is always `EXTRA_ELEMENTS` blocks more than the length (this is due to
    ///    reallocation pushing at maximum two elements, so we reserve two or more extra to allow
    ///    pushing one additional element without unbounded recursion).
    ///
    /// These are **not** invariants: If these assumpptions are not held, it will simply act strange
    /// (e.g. logic bugs), but not memory unsafety.
    pool: Vec<Block>,
    /// The total number of bytes in the pool.
    total_bytes: usize,
    /// Is this bookkeeper currently reserving?
    ///
    /// This is used to avoid unbounded metacircular reallocation (reservation).
    ///
    // TODO: Find a replacement for this "hack".
    reserving: bool,
    /// The allocator ID.
    ///
    /// This is simply to be able to distinguish allocators in the locks.
    #[cfg(feature = "alloc_id")]
    id: usize,
}

impl Bookkeeper {
    /// Create a new bookkeeper with some initial vector.
    pub fn new(vec: Vec<Block>) -> Bookkeeper {
        // Make sure the assumptions are satisfied.
        debug_assert!(
            vec.capacity() >= EXTRA_ELEMENTS,
            "Not enough initial capacity of the vector."
        );
        debug_assert!(vec.is_empty(), "Initial vector isn't empty.");

        // TODO: When added use expr field attributes.
        #[cfg(feature = "alloc_id")]
        let res = Bookkeeper {
            pool: vec,
            total_bytes: 0,
            reserving: false,
            // Increment the ID counter to get a brand new ID.
            id: BOOKKEEPER_ID_COUNTER.fetch_add(1, atomic::Ordering::SeqCst),
        };
        #[cfg(not(feature = "alloc_id"))]
        let res = Bookkeeper {
            pool: vec,
            total_bytes: 0,
            reserving: false,
        };

        bk_log!(res, "Bookkeeper created.");
        res.check();

        res
    }

    /// Perform a binary search to find the appropriate place where the block can be insert or is
    /// located.
    ///
    /// It is guaranteed that no block left to the returned value, satisfy the above condition.
    #[inline]
    fn find(&mut self, block: &Block) -> usize {
        // Logging.
        bk_log!(self, "Searching (exact) for {:?}.", block);

        let ind = match self.pool.binary_search(block) {
            Ok(x) | Err(x) => x,
        };
        let len = self.pool.len();

        // Move left.
        ind - self
            .pool
            .iter_mut()
            .rev()
            .skip(len - ind)
            .take_while(|x| x.is_empty())
            .count()
    }

    /// Perform a binary search to find the appropriate bound where the block can be insert or is
    /// located.
    ///
    /// It is guaranteed that no block left to the returned value, satisfy the above condition.
    #[inline]
    fn find_bound(&mut self, block: &Block) -> Range<usize> {
        // Logging.
        bk_log!(self, "Searching (bounds) for {:?}.", block);

        let mut left_ind = match self.pool.binary_search(block) {
            Ok(x) | Err(x) => x,
        };

        let len = self.pool.len();

        // Move left.
        left_ind -= self
            .pool
            .iter_mut()
            .rev()
            .skip(len - left_ind)
            .take_while(|x| x.is_empty())
            .count();

        let mut right_ind = match self.pool.binary_search(&block.empty_right()) {
            Ok(x) | Err(x) => x,
        };

        // Move right.
        right_ind += self
            .pool
            .iter()
            .skip(right_ind)
            .take_while(|x| x.is_empty())
            .count();

        left_ind..right_ind
    }

    /// Go over every block in the allocator and call some function.
    ///
    /// Technically, this could be done through an iterator, but this, more unidiomatic, way is
    /// slightly faster in some cases.
    pub fn for_each<F: FnMut(Block)>(mut self, mut f: F) {
        // Logging.
        bk_log!(self, "Iterating over the blocks of the bookkeeper...");

        // Run over all the blocks in the pool.
        for i in self.pool.pop_iter() {
            f(i);
        }

        // Take the block holding the pool.
        f(Block::from(self.pool));
    }

    /// Pop the top block from the pool.
    pub fn pop(&mut self) -> Option<Block> {
        self.pool.pop().map(|res| {
            // Update the byte count.
            self.total_bytes -= res.size();

            // Check stuff, just in case.
            self.check();

            res
        })
    }

    /// Get the length of the pool.
    pub fn len(&self) -> usize {
        self.pool.len()
    }

    /// Get the total bytes of memory in the pool.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Perform consistency checks.
    ///
    /// This will check for the following conditions:
    ///
    /// 1. The list is sorted.
    /// 2. No blocks are adjacent.
    ///
    /// This is NOOP in release mode.
    fn check(&self) {
        if cfg!(debug_assertions) {
            // Logging.
            bk_log!(self, "Checking...");

            // The total number of bytes.
            let mut total_bytes = 0;
            // Reverse iterator over the blocks.
            let mut it = self.pool.iter().enumerate().rev();

            // Check that the capacity is large enough.
            assert!(
                self.reserving || self.pool.len() + EXTRA_ELEMENTS <= self.pool.capacity(),
                "The capacity should be at least {} more than the length of the pool.",
                EXTRA_ELEMENTS
            );

            if let Some((_, x)) = it.next() {
                // Make sure there are no leading empty blocks.
                assert!(!x.is_empty(), "The leading block is empty.");

                total_bytes += x.size();

                let mut next = x;
                for (n, i) in it {
                    total_bytes += i.size();

                    // Check if sorted.
                    assert!(
                        next >= i,
                        "The block pool is not sorted at index, {} ({:?} < {:?}).",
                        n,
                        next,
                        i
                    );
                    // Make sure no blocks are adjacent.
                    assert!(
                        !i.left_to(next) || i.is_empty(),
                        "Adjacent blocks at index, {} ({:?} and \
                         {:?})",
                        n,
                        i,
                        next
                    );
                    // Make sure an empty block has the same address as its right neighbor.
                    assert!(
                        !i.is_empty() || i == next,
                        "Empty block not adjacent to right neighbor \
                         at index {} ({:?} and {:?})",
                        n,
                        i,
                        next
                    );

                    // Set the variable tracking the previous block.
                    next = i;
                }

                // Check for trailing empty blocks.
                assert!(
                    !self.pool.last().unwrap().is_empty(),
                    "Trailing empty blocks."
                );
            }

            // Make sure the sum is maintained properly.
            assert!(
                total_bytes == self.total_bytes,
                "The sum is not equal to the 'total_bytes' \
                 field: {} ≠ {}.",
                total_bytes,
                self.total_bytes
            );
        }
    }
}

/// An allocator.
///
/// This provides the functionality of the memory bookkeeper, requiring only provision of two
/// methods, defining the "breaker" (fresh allocator). The core functionality is provided by
/// default methods, which aren't generally made to be overwritten.
///
/// The reason why these methods aren't implemented directly on the bookkeeper is the distinction
/// between different forms of allocators (global, local, and so on). Any newtype of
/// [`Bookkeeper`](./struct.Bookkeeper.html).
///
/// # Guarantees vs. assumptions
///
/// Please note that whenever a guarantee is mentioned, it relies on that the all the methods
/// overwritten are upholding the guarantees specified in the documentation.
pub trait Allocator: ops::DerefMut<Target = Bookkeeper> {
    /// Allocate _fresh_ space.
    ///
    /// "Fresh" means that the space is allocated through some breaker (be it SBRK or the global
    /// allocator).
    ///
    /// The returned pointer is assumed to be aligned to `align`. If this is not held, all future
    /// guarantees are invalid.
    ///
    /// # Assumptions
    ///
    /// This is assumed to not modify the order. If some block `b` is associated with index `i`
    /// prior to call of this function, it should be too after it.
    fn alloc_fresh(&mut self, size: usize, align: usize) -> Block;

    /// Called right before new memory is added to the pool.
    fn on_new_memory(&mut self) {}

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
    fn alloc(&mut self, size: usize, align: usize) -> Block {
        // Logging.
        bk_log!(self, "Allocating {} bytes with alignment {}.", size, align);

        if let Some((n, b)) = self
            .pool
            .iter_mut()
            .enumerate()
            .filter_map(|(n, i)| {
                if i.size() >= size {
                    // Try to split at the aligner.
                    i.align(align).and_then(|(mut a, mut b)| {
                        if b.size() >= size {
                            // Override the old block.
                            *i = a;
                            Some((n, b))
                        } else {
                            // Put the split block back together and place it back in its spot.
                            a.merge_right(&mut b).expect("Unable to merge block right.");
                            *i = a;
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .next()
        {
            // Update the pool byte count.
            self.total_bytes -= b.size();

            if self.pool[n].is_empty() {
                // For empty alignment invariant.
                let _ = self.remove_at(n);
            }

            // Split and mark the block uninitialized to the debugger.
            let (res, excessive) = b.mark_uninitialized().split(size);

            // There are many corner cases that make knowing where to insert it difficult
            // so we search instead.
            self.free(excessive);

            // Check consistency.
            self.check();
            debug_assert!(res.aligned_to(align), "Alignment failed.");
            debug_assert!(
                res.size() == size,
                "Requested space does not match with the returned \
                 block."
            );

            res
        } else {
            // No fitting block found. Allocate a new block.
            self.alloc_external(size, align)
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
    fn free(&mut self, block: Block) {
        // Just logging for the unlucky people debugging this shit. No problem.
        bk_log!(self, "Freeing {:?}...", block);

        // Binary search for the block.
        let bound = self.find_bound(&block);

        // Free the given block.
        self.free_bound(bound, block);
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
    /// # Example
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
    fn realloc(&mut self, block: Block, new_size: usize, align: usize) -> Block {
        // Find the index bound.
        let ind = self.find_bound(&block);

        // Logging.
        bk_log!(self;ind, "Reallocating {:?} to size {} with align {}...", block, new_size, align);

        // Try to do an inplace reallocation.
        match self.realloc_inplace_bound(ind, block, new_size) {
            Ok(block) => block,
            Err(block) => {
                // Reallocation cannot be done inplace.

                // Allocate a new block with the same size.
                let mut res = self.alloc(new_size, align);

                // Copy the old data to the new location.
                block.copy_to(&mut res);

                // Free the old block.
                // Allocation may have moved insertion so we search again.
                self.free(block);

                // Check consistency.
                self.check();
                debug_assert!(res.aligned_to(align), "Alignment failed.");
                debug_assert!(
                    res.size() >= new_size,
                    "Requested space does not match with the \
                     returned block."
                );

                res
            }
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
    /// [`realloc_inplace_bound`](#method.realloc_inplace_bound.html).
    #[inline]
    fn realloc_inplace(&mut self, block: Block, new_size: usize) -> Result<Block, Block> {
        // Logging.
        bk_log!(self, "Reallocating {:?} inplace to {}...", block, new_size);

        // Find the bounds of given block.
        let bound = self.find_bound(&block);

        // Go for it!
        let res = self.realloc_inplace_bound(bound, block, new_size);

        // Check consistency.
        debug_assert!(
            res.as_ref().ok().map_or(true, |x| x.size() == new_size),
            "Requested space \
             does not match with the returned block."
        );

        res
    }

    /// Reallocate a block on a know index bound inplace.
    ///
    /// See [`realloc_inplace`](#method.realloc_inplace.html) for more information.
    fn realloc_inplace_bound(
        &mut self,
        ind: Range<usize>,
        mut block: Block,
        new_size: usize,
    ) -> Result<Block, Block> {
        // Logging.
        bk_log!(self;ind, "Try inplace reallocating {:?} to size {}.", block, new_size);

        /// Assertions...
        debug_assert!(
            self.find(&block) == ind.start,
            "Block is not inserted at the appropriate \
             index."
        );

        if new_size <= block.size() {
            // Shrink the block.
            bk_log!(self;ind, "Shrinking {:?}.", block);

            // Split the block in two segments, the main segment and the excessive segment.
            let (block, excessive) = block.split(new_size);
            // Free the excessive segment.
            self.free_bound(ind, excessive);

            // Make some assertions to avoid dumb bugs.
            debug_assert!(block.size() == new_size, "Block wasn't shrinked properly.");

            // Run a consistency check.
            self.check();

            return Ok(block);

        // We check if `ind` is the end of the array.
        } else {
            let mut mergable = false;
            if let Some(entry) = self.pool.get_mut(ind.end) {
                mergable = entry.size() + block.size() >= new_size && block.left_to(entry);
            }
            // Note that we are sure that no segments in the array are adjacent (unless they have size
            // 0). This way we know that we will, at maximum, need one and only one block for extending
            // the current block.
            if mergable {
                // Logging...
                bk_log!(self;ind, "Merging {:?} to the right.", block);

                // We'll merge it with the block at the end of the range.
                block
                    .merge_right(&mut self.remove_at(ind.end))
                    .expect("Unable to merge block right, to the end of the range.");
                // Merge succeeded.

                // Place the excessive block back.
                let (res, excessive) = block.split(new_size);
                // Remove_at may have shortened the vector.
                if ind.start == self.pool.len() {
                    self.push(excessive);
                } else if !excessive.is_empty() {
                    self.total_bytes += excessive.size();
                    self.pool[ind.start] = excessive;
                }
                // Block will still not be adjacent, due to `excessive` being guaranteed to not be
                // adjacent to the next block.

                // Run a consistency check.
                self.check();

                return Ok(res);
            }
        }

        Err(block)
    }

    /// Free a block placed in some index bound.
    ///
    /// This will at maximum insert one element.
    ///
    /// See [`free`](#method.free) for more information.
    #[inline]
    fn free_bound(&mut self, ind: Range<usize>, mut block: Block) {
        // Logging.
        bk_log!(self;ind, "Freeing {:?}.", block);

        // Short circuit in case of empty block.
        if block.is_empty() {
            return;
        }

        // When compiled with `security`, we zero this block.
        block.sec_zero();

        if ind.start == self.pool.len() {
            self.push(block);
            return;
        }

        // Assertions...
        debug_assert!(
            self.find(&block) == ind.start,
            "Block is not inserted at the appropriate \
             index."
        );

        // Try to merge it with the block to the right.
        if ind.end < self.pool.len() && block.left_to(&self.pool[ind.end]) {
            // Merge the block with the rightmost block in the range.
            block
                .merge_right(&mut self.remove_at(ind.end))
                .expect("Unable to merge block right to the block at the end of the range");

            // The merging succeeded. We proceed to try to close in the possible gap.
            let size = block.size();
            if ind.start != 0 && self.pool[ind.start - 1].merge_right(&mut block).is_ok() {
                self.total_bytes += size;
            }
            // Check consistency.
            self.check();

            return;
        // Dammit, let's try to merge left.
        } else if ind.start != 0 && self.pool[ind.start - 1].left_to(&block) {
            let size = block.size();
            if self.pool[ind.start - 1].merge_right(&mut block).is_ok() {
                self.total_bytes += size;
            }
            // Check consistency.
            self.check();

            return;
        }

        // Well, it failed, so we insert it the old-fashioned way.
        self.insert(ind.start, block);

        // Check consistency.
        self.check();
    }

    /// Allocate external ("fresh") space.
    ///
    /// "Fresh" means that the space is allocated through the breaker.
    ///
    /// The returned pointer is guaranteed to be aligned to `align`.
    fn alloc_external(&mut self, size: usize, align: usize) -> Block {
        // Logging.
        bk_log!(
            self,
            "Fresh allocation of size {} with alignment {}.",
            size,
            align
        );

        // Break it to me!
        let res = self.alloc_fresh(size, align);

        // Check consistency.
        self.check();

        res
    }

    /// Push an element without reserving.
    // TODO: Make `push` and `free` one.
    fn push(&mut self, block: Block) {
        // Logging.
        bk_log!(self;self.pool.len(), "Pushing {:?}.", block);

        // Mark the block free.
        let mut block = block.mark_free();

        // Short-circuit in case on empty block.
        if !block.is_empty() {
            // Trigger the new memory event handler.
            self.on_new_memory();

            // Update the pool byte count.
            self.total_bytes += block.size();

            // Some assertions...
            debug_assert!(
                self.pool.is_empty() || &block > self.pool.last().unwrap(),
                "Pushing will \
                 make the list unsorted."
            );

            // We will try to simply merge it with the last block.
            if let Some(x) = self.pool.last_mut() {
                if x.merge_right(&mut block).is_ok() {
                    return;
                }
            }

            // Reserve space and free the old buffer.
            if let Some(x) = unborrow!(self.reserve(self.pool.len() + 1)) {
                // Note that we do not set the count down because this isn't setting back our
                // pushed block.

                self.free(x);
            }

            // Try again to merge with last block on the off chance reserve pushed something we can
            // merge with. This has actually happened in testing.
            if let Some(x) = self.pool.last_mut() {
                if x.merge_right(&mut block).is_ok() {
                    return;
                }
            }

            // Merging failed. Note that trailing empty blocks are not allowed, hence the last block is
            // the only non-empty candidate which may be adjacent to `block`.

            // Check again that pushing is correct.
            if self.pool.is_empty() || &block > self.pool.last().unwrap() {
                // We push.
                let res = self.pool.push(block);

                // Make some assertions.
                debug_assert!(res.is_ok(), "Push failed (buffer full).");
            } else {
                // `free` handles the count, so we set it back.
                // TODO: Find a better way to do so.
                self.total_bytes -= block.size();

                // Can't push because reserve changed the end of the pool.
                self.free(block);
            }
        }

        // Check consistency.
        self.check();
    }

    /// Reserve some number of elements, and return the old buffer's block.
    ///
    /// # Assumptions
    ///
    /// This is assumed to not modify the order. If some block `b` is associated with index `i`
    /// prior to call of this function, it should be too after it.
    fn reserve(&mut self, min_cap: usize) -> Option<Block> {
        // Logging.
        bk_log!(self;min_cap, "Reserving {}.", min_cap);

        if !self.reserving
            && (self.pool.capacity() < self.pool.len() + EXTRA_ELEMENTS
                || self.pool.capacity() < min_cap + EXTRA_ELEMENTS)
        {
            // Reserve a little extra for performance reasons.
            // TODO: This should be moved to some new method.
            let new_cap = min_cap + EXTRA_ELEMENTS + config::extra_fresh(min_cap);

            // Catch 'em all.
            debug_assert!(new_cap > self.pool.capacity(), "Reserve shrinks?!");

            // Make sure no unbounded reallocation happens.
            self.reserving = true;

            // Break it to me!
            let new_buf =
                self.alloc_external(new_cap * mem::size_of::<Block>(), mem::align_of::<Block>());

            // Go back to the original state.
            self.reserving = false;

            // Check consistency.
            self.check();

            Some(self.pool.refill(new_buf))
        } else {
            None
        }
    }

    /// Insert a block entry at some index.
    ///
    /// If the space is non-empty, the elements will be pushed filling out the empty gaps to the
    /// right.
    ///
    /// # Warning
    ///
    /// This might in fact break the order.
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
        bk_log!(self;ind, "Inserting block {:?}...", block);

        // Bound check.
        assert!(self.pool.len() >= ind, "Insertion out of bounds.");

        // Some assertions...
        debug_assert!(
            self.pool.len() <= ind || block <= self.pool[ind],
            "Inserting at {} will make \
             the list unsorted.",
            ind
        );
        debug_assert!(
            self.find(&block) == ind,
            "Block is not inserted at the appropriate index."
        );
        debug_assert!(!block.is_empty(), "Inserting an empty block.");

        // Trigger the new memory event handler.
        self.on_new_memory();

        // Find the next gap, where a used block were.
        let gap = self.pool
            .iter()
            .enumerate()
            // We only check _after_ the index.
            .skip(ind)
            // Until the block is empty.
            .filter(|&(_, x)| x.is_empty())
            .next()
            .map(|(n, _)| n);

        // Log the operation.
        bk_log!(self;ind, "Moving all blocks right to {} blocks to the right.",
             gap.unwrap_or_else(|| self.pool.len()));

        // The old vector's buffer.
        let mut old_buf = None;

        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // Memmove the elements to make a gap to the new block.
            ptr::copy(
                self.pool.get_unchecked(ind) as *const Block,
                self.pool.get_unchecked_mut(ind + 1) as *mut Block,
                // The gap defaults to the end of the pool.
                gap.unwrap_or_else(|| {
                    // We will only extend the length if we were unable to fit it into the current length.

                    // Loooooooging...
                    bk_log!(self;ind, "Block pool not long enough for shift. Extending.");

                    // Reserve space. This does not break order, due to the assumption that
                    // `reserve` never breaks order.
                    old_buf = unborrow!(self.reserve(self.pool.len() + 1));

                    // We will move a block into reserved memory but outside of the vec's bounds. For
                    // that reason, we push an uninitialized element to extend the length, which will
                    // be assigned in the memcpy.
                    let res = self.pool.push(mem::uninitialized());

                    // Just some assertions...
                    debug_assert!(res.is_ok(), "Push failed (buffer full).");

                    self.pool.len() - 1
                }) - ind,
            );

            // Update the pool byte count.
            self.total_bytes += block.size();

            // Mark it free and set the element.
            ptr::write(self.pool.get_unchecked_mut(ind), block.mark_free());
        }

        // Free the old buffer, if it exists.
        if let Some(block) = old_buf {
            self.free(block);
        }

        // Check consistency.
        self.check();
    }

    /// Remove a block.
    fn remove_at(&mut self, ind: usize) -> Block {
        // Logging.
        bk_log!(self;ind, "Removing block at {}.", ind);

        let res = if ind + 1 == self.pool.len() {
            let block = self.pool[ind].pop();
            // Make sure there are no trailing empty blocks.
            let new_len =
                self.pool.len() - self.pool.iter().rev().take_while(|x| x.is_empty()).count();

            // Truncate the vector.
            self.pool.truncate(new_len);

            block
        } else {
            // Calculate the upper and lower bound
            let empty = self.pool[ind + 1].empty_left();
            let empty2 = empty.empty_left();

            // Replace the block at `ind` with the left empty block from `ind + 1`.
            let block = mem::replace(&mut self.pool[ind], empty);

            // Iterate over the pool from `ind` and down and set it to the  empty of our block.
            let skip = self.pool.len() - ind;
            for place in self
                .pool
                .iter_mut()
                .rev()
                .skip(skip)
                .take_while(|x| x.is_empty())
            {
                // Empty the blocks.
                *place = empty2.empty_left();
            }

            block
        };

        // Update the pool byte count.
        self.total_bytes -= res.size();

        // Check consistency.
        self.check();

        // Mark the block uninitialized to the debugger.
        res.mark_uninitialized()
    }
}
