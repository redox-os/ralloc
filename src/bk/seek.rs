/// A "seek".
///
/// Seek represents the a found node ("needle") and backtracking information ("lookback").
struct Seek<'a> {
    /// The last shortcut below our target for each level.
    ///
    /// This is used for insertions and other backtracking.
    ///
    /// The lower indexes are the denser layers.
    ///
    /// # An important note!
    ///
    /// It is crucial that the backlook is pointers to shortcuts _before_ the target, not shortcuts
    /// starting at the target.
    ///
    /// # Example
    ///
    /// Consider if we search for 8. Now, we move on until we overshoot. The node before the
    /// overshot is a skip (marked with curly braces).
    ///
    ///     ...
    ///     2      # ---------------------> {6} ---------------------> [9] -------------> NIL
    ///     1      # ---------------------> [6] ---> {7} ------------> [9] -------------> NIL
    ///     0      # ------------> [5] ---> [6] ---> {7} ---> [8] ---> [9] ---> [10] ---> NIL
    ///     bottom # ---> [1] ---> [5] ---> [6] ---> [7] ---> [8] ---> [9] ---> [10] ---> NIL
    ///
    /// So, the lookback of this particular seek is `[6, 7, 7, ...]`.
    // FIXME: Find a more rustic way than raw pointers.
    lookback: [Pointer<Shortcuts>; LEVELS.0],
    /// A pointer to a pointer to the found node.
    ///
    /// This is the node equal to or less than the target. The two layers of pointers are there to
    /// make it possible to modify the links and insert nodes before our target.
    node: &'a mut Jar<Node>,
}

impl<'a> Seek<'a> {
    /// Update the fat values of this seek to be higher than or equal to some new block size.  .
    ///
    /// This will simply run over and update the fat values of shortcuts above some level with a
    /// new size.
    ///
    /// Note that this cannot be used to remove a block, since that might decrease some fat values.
    #[inline]
    fn increase_fat(&mut self, size: block::Size, above: shortcut::Level) {
        // Go from the densest layer and up, to update the fat node values.
        for i in self.lookback.iter_mut().skip(above) {
            if !i.increase_fat(size) {
                // Short-circuit for performance reasons.
                break;
            }
        }
    }

    fn put(&mut self, block: Block, arena: &mut Arena<Node>) {
        if self.node.block.merge_right(block).is_ok() {
            // Merge suceeded:
            //               [==block==]
            //     [==self==]            [==rest==]
            // is now:
            //     [==self=============] [==rest==]

            // Update the fat values.
            self.increase_fat(self.node.block.size(), 0);

            // If our configuration now looks like:
            //     [==self=============][==rest==]
            // We need to maximize the former level, so we merge right.
            if self.try_merge_right(arena).is_ok() { return; }

        // Note that we do not merge our block to the seeked block from the left side. This is due
        // to the seeked block being strictly less than our target, and thus we go one forward to
        // the next block to see if it is mergable by its left side.
        } else if self.node.next.and_then(|x| block.merge_right(x)).is_ok() {
            // Merge suceeded:
            //                [==block==]
            //     [==self==]            [==right==]
            // is now:
            //     [==self==] [==right=============]

            // Update the fat values.
            self.increase_fat(block.size(), 0);

            // In case that our new configuration looks like:
            //     [==self==][==right=============]
            // We need to merge the former block right:
            self.try_merge_right(arena);
        } else {
            self.insert_no_merge(block, arena);
        }
    }

    fn try_merge_right(&mut self, arena: &mut Arena<Node>) {
        if self.node.block.merge_right(self.node.next.block).is_ok() {
            // We merged our node left. This means that the node is simply extended and an empty
            // block will be left right to the node. As such the fat node's size is greater than or
            // equal to the new, merged node's size. So we need not full reevaluation of the fat
            // values, instead we can simply climb upwards and max the new size together.
            for (i, shortcut) in self.lookback.iter().zip(i.next.shortcuts) {
                // Update the shortcut to skip over the next shortcut. Note that this statements
                // makes it impossible to shortcut like in `insert`.
                i.next = shortcut.next;
                // Update the fat value to make sure it is greater or equal to the new block size.
                // Note that we do not short-circuit -- on purpose -- due to the above statement
                // being needed for correctness.
                i.increase_fat(self.node.block.size(), block::Size(0));

                // TODO: Consider breaking this loop into two loops to avoid too many fat value
                //       updates.
            }

            // Finally, replace the useless node, and free it to the arena.
            arena.free(mem::replace(self.node.next, self.node.next.unwrap().next));
        }
    }

