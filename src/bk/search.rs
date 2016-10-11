//! Primitives for searching the block pool.

use bk::node::Node;
use bk::shortcut::Shortcut;
use block::Block;

pub trait Search {
    /// Determine if this shortcut skips the needle.
    ///
    /// If this shortcut spans the needle (target), this method should return `true`. This is used
    /// to refine shortcuts. If a shortcut is determined to not skip the needle, we will simply
    /// progress to the next shortcut and not the lower one.
    fn refine(self, shortcut: &Shortcut) -> bool;
    /// Determine if some node is a match.
    ///
    /// This is used at the bottom layer to determine if we have completeed our search yet.
    fn is_match(self, node: &Node) -> bool;
}

#[derive(Copy, Clone)]
pub struct BlockSearcher<'a> {
    /// The target block.
    needle: &'a Block,
}

impl<'a> Search for BlockSearcher {
    fn refine(self, shortcut: &Shortcut) -> bool {
        if let Some(next) = shortcut.next {
            // We refine if the next block is above our needle, and hence not satisfying our
            // search condition.
            next.block > self.needle
        } else {
            // If the shortcut has no successor, we have to refine.
            true
        }
    }

    fn is_match(self, node: &Node) -> bool {
        if let Some(next) = node.next {
            // We refine if the next block is above our needle, and hence not satisfying our
            // search condition.
            next.block > self.needle
        } else {
            // If the shortcut has no successor (i.e. it is the last block in the pool), we have to
            // refine.
            true
        }
    }
}
