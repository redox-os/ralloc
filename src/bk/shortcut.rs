use block;

/// A shortcut (a pointer to a later node and the size of the biggest block it skips).
///
/// A shortcut stores two values: a pointer to the node it skips to, and the size of the "fat
/// node(s)".
///
/// # Span
///
/// If a given node starts at $$a$$ and has a shortcut skipping to node $$b$$, then the shortcut's
/// span is $$(a, b]$$. If it has no node to skip to (i.e. it is `None`), the shortcut spans from
/// $$a$$ (exclusive) to the end of the list (inclusive).
///
/// In other words, the node which holds the shortcut is not included in the span, but the node it
/// skips to is included.
///
/// # Fat nodes and fat values
///
/// If a block $$c \in S$$ with $$S$$ being the span of some shortcut satisfy $c \geq d$$ for any
/// $$d \in S$$, this node is said to be a _fat node_.
///
/// In less formal terms, we can considered a fat node is (one of) the nodes with the biggest block
/// in the span of the shortcut.
///
/// The size of the fat nodes is called the _fat value_ of the shortcut.
///
/// # Children
///
/// Because we can view the shortcuts as a tree satisfying the heap property wrt. the fat value, we
/// refer to shortcuts which are contained in another shortcut's span as children of said shortcut.
#[derive(Default)]
struct Shortcut {
    /// The node it skips to (if any).
    next: Option<Pointer<Node>>,
    /// The fat value of this shortcut.
    ///
    /// This is the size of the biggest block the shortcut spans.
    fat: block::Size,
}

impl Shortcut {
    #[inline]
    fn is_null(&self) -> bool {
        self.fat == 0
    }

    /// Increase the fat value in case the new node is bigger than the current fat node.
    ///
    /// When inserting it is important for us to maintain the invariants. In this case, keeping
    /// track of the size of the biggest node skipped. When a new node is inserted, this value
    /// should naturally reflect that. If the new node's size is in fact greater than the fat
    /// value, the fat value will be updated.
    ///
    /// However, if a node is removed, this function is not the appropriate one to update the fat
    /// value, since such an operation might decrease the fat value, rather than increase it.
    ///
    /// # Short-circuiting
    ///
    /// The returned value indicates if the caller should continue propagating new fat value up.
    /// This can be either because the updated fat value is equal to old fat value, or that the
    /// shortcut was null (and thus all higher shortcuts are too).
    ///
    /// The former is based on the observation that updating the fat value is similar to heap insertion.
    ///
    /// Consider insertion a block of size 4:
    ///     [6] -6-------------------> ...
    ///     [6] -6-------------------> ...
    ///     [6] -6--------> [2] -2---> ...
    ///     [6] -6--------> [4] -4---> ...
    /// Clearly, all the unchanged fat values of the two highest levels are correct, but the third
    /// level's value following [2] is not. So we can shortcircuit when we get to the second last
    /// level, due to the fact that the tree of the shortcut's fat values satisfy the heap
    /// property:
    ///
    ///       6
    ///       |
    ///       6
    ///      / \
    ///     6   |
    ///         4
    ///         |
    ///         2
    ///
    /// Let $$A$$ be a node with set of children $$C$$, then $$A = \max(C)$$. As such, if I start
    /// in the bottom and iterate upwards, as soon as the value stays unchanged, the rest of the
    /// values won't change either.
    #[inline]
    fn increase_fat(&mut self, new_size: block::Size) -> bool {
        if self.fat < new_size && !self.is_null() {
            // The fat value is smaller than the new size and thus an update is required.
            self.fat = new_size;

            true
        } else {
            // Since the old fat value is either not smaller than or empty, we can safely
            // shortcircuits (see the notes layed out in the documentation comment).
            false
        }
    }
}

struct ShortcutIter<'a> {
    lv: Level,
    shortcut: Option<&'a Shortcut>,
}

impl<'a> ShortcutIter<'a> {
    /// Decrement the shortcut level of this iterator and return the old level.
    ///
    /// This will make the iterator skip approximately half of the elements of the previous state
    /// of the iterator.
    #[inline]
    fn decrement_level(&mut self) -> Level {
        let lv = self.lv;
        self.lv -= Level(1);
        lv
    }

    /// Unwrap the inner node (of the shortcut that the iterator is currently on).
    ///
    /// # Panics
    ///
    /// This will panic if the iterator is over (i.e. no node is left)
    #[inline]
    fn unwrap_node(self) -> &Node {
        self.shortcut.unwrap().node
    }
}

impl<'a> Iterator for ShortcutIter<'a> {
    type Item = &'a Shortcut;

    fn next(&mut self) -> &'a mut Shortcut {
        // Replace `self.shortcut` by the next shortcut, and return the old value.
        mem::replace(&mut self.shortcut, self.shortcut.map(|x| x.shortcut[self.lv].get()))
    }
}
