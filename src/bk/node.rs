//! Bookkeeping nodes.
//!
//! This module provides the basic unit in the bookkeeper, the nodes.

/// A block list node.
///
/// A node consists of three components:
///
/// 1. The inner value that the node holds.
/// 2. A pointer to the next node.
/// 3. A stack of so called "shortcuts", which contains data about jumping over/searching for
///    nodes.
struct Node {
    /// The inner block.
    ///
    /// This should never be empty (zero-sized).
    block: Block,
    /// The node that follows this node.
    ///
    /// This cannot be adjacent (tangent) to `self.block`. It is important to maintain the blocks
    /// as long as possible, and hence merge if that is the case.
    ///
    /// `None` indicates that the node is the last node in the list.
    next: Option<Jar<Node>>,
    /// Shortcuts/jumps of the current node.
    ///
    /// This is a stack of linked list nodes, such that any entry has a list which is a superset of
    /// the latter. The lowest layer is a subset of the block list itself.
    ///
    ///     ...
    ///     2      # ---------------------> [6] ---------------------> [9] -------------> NIL
    ///     1      # ---------------------> [6] ---> [7] ------------> [9] -------------> NIL
    ///     0      # ------------> [5] ---> [6] ---> [7] ------------> [9] ---> [10] ---> NIL
    ///     bottom # ---> [1] ---> [5] ---> [6] ---> [7] ---> [8] ---> [9] ---> [10] ---> NIL
    ///
    /// As a result the lowest entry is the most dense.
    ///
    /// If we assume our node is `[6]`, the stack would contain shotcuts to 7, 7, and 8, in that
    /// order. The rest would simply be null pointers with fat value 0.
    ///
    /// # Height
    ///
    /// The index of the highest null shortcut (or, if none, the length of the array) is called the
    /// height of the node.
    shortcuts: lv::Array<Shortcut>,
}

impl Jar<Node> {
    /// Insert a new node after this node.
    fn insert(&mut self, new_node: Jar<Node>) {
        // We move out of the pointer temporarily in order to restructure the list.
        take::replace_with(self, |node| {
            // Place the old node next to the new node.
            new_node.next = Some(node);
            // Set the new node in the old node's previous place.
            new_node
        });
    }
}

impl Node {
    /// Create an iterator over the nodes.
    ///
    /// This iterator starts at `self` and go to `self.next` until it is `None`.
    // TODO: Implement `IntoIterator`.
    fn iter(&mut self) -> impl Iterator<Item = &Node> {
        NodeIter {
            node: Some(self),
        }
    }

    /// Create an iterator following the `lv`'th shortcut.
    fn follow_shortcut(&self, lv: shotcut::Level) -> impl Iterator<Item = Shortcut> {
        ShortcutIter {
            lv: lv,
            node: Some(self),
        }
    }

    /// An iterator over the shortcuts of this node.
    ///
    /// It starts with the lowest (densest) layer's shortcut and progress upwards.
    fn shortcuts(&self) -> impl Iterator<Item = Shortcut> {
        self.shortcuts.iter().take_while(|x| !x.is_null())
    }

    /// Calculate the fat value at some level based on the level below and the inner block's size.
    ///
    /// This will simply traverse the layer below (given in the form of an iterator) and find the
    /// maximal fat value. The result is guaranteed to be equal to or greater than
    /// `self.block.size()`.
    fn calculate_fat_value<I>(&self, lv: lv::Level, below: I) -> block::Size
        where I: Iterator<Item = &Node> {
        // We start at the block's size.
        let mut new_fat = 0;

        // The current skip at `lv`.
        let shortcut = &self.shortcuts[lv];
        // Avoid multiple checking branches for the sake of performance.
        let next_node = shortcut.next.get().unwrap();

        // Follow the shortcuts until we reach `new_node`.
        // TODO: Unroll the first iteration of the loop below to avoid the unneccesary
        //       branch in the first iteration's call of `cmp::max`.
        for i in below {
            new_fat = cmp::max(i.fat, new_fat);

            // Check if the next node isn't reached yet.
            if i == next_node {
                break;
            }

            // We short-circuit in case we reached the old fat value, since the nodes
            // are bounded by this size and thus no bigger nodes are to be found.
            if new_fat == shortcut.fat {
                break;
            }

            // A note on code style: While it would be more idiomatic to make the above two
            // conditionals above into a `take_while` iterator. Unfortunately, this
            // introduces two (!) branches. Iterator adapters are not as zero-cost as
            // everybody claims.
        }

        new_fat
    }

