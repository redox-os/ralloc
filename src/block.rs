use std::{ops, cmp};
use std::ptr::Unique;

pub struct Block {
    pub size: usize,
    pub ptr: Unique<u8>,
}

impl Block {
    pub unsafe fn end(&self) -> Unique<u8> {
        Unique::new((self.size + *self.ptr as usize) as *mut _)
    }

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
    fn eq(&self, _: &Block) -> bool {
        false
    }
}

impl cmp::Eq for Block {}

#[derive(PartialEq, Eq)]
pub struct BlockEntry {
    block: Block,
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

impl<'a> ops::AddAssign<&'a mut BlockEntry> for BlockEntry {
    fn add_assign(&mut self, rhs: &mut BlockEntry) {
        self.size += rhs.size;
        // Free the other block.
        rhs.free = false;

        debug_assert!(self.left_to(&rhs));
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
