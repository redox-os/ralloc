use core::{cmp, mem};

use arena::Arena;
use bk::search::{self, Search};
use ptr;
use random;

struct Pool {
    head: Node,
    arena: Arena<Node>,
}

impl Pool {
    pub fn update(&mut self) {
        /// The threshold to be reached before a refill happens.
        const REFILL_THRESHOLD: arena::Length = arena::Length(2);
        /// The nodes which will be added during a refill.
        const REFILL_NODES: usize = 64;

        if self.arena.len() < REFILL_THRESHOLD {
            // The length was below the refill threshold. To avoid infinite recursion (which could
            // happen as a result of a refill-when-empty policy, due to the refill itself needing
            // to allocate nodes), we have a threshold which symbolizes the maximum amount of arena
            // allocation that can happen inbetween the `update` calls.

            // Allocate the block to provide to the arena.
            let alloc = self.alloc(REFILL_NODES * mem::size_of::<Node>(), mem::align_of::<Node>());
            // Cast the block to a pointer and hand it over to the arena.
            self.arena.refill(ptr::Uninit::new(ptr::Pointer::from(alloc).cast()));
        }
    }

    /// Search the block pool with a particular searcher.
    ///
    /// The outline of the algorithm is this: We start by shortcutting from the top level until we
    /// need to refine (which is determined by the serarcher), then we repeat on the next level
    /// starting at the last refined shortcut from the previous level. At the lowest level, we go
    /// forward until we find a match.
    ///
    /// A "lookback", i.e. the refined nodes of every level is stored in the returned value.
    ///
    /// # Example
    ///
    /// If we look for block 8, we start in the top level and follow until we hit 9.
    ///     ==================> [6] --- overshoot ----> [9] -----------> NIL
    ///     ------------------> [6] ==> [7] ----------> [9] -----------> NIL
    ///     ----------> [5] --> [6] ==> [7] ----------> [9] --> [10] --> NIL
    ///     --> [1] --> [5] --> [6] --> [7] ==> [8] --> [9] --> [10] --> NIL
    pub fn search<S: Search>(&mut self, searcher: S) -> Result<Seek, ()> {
        // We start by an uninitialized value, which we fill out.
        let mut seek = unsafe { mem::uninitialized() };

        // Start at the highest (least dense) level.
        let mut iter = self.head.follow_shortcut(lv::Level::max());
        // Go forward until we can refine (e.g. we overshoot).
        while let Some(shortcut_taken) = iter.find(|x| searcher.refine(x)) {
            // Decrement the level.
            let lv::Level(lv) = iter.decrement_level();
            log!(INTERNAL, "Going from level {} to level {}.", lv, lv - 1);

            // Update the back look respectively.
            seek.back_look[lv] = shortcut_taken;

            // End the loop at the last level.
            if lv == 1 {
                // We decremented the level previously, and given that our old level is one, the
                // new level is zero.

                log!(INTERNAL, "We're at the last level now.");

                break;
            }
        }

        // We're now at the bottom layer, and we need to find a match by iterating over the nodes
        // of this layer.
        if let Some(shortcut) = iter.next() {
            if let Some(found) = shortcut.node.iter().find(|x| searcher.is_match(x)) {
                // Set the seek's found node to the first match (as defined by the searcher).
                seek.node = found;
            } else {
                // No match was found, return error.
                return Err(());
            }
        } else {
            // We reached the end of iterator.
            return Err(());
        }

        seek.check();

        // Everything have been initialized, including all the back look cells (i.e. every level
        // have been visited).
        Ok(seek)
    }
}

// Here is a rare Ferris to cheer you up.
//          |
//        \ _ /
//      -= (_) =-
//        /   \         _\/_ _\/_
//          |           //o\ /o\\
//   _____ _ __ __ _______|____|___________
// =-=-_-__=_-= _=_=-=_,-'|_   |_  ()    ()
//  =- _=-=- -_=-=_,-     "-   "-   \/\/\/
//    =- =- -=.--"                  &_^^_&
//                                  \    /
// Don't share beach crab or it will lose
// its value.
