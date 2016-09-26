use block;

// TODO: Tweak.
// TODO: Move to shim.
const LEVELS: Level = Level(8);

usize_newtype!(pub Level);

impl Level {
    /// Generate a skip list level.
    ///
    /// This is equivalent to making `LEVELS` coinflips and count until you get heads and saturate to
    /// `LEVELS` if none is made.
    ///
    /// We make use of bit hacks to speed this process up such that only one random state update is
    /// needed.
    #[inline]
    fn generate_level() -> Level {
        // Naturally, the ffz conforms to our wanted probability distribution, $$p(x) = 2^{-x}$$. We
        // apply a bit mask to saturate when the ffz is greater than `LEVELS`.
        (random::get() & (1 << LEVELS - 1)).trailing_zeros()
    }

    #[inline]
    fn max() -> Level {
        LEVELS - Level(1)
    }
}

impl Into<usize> for Level {
    fn into(self) -> usize {
        self.0
    }
}

#[inline]
fn level_iter() -> impl Iterator<Item = Level> {
    Level(0)..Level::max()
}

#[derive(Default)]
struct Shortcut {
    next: Option<Pointer<Node>>,
    fat: block::Size,
}

impl Shortcut {
    #[inline]
    fn is_null(&self) -> bool {
        self.fat == 0
    }

    /// Update the fat value in case the new node is bigger than the current fat node.
    ///
    /// When inserting it is important for us to maintain the invariants. In this case, keeping
    /// track of the size of the biggest node skipped. When a new node is inserted, this value
    /// should naturally reflect that. If the new node's size is in fact greater than the fat
    /// value, the fat value will be updated.
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
    /// Let $$A$$ be a node with set of children $$C$$, then $$A = \max(C)$$. As such, if I start
    /// in the bottom and iterate upwards, as soon as the value stays unchanged, the rest of the
    /// values won't change either.
    #[inline]
    fn update_fat(&mut self, new_size: block::Size) -> bool {
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
