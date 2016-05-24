//! Memory blocks.
//!
//! Blocks are the main unit for the memory bookkeeping. A block is a simple construct with a
//! `Pointer` pointer and a size. Occupied (non-free) blocks are represented by a zero-sized block.

use core::{ptr, cmp, mem, fmt};

use ptr::Pointer;
use sys;

/// A contiguous memory block.
///
/// This provides a number of guarantees,
///
/// 1. The inner pointer is never aliased. No byte in the block is contained in another block
///    (aliased in this case is defined purely based on liveliness).
/// 2. The buffer is valid, but not necessarily initialized.
///
/// All this is enforced through the type system.
pub struct Block {
    /// The size of this block, in bytes.
    size: usize,
    /// The pointer to the start of this block.
    ptr: Pointer<u8>,
}

impl Block {
    /// Create an empty block starting at `ptr`.
    pub fn empty(ptr: &Pointer<u8>) -> Block {
        Block {
            size: 0,
            // This won't alias `ptr`, since the block is empty.
            ptr: unsafe { Pointer::new(**ptr) },
        }
    }

    /// Construct a block from its raw parts (pointer and size).
    pub unsafe fn from_raw_parts(ptr: Pointer<u8>, size: usize) ->  Block {
        Block {
            size: size,
            ptr: ptr,
        }
    }

    /// Get the size of the block.
    pub fn size(&self) -> usize {
        self.size
    }

    /// BRK allocate a block.
    ///
    /// This is unsafe due to the allocator assuming that only it makes use of BRK.
   pub unsafe fn brk(size: usize) -> Block {
        Block {
            size: size,
            ptr: sys::inc_brk(size).unwrap_or_else(|x| x.handle()),
        }
    }

    /// Merge this block with a block to the right.
    ///
    /// This will simply extend the block, adding the size of the block, and then set the size to
    /// zero. The return value is `Ok(())` on success, and `Err(())` on failure (e.g., the blocks
    /// are not adjacent).
    pub fn merge_right(&mut self, block: &mut Block) -> Result<(), ()> {
        if self.left_to(&block) {
            // Since the end of `block` is bounded by the address space, adding them cannot
            // overflow.
            self.size += block.pop().size;
            // We pop it to make sure it isn't aliased.
            Ok(())
        } else { Err(()) }
    }

    /// Is this block empty/free?
    pub fn is_empty(&self) -> bool {
        self.size != 0
    }

    /// Is this block aligned to `align`?
    pub fn aligned_to(&self, align: usize) -> bool {
        *self.ptr as usize % align == 0
    }

    /// Get the inner pointer.
    pub fn into_ptr(self) -> Pointer<u8> {
        self.ptr
    }

    /// memcpy the block to another pointer.
    ///
    /// # Panics
    ///
    /// This will panic if the target block is smaller than the source.
    pub fn copy_to(&self, block: &mut Block) {
        // Bound check.
        assert!(self.size <= block.size, "Block too small.");

        unsafe {
            ptr::copy_nonoverlapping(*self.ptr, *block.ptr, self.size);
        }
    }

    /// "Pop" this block.
    ///
    /// This marks it as free, and returns the old value.
    pub fn pop(&mut self) -> Block {
        let empty = Block::empty(&self.ptr);
        mem::replace(self, empty)
    }

    /// Is this block placed left to the given other block?
    pub fn left_to(&self, to: &Block) -> bool {
        // This won't overflow due to the end being bounded by the address space.
        self.size + *self.ptr as usize == *to.ptr as usize
    }

    /// Split the block at some position.
    ///
    /// # Panics
    ///
    /// Panics if `pos` is out of bound.
    pub fn split(self, pos: usize) -> (Block, Block) {
        assert!(pos <= self.size, "Split {} out of bound (size is {})!", pos, self.size);

        (
            Block {
                size: pos,
                ptr: self.ptr.duplicate(),
            },
            Block {
                size: self.size - pos,
                ptr: unsafe { self.ptr.offset(pos as isize) },
            }
        )
    }

    /// Split this block, such that the second block is aligned to `align`.
    ///
    /// Returns an `None` holding the intact block if `align` is out of bounds.
    pub fn align(&mut self, align: usize) -> Option<(Block, Block)> {
        let aligner = align - *self.ptr as usize % align;

        // Bound check.
        if aligner < self.size {
            // Invalidate the old block.
            let old = self.pop();

            Some((
                Block {
                    size: aligner,
                    ptr: old.ptr.duplicate(),
                },
                Block {
                    size: old.size - aligner,
                    ptr: unsafe { old.ptr.offset(aligner as isize) },
                }
            ))
        } else { None }
    }
}

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Block) -> Option<cmp::Ordering> {
        self.ptr.partial_cmp(&other.ptr)
    }
}

/// Compare the blocks address.
impl Ord for Block {
    fn cmp(&self, other: &Block) -> cmp::Ordering {
        self.ptr.cmp(&other.ptr)
    }
}

impl cmp::PartialEq for Block {
    fn eq(&self, other: &Block) -> bool {
        self.size == other.size && *self.ptr == *other.ptr
    }
}

impl cmp::Eq for Block {}

impl fmt::Debug for Block {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:x}[0x{:x}]", *self.ptr as usize, self.size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use ptr::Pointer;

    #[test]
    fn test_array() {
        let arr = b"Lorem ipsum dolor sit amet";
        let block = unsafe {
            Block::from_raw_parts(Pointer::new(arr.as_ptr() as *mut u8), arr.len())
        };

        // Test split.
        let (mut lorem, mut rest) = block.split(5);
        assert_eq!(lorem.size(), 5);
        assert_eq!(lorem.size() + rest.size(), arr.len());
        assert!(lorem < rest);

        /* TODO
        assert_eq!(unsafe {
            slice::from_raw_parts(*lorem.into_ptr() as *const _, lorem.size())
        }, b"Lorem");
        */

        assert_eq!(lorem, lorem);
        assert!(rest.is_empty());
        assert!(lorem.align(2).unwrap().1.aligned_to(2));
        assert!(rest.align(16).unwrap().1.aligned_to(16));
        assert_eq!(*lorem.into_ptr() as usize + 5, *rest.into_ptr() as usize);
    }

    #[test]
    fn test_merge() {
        let arr = b"Lorem ipsum dolor sit amet";
        let block = unsafe {
            Block::from_raw_parts(Pointer::new(arr.as_ptr() as *mut u8), arr.len())
        };

        let (mut lorem, mut rest) = block.split(5);
        lorem.merge_right(&mut rest).unwrap();
    }

    #[test]
    #[should_panic]
    fn test_oob() {
        let arr = b"lorem";
        let block = unsafe {
            Block::from_raw_parts(Pointer::new(arr.as_ptr() as *mut u8), arr.len())
        };

        // Test OOB.
        block.split(6);
    }
}
