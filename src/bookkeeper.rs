//! The memory bookkeeping module.
//!
//! Blocks are the main unit for the memory bookkeeping. A block is a simple construct with a
//! `Unique` pointer and a size. Occupied (non-free) blocks are represented by a zero-sized block.

use block::Block;
use sys;

use core::mem::{align_of, size_of};
use core::{ops, ptr, slice, cmp, intrinsics};
use core::ptr::Unique;

/// An address representing an "empty" or non-allocated value on the heap.
const EMPTY_HEAP: *mut u8 = 0x1 as *mut _;

/// The memory bookkeeper.
///
/// This is the main primitive in ralloc. Its job is to keep track of the free blocks in a
/// structured manner, such that allocation, reallocation, and deallocation are all efficient.
/// Parituclarly, it keeps a list of blocks, commonly called the "block list". This list is kept.
/// Entries in the block list can be "empty", meaning that you can overwrite the entry without
/// breaking consistency.
///
/// For details about the internals, see [`BlockList`](./struct.BlockList.html) (requires the docs
/// to be rendered with private item exposed).
pub struct Bookkeeper {
    /// The internal block list.
    ///
    /// Guarantees
    /// ==========
    ///
    /// Certain guarantees are made:
    ///
    /// 1. The list is always sorted with respect to the block's pointers.
    /// 2. No two blocks overlap.
    /// 3. No two free blocks are adjacent.
    block_list: BlockList,
}

impl Bookkeeper {
    /// Construct a new, empty bookkeeper.
    ///
    /// No allocations or BRKs are done.
    pub fn new() -> Bookkeeper {
        Bookkeeper {
            block_list: BlockList::new(),
        }
    }

    /// Allocate a chunk of memory.
    ///
    /// This function takes a size and an alignment. From these a fitting block is found, to which
    /// a pointer is returned. The pointer returned has the following guarantees:
    ///
    /// 1. It is aligned to `align`: In particular, `align` divides the address.
    /// 2. The chunk can be safely read and written, up to `size`. Reading or writing out of this
    ///    bound is undefined behavior.
    /// 3. It is a valid, unique, non-null pointer, until `free` is called again.
    pub fn alloc(&mut self, size: usize, align: usize) -> Unique<u8> {
        self.block_list.alloc(size, align)
    }

    /// Reallocate memory.
    ///
    /// If necessary it will allocate a new buffer and deallocate the old one.
    ///
    /// The following guarantees are made:
    ///
    /// 1. The returned pointer is valid and aligned to `align`.
    /// 2. The returned pointer points to a buffer containing the same data byte-for-byte as the
    ///    original buffer.
    /// 3. Reading and writing up to the bound, `new_size`, is valid.
    pub fn realloc(&mut self, block: Block, new_size: usize, align: usize) -> Unique<u8> {
        self.block_list.realloc(block, new_size, align)
    }

    /// Free a memory block.
    ///
    /// After this have been called, no guarantees are made about the passed pointer. If it want
    /// to, it could begin shooting laser beams.
    ///
    /// Freeing an invalid block will drop all future guarantees about this bookkeeper.
    pub fn free(&mut self, block: Block) {
        self.block_list.free(block)
    }
}

