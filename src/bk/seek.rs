/// A "seek".
///
/// Seek represents the a found node ("needle") and backtracking information ("lookback").
struct Seek<'a> {
    /// The last node below our target for each level.
    ///
    /// This is used for insertions and other backtracking.
    ///
    /// The lower indexes are the denser layers.
    ///
    /// # An important note!
    ///
    /// It is crucial that the backlook is pointers to nodes _before_ the target, not the target
    /// node itself.
    ///
    /// # Example
    ///
    /// Consider if we search for 8. Now, we move on until we overshoot. The node before the
    /// overshot is a lookback entry (marked with curly braces).
    ///
    ///     ...
    ///     2      # ---------------------> {6} ---------------------> [9] -------------> NIL
    ///     1      # ---------------------> [6] ---> {7} ------------> [9] -------------> NIL
    ///     0      # ------------> [5] ---> [6] ---> {7} ---> [8] ---> [9] ---> [10] ---> NIL
    ///     bottom # ---> [1] ---> [5] ---> [6] ---> [7] ---> [8] ---> [9] ---> [10] ---> NIL
    ///
    /// So, the lookback of this particular seek is `[6, 7, 7, ...]`.
    // FIXME: Find a more rustic way than raw pointers.
    lookback: lv::Array<Pointer<Node>>,
    /// A pointer to a pointer to the found node.
    ///
    /// This is the node equal to or less than the target. The two layers of pointers are there to
    /// make it possible to modify the links and insert nodes before our target.
    node: &'a mut Jar<Node>,
}

impl<'a> Seek<'a> {
    /// Update the fat values of this seek to be higher than or equal to some new block size.  .
    ///
    /// This will simply run over and update the fat values of shortcuts of some level and above
    /// with a new size.
    ///
    /// Note that this cannot be used to remove a block, since that might decrease some fat values.
    #[inline]
    fn increase_fat(&mut self, size: block::Size, from: lv::Level) {
        // Start after `above` and go up, to update the fat node values.
        for i in &mut self.skips_from(above) {
            if !i.increase_fat(size) {
                // Short-circuit for performance reasons.
                break;
            }
        }
    }

    /// Get the `lv`'th "skip" of this seek.
    ///
    /// The $$n$$'th shortcut of the $$n$$'th entry of the lookback is referred to as the $$n$$'th
    /// "skip", because it _skips_ over the target node.
    fn get_skip(&mut self, lv: lv::Level) -> &mut Shortcut {
        &mut self.lookback[lv].shorcuts[lv]
    }

    /// Create an iterator over all of the skips of this seek.
    fn skips(&mut self) -> impl Iterator<Item = &mut Shortcut> {
        self.skips_from(Level(0))
    }

    /// Create an iterator over the skips of this seek starting at some level.
    fn skips_from(&mut self, lv: lv::Level) -> impl Iterator<Item = &mut Shortcut> {
        Skips {
            seek: self,
            n: lv::Iter::start_at(lv),
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
            self.increase_fat(self.node.block.size(), Level::min());

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
            self.increase_fat(block.size(), Level::min());

            // In case that our new configuration looks like:
            //     [==self==][==right=============]
            // We need to merge the former block right:
            self.try_merge_right(arena);
        } else {
            // We were unabled to merge it with a preexisting block, so we need to insert it
            // instead.
            self.insert_no_merge(block, arena);
        }
    }

    fn try_merge_right(&mut self, arena: &mut Arena<Node>) {
        if self.node.block.merge_right(self.node.next.block).is_ok() {
            // We merged our node left. This means that the node is simply extended and an empty
            // block will be left right to the node. As such the fat node's size is greater than or
            // equal to the new, merged node's size. So we need not full reevaluation of the fat
            // values, instead we can simply climb upwards and max the new size together.
            for (i, shortcut) in self.skips().zip(i.next.shortcuts) {
                // Update the shortcut to skip over the next shortcut. Note that this statements
                // makes it impossible to shortcut like in `insert`.
                i.next = shortcut.next;
                // Update the fat value to make sure it is greater or equal to the new block size.
                // Note that we do not short-circuit -- on purpose -- due to the above statement
                // being needed for correctness.
                i.increase_fat(self.node.block.size(), Level::min());

                // TODO: Consider breaking this loop into two loops to avoid too many fat value
                //       updates.
            }

            // Finally, replace the useless node, and free it to the arena.
            arena.free(mem::replace(self.node.next, self.node.next.unwrap().next));
        }
    }

