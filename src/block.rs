//! Memory primitives.

use std::{ops, cmp};
use std::ptr::Unique;

/// A contigious memory block.
pub struct Block {
    /// The size of this block, in bytes.
    pub size: usize,
    /// The pointer to the start of this block.
    pub ptr: Unique<u8>,
}

impl Block {
    /// Get a pointer to the end of this block, not inclusive.
    pub fn end(&self) -> Unique<u8> {
        unsafe {
            Unique::new((self.size + *self.ptr as usize) as *mut _)
        }
    }

    /// Is this block is left to `to`?
    pub fn left_to(&self, to: &Block) -> bool {
        self.size + *self.ptr as usize == *to.ptr as usize
    }
}

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Block) -> Option<cmp::Ordering> {
        self.ptr.partial_cmp(&other.ptr)
    }
}

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

/// A block entry.
///
/// A block entry is a wrapper around `Block` containing an extra field telling if this block is
/// free or not.
#[derive(PartialEq, Eq)]
pub struct BlockEntry {
    /// The underlying block.
    block: Block,
    /// Is this block free?
    pub free: bool,
}

impl ops::Deref for BlockEntry {
    type Target = Block;

    fn deref(&self) -> &Block {
        &self.block
    }
}

impl ops::DerefMut for BlockEntry {
    fn deref_mut(&mut self) -> &mut Block {
        &mut self.block
    }
}

impl PartialOrd for BlockEntry {
    fn partial_cmp(&self, other: &BlockEntry) -> Option<cmp::Ordering> {
        self.block.partial_cmp(other)
    }
}

impl Ord for BlockEntry {
    fn cmp(&self, other: &BlockEntry) -> cmp::Ordering {
        self.block.cmp(other)
    }
}

impl From<Block> for BlockEntry {
    fn from(block: Block) -> BlockEntry {
        BlockEntry {
            block: block,
            free: true,
        }
    }
}