    // Put a new shortcut starting at the current node at some level.
    //
    // This will simply insert a shortcut on level `lv`, spanning `self.node` to the end of the old
    // shortcut, which is returned.
    //
    // The old shortcut is updated to point to `self.node`, but the fat value is kept as-is.
    //
    // The new shortcut's fat value will be set to the block's size, and recomputation is likely
    // needed to update it.
    fn update_shortcut(&mut self, lv: shortcut::Level) -> Pointer<Shortcut> {
        // Make the old shortcut point to `self.node`.
        let old_next = mem::replace(&mut self.lookback[lv].next, Some(self.node));
        mem::replace(&mut self.lookback[lv], Shortcut {
            next: old_next,
            fat: self.node.block.size(),
        });
    }

    /// Insert a block (no merge) _after_ the found node.
    ///
    /// This will simply insert (place) the node after our found node, without merges. The fat
    /// values are updated.
    fn insert_no_merge(&mut self, block: Block, arena: &mut Arena<Node>) {
        take::replace_with(self, |mut seek| {
            // Make sure that there are no adjacent blocks.
            debug_assert!(!self.node.left_to(&block), "Inserting left-adjacent block without \
                          merge.");
            debug_assert!(!self.node.next.map_or(false, |x| x.left_to(&block)), "Inserting \
                          right-adjacent block without merge.");
            // Check that we're inserting at a fitting position, such that the list is kept sorted.
            debug_assert!(self.node.block < block, "Inserting at a wrong position.");

            // Generate the maximum level of the new node's shortcuts.
            let height = shortcut::Level::generate();

            // Put the old node behind the new node holding block.
            seek.node.insert(arena.alloc(Node {
                block: block,
                // Place-holder.
                shortcuts: Default::default(),
                next: None,
            }));

            // If we actually have a bottom layer (i.e. the generated level is higher than zero),
            // we obviously need to update its fat values based on the main list.
            // FIXME: This code is copied from the loop below. Find a way to avoid repeating.
            if height > shortcut::Level(0) {
                // Place the node into the bottom shortcut.
                self.update_shortcut(shortcut::Level(0));

                // For details how this works, see the loop below. This is really just taken from
                // the iteration from that to reflect the special structure of the block list.

                // Calculate the fat value of the bottom layer.
                let new_fat = seek.node.calculate_fat_value_bottom();

                let skip = &mut seek.lookback[0];
                if new_fat == skip.fat {
                    if let Some(next) = skip.next {
                        next.fat = next.calculate_fat_value_bottom();
                    }
                } else {
                    skip.node.shortcuts[0].increase_fat(mem::replace(&mut skip.fat, new_fat));
                }
            }

            // Place new shortcuts up to this level.
            for lv in shortcut::Level(1)..height {
                // Place the node inbetween the shortcuts.
                seek.place_node_inbetween(&mut seek.node, lv);

                // The configuration (at level `lv`) should now look like:
                //     [seek.node] --> [old self.node] --> [old shortcut]

                // Since we inserted a new shortcut here, we might have invalidated the old fat
                // value, so we need to recompute the fat value. Fortunately, since we go from
                // densest to opaquer layer, we can base the fat value on the values of the
                // previous layer.

                // An example state could look like this: Assume we go from this configuration:
                //     [a] -y----------> [b] ---> ...
                //     [a] -x----------> [b] ---> ...
                //     ...
                // To this:
                //     [a] -y'---------> [b] ---> ...
                //     [a] -p-> [n] -q-> [b] ---> ...
                //     ...
                // Here, you cannot express _p_ and _q_ in terms  of _x_ and _y_. Instead, we need
                // to compute them from the layer below.
                let new_fat = seek.node.calculate_fat_value_non_bottom(lv);

                // You have been visitied by the silly ferris.
                //      ()    ()
                //       \/\/\/
                //       &_^^_&
                //       \    /
                // Fix all bugs in 0.9345 seconds, or you will be stuck doing premature
                // optimizations forever.

                // Avoid multiple bound checks.
                let skip = &mut seek.lookback[lv];
                // The shortcut behind the updated one might be invalidated as well. We use a nifty
                // trick: If the fat node is not present on one part of the split (defined by the
                // newly inserted node), it must fall on the other. So, we can shortcut and safely
                // skip computation of the second part half of the time.
                if new_fat == skip.fat {
                    if let Some(next) = skip.next {
                        // The fat value was left unchanged. This means that we need to recompute the
                        // next segment's fat value.
                        next.fat = next.calculate_fat_value_non_bottom(lv);
                    }

                    // Since it was unchanged, there is no need for setting the value to what it
                    // already is!
                } else {
                    // The first segment did not contain the fat node! This means that we know a
                    // priori that the second segment contains the old fat node, i.e. has either
                    // the same fat value or the updated node is itself the fat node. So, we
                    // replace the new fat value and update the next segment with the old one.  As
                    // an example, suppose the configuration looked like:
                    //     [a] -f----------> [b]
                    // And we inserted a new node _x_:
                    //     [a] -g-> [x] -h-> [b]
                    // Since the fat node (size _f_) couldn't be found on the left side (_g_) of
                    // _x_, it must be on the right side (_h ≥ f_). It might not be equal to it,
                    // because the size of _x_ could be greater than _f_, so we take the maximal of
                    // _x_ and _f_ and assigns that to _h_. This way we avoid having to iterate
                    // over all the nodes between _x_ and _b_.
                    skip.next.unwrap().shortcuts[lv].increase_fat(mem::replace(&mut skip.fat, new_fat), seek.node.block.size());
                }
            }

            // The levels above the inserted shortcuts need to get there fat value updated, since a
            // new node was inserted below. Since we only inserted (i.e. added a potentially new
            // fat node), we simply need to max the old (unupdated) fat values with the new block's
            // size.
            seek.increase_fat(seek.node.block.size(), height);

            seek.node.check();

            seek
        });

        self.check();
    }