    // Put a new shortcut starting at the current node at some level.
    //
    // This will simply insert a shortcut on level `lv`, spanning the old shortcut to `self.node`.
    //
    // The old shortcut is updated to point to a shortcut representing `self.node`, but the fat
    // value is kept as-is.
    //
    // The new shortcut's fat value will be set to zero, and recomputation is likely needed to
    // update it.
    //
    // # Illustrated
    //
    // We go from:
    //     [self.lookback[lv]] -f-> [self.lookback[lv].next]
    // To:
    //     [self.lookback[lv]] -f-> [self.node] -0-> [old self.lookback[lv].next]
    fn insert_shortcut(&mut self, lv: lv::Level) -> Pointer<Shortcut> {
        debug_assert!(self.node.shortcuts[lv].is_null(), "Overwriting a non-null shortcut.");

        // Make the old shortcut point to `self.node`.
        let old_next = mem::replace(&mut self.get_skip(lv).next, Some(self.node));
        // Update the shortcut of our node to point to the old shortcut's next node.
        self.node.shortcuts[lv] = Shortcut {
            next: old_next,
            fat: block::Size(0),
        };
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

            // Put the old node behind the new node holding block.
            seek.node.insert(arena.alloc(Node {
                block: block,
                // Place-holder.
                shortcuts: Default::default(),
                next: None,
            }));

            // Generate the maximum level of the new node's shortcuts.
            if let Some(max_lv) = lv::Level::generate() {
                // If we actually have a bottom layer (i.e. the generated level is higher than zero),
                // we obviously need to update its fat values based on the main list.
                // FIXME: This code is copied from the loop below. Find a way to avoid repeating.
                // Place the node into the bottom shortcut.
                self.insert_shortcut(lv::Level::min());

                // For details how this works, see the loop below. This is really just taken from
                // the iteration from that to reflect the special structure of the block list.

                // Calculate the fat value of the bottom layer.
                let new_fat = seek.node.calculate_fat_value_bottom();

                let skip = &mut seek.get_skip(lv::Level::min());
                if new_fat == skip.fat {
                    if let Some(next) = skip.next {
                        next.fat = next.calculate_fat_value_bottom();
                    }
                } else {
                    skip.node.shortcuts[0].increase_fat(mem::replace(&mut skip.fat, new_fat));
                }

                // Place new shortcuts up to this level.
                for lv in lv::Iter::non_bottom().to(max_lv) {
                    // Place the node inbetween the shortcuts.
                    seek.insert_shortcut(lv);

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
                    let skip = &mut seek.get_skip(lv);
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
                        // And we inserted a new node $x$:
                        //     [a] -g-> [x] -h-> [b]
                        // Since the fat node (size $f$) couldn't be found on the left side ($g$) of
                        // $x$, it must be on the right side ($h ≥ f$). It might not be equal to it,
                        // because the size of $x$ could be greater than $f$, so we take the maximal of
                        // $x$ and $f$ and assigns that to $h$. This way we avoid having to iterate
                        // over all the nodes between $x$ and $b$.
                        skip.next.unwrap().shortcuts[lv]
                            .increase_fat(cmp::max(mem::replace(&mut skip.fat, new_fat), seek.node.block.size()));
                    }
                }

                // The levels above the inserted shortcuts need to get there fat value updated, since a
                // new node was inserted below. Since we only inserted (i.e. added a potentially new
                // fat node), we simply need to max the old (unupdated) fat values with the new block's
                // size.
                if let Some(above) = max_lv.above() {
                    seek.increase_fat(seek.node.block.size(), above);
                }
            }

            seek.node.check();

            seek
        });

        self.check();
    }

    fn remove(self) -> Jar<Node> {
        // Remove the shortcuts that skips the target node (exclude the node from the skip of every
        // level). This is in place to make sure there's no dangling pointers after.
        for (_, skip) in self.node.shortcuts().zip(self.skips()) {
            // Jump over the skip's next node (note how the shortcut will never start at
            // `self.node` and hence always be the one we remove here). We can safely unwrap this,
            // because we know that `self.node` is at least of this height, by the zip iterator
            // adapter.
            skip.next = skip.next.unwrap().next;
        }

        // Update the fat values to reflect the new state.
        for i in self.skips() {
            if i.fat == self.node.size() {
                // Recalculate the fat value.
                let old_fat = i.fat;
                i.fat = i.calculate_fat_value(b);

                if old_fat == i.fat {
                    // The fat value was unchanged (i.e. there are multiple fat nodes), so we
                    // shortcircuit, because these duplicates will exist in higher layers too (due
                    // to the heap property).
                    // TODO: Is heap property the right term here? It isn't technically simply due
                    //       to the heap property...
                    break;
                }
            } else {
                // Since the node we're removing is not the same size as the fat node, it cannot be
                // a fat node. Due to the heap property of the fat values in the lookback, we can
                // shortcircuit (if we didn't remove the fat node in this layer, we didn't either
                // on any of the later layers).
                break;
            }
        }

        // We use the lowest layer in the lookback to use as offset for our search for the node
        // before `self.node`. We need to find said node to avoid having dangling pointers to
        // `self.node`.
        let before_node = self.lookback[0].iter().take_while(|x| x.block < self.node).last();
        // Remove the next link to skip the current node.
        before_node.next = self.node.next.take();

        self.node
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

            // Make sure that the first skip is overshooting the node as expected.
            assert!(self.get_skip(0).next.and_then(|x| x.block) >= self.node.block, "The first \
                    skip is not overshooting the node of the seek.");

            // Check the lookback.
            let mut iter = self.skips().peekable();
            let mut n = 0;
            loop {
                let cur = iter.next();
                let next = iter.peek();

                if let Some(cur) = cur {
                    // Make sure the shortcut doesn't start at the node (this is done by making
                    // sure the skip and the $n$'th shortcut of the current node are distinct).
                    assert!(cur.next != self.node.shortcuts[n].next, "The {}'th skip starts at \
                            the target node.");

                    if let Some(next) = next {
                        // The fat value satisfy the heap property, and thus must be ordered as such.
                        assert!(cur.fat <= next.fat, "The {}'th skip has a fat value higher than \
                                its parent level, which ought to be less dense.", n);
                        // The next layer should be less dense, as such, the pointer is lower than the
                        // current one.
                        assert!(cur.next >= next.next, "The {}'th skip's next-node pointer is \
                                lower than the parent level's pointer, despite that it ought to be \
                                denser.", n);
                    }
                } else {
                    break;
                }

                // Increment the counter (go to the next skip).
                n += 1;
            }
        }
    }
}

struct Skips<'a> {
    seek: &'a Seek<'a>,
    levels: lv::Iter,
}

impl<'a> Iterator for Skips<'a> {
    type Iter = &'a mut Shortcut;

    fn next(&mut self) -> Option<&'a mut Shortcut> {
        // Progress the level iterator.
        if let Some(lv) = self.levels.next() {
            // Return the skip.
            Some(self.seek.get_skip(lv))
        } else {
            None
        }
    }
}
