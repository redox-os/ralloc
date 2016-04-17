use block::{BlockEntry, Block};
use sys;

use std::mem::{align_of, size_of};
use std::{ops, ptr, slice, cmp};
use std::ptr::Unique;

pub struct BlockList {
    cap: usize,
    len: usize,
    ptr: Unique<BlockEntry>,
}

fn aligner(ptr: *mut u8, align: usize) -> usize {
    align - ptr as usize % align
}

fn canonicalize_brk(size: usize) -> usize {
    const BRK_MULTIPLIER: usize = 1;
    const BRK_MIN: usize = 200;
    const BRK_MIN_EXTRA: usize = 500;

    cmp::max(BRK_MIN, size + cmp::min(BRK_MULTIPLIER * size, BRK_MIN_EXTRA))
}

impl BlockList {
    pub fn alloc(&mut self, size: usize, align: usize) -> Unique<u8> {
        let mut ins = None;

        for (n, i) in self.iter_mut().enumerate() {
            let aligner = aligner(*i.ptr, align);

            if i.size - aligner >= size {
                // Set the excessive space as free.
                ins = Some((n, Block {
                    size: i.size - aligner - size,
                    ptr: unsafe { Unique::new((*i.ptr as usize + aligner + size) as *mut _) },
                }));

                // Leave the stub behind.
                if aligner == 0 {
                    i.free = false;
                } else {
                    i.size = aligner;
                }
            }
        }

        if let Some((n, b)) = ins {
            let res = unsafe {
                Unique::new((*b.ptr as usize - size) as *mut _)
            };

            if b.size != 0 {
                self.insert(n, b.into());
            }

            res
        } else {
            // No fitting block found. Allocate a new block.
            self.alloc_new(size, align)
        }
    }

    fn push(&mut self, block: BlockEntry) {
        let len = self.len;
        self.reserve(len + 1);

        unsafe {
            ptr::write((*self.ptr as usize + self.len * size_of::<Block>()) as *mut _, block);
        }
    }

    fn search(&mut self, block: &Block) -> Result<usize, usize> {
        self.binary_search_by(|x| (**x).cmp(block))
    }

    fn alloc_new(&mut self, size: usize, align: usize) -> Unique<u8> {
        // Calculate the canonical size (extra space is allocated to limit the number of system calls).
        let can_size = canonicalize_brk(size);
        // Get the previous segment end.
        let seg_end = sys::segment_end().unwrap_or_else(|x| x.handle());
        // Use SYSBRK to allocate extra data segment.
        let ptr = sys::inc_brk(can_size + aligner(seg_end, align)).unwrap_or_else(|x| x.handle());

        let res = unsafe {
            Unique::new((*ptr as usize + align) as *mut _)
        };
        let extra = unsafe {
            Unique::new((*res as usize + size) as *mut _)
        };

        // Add it to the list. This will not change the order, since the pointer is higher than all
        // the previous blocks.
        self.push(Block {
            size: align,
            ptr: ptr,
        }.into());

        // Add the extra space allocated.
        self.push(Block {
            size: can_size - size,
            ptr: extra,
        }.into());

        res
    }

    fn realloc_inplace(&mut self, ind: usize, old_size: usize, size: usize) -> Result<(), ()> {
        if ind == self.len - 1 { return Err(()) }

        let additional = old_size - size;

        if old_size + self[ind + 1].size >= size {
            // Leave the excessive space.
            self[ind + 1].ptr = unsafe {
                Unique::new((*self[ind + 1].ptr as usize + additional) as *mut _)
            };
            self[ind + 1].size -= additional;

            // Set the excessive block as free if it is empty.
            if self[ind + 1].size == 0 {
                self[ind + 1].free = false;
            }

            Ok(())
        } else {
            Err(())
        }
    }

    pub fn realloc(&mut self, block: Block, new_size: usize, align: usize) -> Unique<u8> {
        let ind = self.find(&block);

        if self.realloc_inplace(ind, block.size, new_size).is_ok() {
            block.ptr
        } else {
            // Reallocation cannot be done inplace.

            // Allocate a new block with the same size.
            let ptr = self.alloc(new_size, align);

            // Copy the old data to the new location.
            unsafe { ptr::copy(*block.ptr, *ptr, block.size); }

            // Free the old block.
            self.free(block);

            ptr
        }
    }

    fn reserve(&mut self, needed: usize) {
        if needed > self.cap {
            // Reallocate the block list.
            self.ptr = unsafe {
                let block = Block {
                    ptr: Unique::new(*self.ptr as *mut _),
                    size: self.cap,
                };

                Unique::new(*self.realloc(block, needed * 2, align_of::<Block>()) as *mut _)
            };
            // Update the capacity.
            self.cap = needed * 2;
        }
    }

    fn find(&mut self, block: &Block) -> usize {
        match self.search(block) {
            Ok(x) => x,
            Err(x) => x,
        }
    }

    pub fn free(&mut self, block: Block) {
        let ind = self.find(&block);

        // Try to merge left.
        if ind != 0 && self[ind - 1].left_to(&block) {
            self[ind - 1].size += block.size;
        // Try to merge right.
        } else if ind < self.len - 1 && self[ind].left_to(&block) {
            self[ind].size += block.size;
        } else {
            self.insert(ind, block.into());
        }
    }

    fn insert(&mut self, ind: usize, block: BlockEntry) {
        let len = self.len;

        // Find the next gap, where a used block were.
        let n = self.iter()
            .skip(ind)
            .enumerate()
            .filter(|&(_, x)| x.free)
            .next().map(|x| x.0)
            .unwrap_or_else(|| {
                // No gap was found, so we need to reserve space for new elements.
                self.reserve(len + 1);
                ind
            });

        // Memmove the blocks to close in that gap.
        unsafe {
            ptr::copy(self[ind..].as_ptr(), self[ind + 1..].as_mut_ptr(), self.len - n);
        }

        // Place the block left to the moved line.
        self[ind] = block;
        self.len += 1;
    }
}

impl ops::Deref for BlockList {
    type Target = [BlockEntry];

    fn deref(&self) -> &[BlockEntry] {
        unsafe {
            slice::from_raw_parts(*self.ptr as *const _, self.len)
        }
    }
}
impl ops::DerefMut for BlockList {
    fn deref_mut(&mut self) -> &mut [BlockEntry] {
        unsafe {
            slice::from_raw_parts_mut(*self.ptr, self.len)
        }
    }
}