    /// Calculate the fat value of a non bottom layer.
    pub fn calculate_fat_value_non_bottom(&self, lv: lv::NonBottomLevel) -> block::Size {
        // Since `lv != 0` decrementing will not underflow.
        self.calculate_fat_value(lv, self.shortcuts[lv.below()].follow_shortcut(lv.below()))
    }

    /// Calculate the fat value of the lowest level.
    pub fn calculate_fat_value_bottom(&self) -> block::Size {
        // We base the new fat value of the lowest layer on the block list.
        self.calculate_fat_value(lv::Level::min(), self.iter());
    }

    /// Check that this structure satisfy its invariants.
    ///
    /// This is NOP in release mode (`debug_assertions` disabled).
    #[inline]
    fn check(&self) {
        // We only do the check when `debug_assertions` is set, since it is rather expensive and
        // requires traversal of the whole list.
        if cfg!(debug_assertions) {
            // Check against empty blocks.
            assert!(!self.block.is_empty(), "Node's block {:?} is empty (zero sized)", self.block);

            if let Some(next) = self.next {
                // First, make sure that our node is sorted with respect to the next node.
                assert!(next > self.block, "Node holding block {:?} is not sorted wrt. the next \
                        block {:?}", self.block, next);

                // The nodes may never be adjacent. If they are, a merge have been missed.
                assert!(!self.block.left_to(next), "Node's block {:?} adjacent to the next node's \
                        block {:?}", self.block, next);
            }

            // WHO'S A GOOD BOY???
            //                .--~~,__
            //   :-....,-------`~~'._.'
            //    `-,,,  ,_      ;'~U'
            //     _,-' ,'`-__; '--.
            //    (_/'~~      ''''(;

            // FIXME: The short-circuit in `calculate_fat_value` makes the check incomplete, if a
            //        larger element follows.

            // Check the fat value of the bottom level.
            assert!(self.shortcuts[0].fat == self.calculate_fat_value_bottom(), "The bottom layer's \
                    fat value does not match the calculated fat value.");

            // Check the fat values of the non bottom level.
            for lv in lv::Iter::non_bottom() {
                assert!(self.shortcuts[lv.into()].fat == self.calculate_fat_value_non_bottom(lv), "The \
                        bottom layer's fat value does not match the calculated fat value.");
            }

            // Check that the shortcut refers to a node with appropriate (equal to or greater)
            // height.
            // FIXME: Fold this loop to the one above.
            for lv in lv::Iter::all() {
                assert!(!self.shortcuts[lv.into()].next.shortcuts[lv.into()].is_null(), "Shortcut \
                        points to a node with a lower height. Is this a dangling pointer?");
            }
        }
    }
}

/// An iterator over the trailing nodes.
struct NodeIter<'a> {
    /// The next node of this iterator.
    ///
    /// If there is another element, it will be returned on next iteration. If not, this field is
    /// `None` and the iterator is over.
    node: Option<&'a mut Node>,
}

impl<'a> Iterator for NodeIter<'a> {
    type Item = &'a mut Node;

    fn next(&mut self) -> &'a mut Node {
        // Replace `self.node` by the next shortcut, and return the old value.
        mem::replace(&mut self.node, self.node.and_then(|x| &mut x.next))
    }
}