/// Calculate the aligner.
///
/// The aligner is what we add to a pointer to align it to a given value.
fn aligner(ptr: *mut u8, align: usize) -> usize {
    align - ptr as usize % align
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
fn canonicalize_brk(size: usize) -> usize {
    const BRK_MULTIPLIER: usize = 1;
    const BRK_MIN: usize = 200;
    const BRK_MIN_EXTRA: usize = 10000; // TODO tune this?

    cmp::max(BRK_MIN, size.saturating_add(cmp::min(BRK_MULTIPLIER * size, BRK_MIN_EXTRA)))
}

/// A block list.
///
/// This primitive is used for keeping track of the free blocks.
///
/// Only making use of only [`alloc`](#method.alloc), [`free`](#method.free),
/// [`realloc`](#method.realloc) (and following their respective assumptions) guarantee that no
/// buffer overrun, segfault, arithmetic overflow, or otherwise unexpected crash.
struct BlockList {
    /// The capacity of the block list.
    cap: usize,
    /// The length of the block list.
    len: usize,
    /// The pointer to the first element in the block list.
    ptr: Unique<Block>,
}

impl BlockList {
    /// Create a new, empty block list.
    ///
    /// This will make no allocations or BRKs.
    fn new() -> BlockList {
        BlockList {
            cap: 0,
            len: 0,
            ptr: unsafe { Unique::new(EMPTY_HEAP as *mut _) },
        }
    }

    /// *[See `Bookkeeper`'s respective method.](./struct.Bookkeeper.html#method.alloc)*
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
    /// We then split it at the [aligner](./fn.aligner.html), which is used for making sure that
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
    /// The pointer to the marked area is then returned.
    fn alloc(&mut self, size: usize, align: usize) -> Unique<u8> {
        // This variable will keep block, we will return as allocated memory.
        let mut block = None;

        // We run right-to-left, since new blocks tend to get added to the right.
        for (n, i) in self.iter_mut().enumerate().rev() {
            let aligner = aligner(*i.ptr, align);

            if i.size >= size + aligner {
                // To catch dumb logic errors.
                debug_assert!(i.is_free(), "Block is not free (What the fuck, Richard?)");

                // Use this block as the one, we use for our allocation.
                block = Some((n, Block {
                    size: i.size,
                    ptr: unsafe { Unique::new((*i.ptr as usize + aligner) as *mut _) },
                }));

                // Leave the stub behind.
                if aligner == 0 {
                    // Since the stub is empty, we are not interested in keeping it marked as free.
                    i.set_free();
                } else {
                    i.size = aligner;
                }

                break;
            }
        }

        if let Some((n, b)) = block {
            if b.size != size {
                // Mark the excessive space as free.
                self.insert(n, Block {
                    size: b.size - size,
                    ptr: unsafe { Unique::new((*b.ptr as usize + size) as *mut _) },
                });
            }

            // Check consistency.
            self.check();
            debug_assert!(*b.ptr as usize % align == 0, "Alignment in `alloc` failed.");

            b.ptr
        } else {
            // No fitting block found. Allocate a new block.
            self.alloc_fresh(size, align)
        }
    }

    /// Push to the block list.
    ///
    /// This will append a block entry to the end of the block list. Make sure that this entry has
    /// a value higher than any of the elements in the list, to keep it sorted.
    fn push(&mut self, block: Block) {
        let len = self.len;
        // This is guaranteed not to overflow, since `len` is bounded by the address space, since
        // each entry represent at minimum one byte, meaning that `len` is bounded by the address
        // space.
        self.reserve(len + 1);

        unsafe {
            ptr::write((*self.ptr as usize + size_of::<Block>() * self.len) as *mut _, block);
        }

        self.len += 1;

        // Check consistency.
        self.check();
    }

    /// Find a block's index through binary search.
    ///
    /// If it fails, the value will be where the block could be inserted to keep the list sorted.
    fn search(&mut self, block: &Block) -> Result<usize, usize> {
        self.binary_search_by(|x| x.cmp(block))
    }

    /// Allocate _fresh_ space.
    ///
    /// "Fresh" means that the space is allocated through a BRK call to the kernel.
    fn alloc_fresh(&mut self, size: usize, align: usize) -> Unique<u8> {
        // Calculate the canonical size (extra space is allocated to limit the number of system calls).
        let can_size = canonicalize_brk(size);
        // Get the previous segment end.
        let seg_end = sys::segment_end().unwrap_or_else(|x| x.handle());
        // Calculate the aligner.
        let aligner = aligner(seg_end, align);
        // Use SYSBRK to allocate extra data segment.
        let ptr = sys::inc_brk(can_size.checked_add(aligner).unwrap_or_else(|| sys::oom())).unwrap_or_else(|x| x.handle());

        let alignment_block = Block {
            size: aligner,
            ptr: ptr,
        };
        let res = Block {
            ptr: alignment_block.end(),
            size: size,
        };

        // Add it to the list. This will not change the order, since the pointer is higher than all
        // the previous blocks.
        self.push(alignment_block);

        // Add the extra space allocated.
        self.push(Block {
            // This won't overflow, since `can_size` is bounded by `size`
            size: can_size - size,
            ptr: res.end(),
        });

        // Check consistency.
        self.check();
        debug_assert!(*res.ptr as usize % align == 0, "Alignment in `alloc_fresh` failed.");

        res.ptr
    }

    /// Reallocate inplace.
    ///
    /// This will try to reallocate a buffer inplace, meaning that the buffers length is merely
    /// extended, and not copied to a new buffer.
    ///
    /// Returns `Err(())` if the buffer extension couldn't be done, `Err(())` otherwise.
    ///
    /// The following guarantees are made:
    ///
    /// 1. If this function returns `Ok(())`, it is valid to read and write within the bound of the
    ///    new size.
    /// 2. No changes are made to the allocated buffer itself.
    /// 3. On failure, the state of the allocator is left unmodified.
    fn realloc_inplace(&mut self, ind: usize, size: usize) -> Result<(), ()> {
        // Bound check.
        if ind == self.len { return Err(()) }
        debug_assert!(ind < self.len, "Index out of bound.");

        if self[ind].size < size {
            // `ind` + 1 is guaranteed to not overflow, since it is bounded (by the array bound check)
            // by `self.len`, which is bounded by the address space (it is strictly less than the
            // address space, since each entry is more than one byte).

            // The addition of the sizes are guaranteed not to overflow, due to only being reach if the
            // next block is free, effectively asserting that the blocks are disjoint, implying that
            // their sum is bounded by the address space (see the guarantees).
            if self[ind + 1].is_free() && self[ind].size + self[ind + 1].size >= size {
                // Calculate the additional space.
                //
                // This is guaranteed to not overflow, since the `if` statement's condition implies
                // so.
                let additional = size - self[ind].size;

                // Leave the excessive space.
                self[ind + 1].ptr = unsafe {
                    Unique::new((*self[ind + 1].ptr as usize + additional) as *mut _)
                };
                self[ind + 1].size -= additional;

                // Check consistency.
                self.check();

                Ok(())
            } else {
                Err(())
            }
        } else {
            // Resize our block.
            self[ind].size = size;

            // Calculate the excessive space.
            //
            // This will not overflow due to the negation of the condition in the if statement.
            let rest = self[ind].size - size;
            // Free the excessive part.
            let exc_ptr = self[ind].end();
            self.free(Block {
                size: rest,
                ptr: exc_ptr,
            });

            Ok(())
        }
    }

    /// *[See `Bookkeeper`'s respective method.](./struct.Bookkeeper.html#method.realloc)*
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
    fn realloc(&mut self, block: Block, new_size: usize, align: usize) -> Unique<u8> {
        let ind = self.find(&block);

        if self.realloc_inplace(ind, new_size).is_ok() {
            block.ptr
        } else {
            // Reallocation cannot be done inplace.

            // Allocate a new block with the same size.
            let ptr = self.alloc(new_size, align);

            // Copy the old data to the new location.
            unsafe { ptr::copy(*block.ptr, *ptr, block.size); }

            // Free the old block.
            self.free(block);

            // Check consistency.
            self.check();
            debug_assert!(*ptr as usize % align == 0, "Alignment in `realloc` failed.");

            ptr
        }
    }

    /// Reserve space for the block list.
    ///
    /// This will extend the capacity to a number greater than or equals to `needed`, potentially
    /// reallocating the block list.
    fn reserve(&mut self, needed: usize) {
        if needed > self.cap {
            // Set the new capacity.
            self.cap = cmp::max(30, self.cap.saturating_mul(2));

            // Reallocate the block list.
            self.ptr = unsafe {
                let block = Block {
                    ptr: Unique::new(*self.ptr as *mut _),
                    size: self.cap,
                };

                let cap = self.cap;
                Unique::new(*self.realloc(block, cap, align_of::<Block>()) as *mut _)
            };

            // Check consistency.
            self.check();
        }
        /*
        if needed > self.cap {
            // Set the new capacity.
            self.cap = cmp::minkii(self.cap + 100, self.cap.saturating_mul(2));

            // Reallocate the block list.
            self.ptr = unsafe {
                let block = Block {
                    ptr: Unique::new(*self.ptr as *mut _),
                    size: self.cap,
                };

                let cap = self.cap;
                let ind = self.find(&block);
                Unique::new(*self.realloc_inplace(block, ind, cap) as *mut _)
            };

            // Check consistency.
            self.check();
        }
        */
    }

    /// Perform a binary search to find the appropriate place where the block can be insert or is
    /// located.
    fn find(&mut self, block: &Block) -> usize {
        match self.search(block) {
            Ok(x) => x,
            Err(x) => x,
        }
    }

    /// *[See `Bookkeeper`'s respective method.](./struct.Bookkeeper.html#method.free)*
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
    fn free(&mut self, block: Block) {
        let ind = self.find(&block);

        debug_assert!(*self[ind].ptr != *block.ptr || !self[ind].is_free(), "Double free.");

        // Try to merge right.
        if self[ind].is_free() && ind + 1 < self.len && self[ind].left_to(&block) {
            self[ind].size += block.size;
        // Try to merge left. Note that `self[ind]` is not free, by the conditional above.
        } else if self[ind - 1].is_free() && ind != 0 && self[ind - 1].left_to(&block) {
            self[ind - 1].size += block.size;
        } else {
            self.insert(ind, block);
        }

        // Check consistency.
        self.check();
    }

    /// Insert a block entry at some index.
    ///
    /// If the space is non-empty, the elements will be pushed filling out the empty gaps to the
    /// right. If all places to the right is occupied, it will reserve additional space to the
    /// block list.
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
        // TODO consider moving right before searching left.

        // Find the next gap, where a used block were.
        let n = self.iter()
            .skip(ind)
            .enumerate()
            .filter(|&(_, x)| x.is_free())
            .next().map(|x| x.0)
            .unwrap_or_else(|| {
                let len = self.len;

                // No gap was found, so we need to reserve space for new elements.
                self.reserve(len + 1);
                // Increment the length, since a gap haven't been found.
                self.len += 1;
                len
            });

        // Memmove the blocks to close in that gap.
        unsafe {
            ptr::copy(self[ind..].as_ptr(), self[ind + 1..].as_mut_ptr(), self.len - n);
        }

        // Place the block left to the moved line.
        self[ind] = block;

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
    /// 2. No entries are not overlapping.
    /// 3. The length does not exceed the capacity.
    #[cfg(debug_assertions)]
    fn check(&self) {
        // Check if sorted.
        let mut prev = *self[0].ptr;
        for (n, i) in self.iter().enumerate().skip(1) {
            assert!(*i.ptr > prev, "The block list is not sorted at index, {}.", n);
            prev = *i.ptr;
        }
        // Check if overlapping.
        let mut prev = *self[0].ptr;
        for (n, i) in self.iter().enumerate().skip(1) {
            assert!(!i.is_free() || *i.ptr > prev, "Two blocks are overlapping/adjacent at index, {}.", n);
            prev = *i.end();
        }

        // Check that the length is lower than or equals to the capacity.
        assert!(self.len <= self.cap, "The capacity does not cover the length.");
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use block::Block;

    use core::ptr;

    #[test]
    fn test_alloc() {
        let mut bk = Bookkeeper::new();
        let mem = bk.alloc(1000, 4);

        unsafe {
            ptr::write(*mem as *mut _, [1u8; 1000]);
        }

        bk.free(Block {
            size: 1000,
            ptr: mem,
        });
    }
}

impl ops::Deref for BlockList {
    type Target = [Block];

    fn deref(&self) -> &[Block] {
        unsafe {
            slice::from_raw_parts(*self.ptr as *const _, self.len)
        }
    }
}
impl ops::DerefMut for BlockList {
    fn deref_mut(&mut self) -> &mut [Block] {
        unsafe {
            slice::from_raw_parts_mut(*self.ptr, self.len)
        }
    }
}