    fn remove(&mut self) -> Jar<Node> {
        // Remove the shortcuts that skips the target node.
        self.remove_shortcuts();
        self.decrease_fat(self.node.size());

        unimplemented!();
    }

    /// Check this seek.
    ///
    /// This is NOOP in release mode.
    fn check(&self) {
        if cfg!(debug_assertions) {
            // Check the nodes.
            for i in self.node.iter() {
                i.check();
            }

            // Make sure that the first lookback entry is overshooting the node as expected.
            assert!(self.lookback[0].next.and_then(|x| x.block) >= self.node.block, "The first \
                    lookback entry is not overshooting the node of the seek.");

            // Check the lookback.
            let mut iter = self.lookback.peekable();
            let mut n = 0;
            loop {
                let cur = iter.next();
                let next = iter.peek();

                if let Some(cur) = cur {
                    // Make sure the shortcut doesn't start at the node (this is done by making
                    // sure the lookback entry and the n'th shortcut of the current node are
                    // distinct).
                    assert!(cur.next != self.node.shortcuts[n].next, "The {}'th lookback entry \
                            starts at the target node.");

                    if let Some(next) = next {
                        // The fat value satisfy the heap property, and thus must be ordered as such.
                        assert!(cur.fat <= next.fat, "The {}'th lookback entry has a fat value higher \
                                than its parent level, which ought to be less dense.", n);
                        // The next layer should be less dense, as such, the pointer is lower than the
                        // current one.
                        assert!(cur.next >= next.next, "The {}'th lookback entry's next-node pointer \
                                is lower than the parent level's pointer, despite that it ought to be \
                                denser.", n);
                    }
                } else {
                    break;
                }

                // Increment the counter (go to the next lookback entry).
                n += 1;
            }
        }
    }
}
