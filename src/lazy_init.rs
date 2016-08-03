//! `LazyStatic` like initialization.

use core::{mem, ptr, intrinsics};

/// The initialization state
enum State<F, T> {
    /// The data is uninitialized, initialization is pending.
    ///
    /// The inner closure contains the initialization function.
    Uninitialized(F),
    /// The data is initialized, and ready for use.
    Initialized(T),
}

/// A lazily initialized container.
pub struct LazyInit<F, T> {
    /// The internal state.
    state: State<F, T>,
}

impl<F: FnMut() -> T, T> LazyInit<F, T> {
    /// Create a new to-be-initialized container.
    ///
    /// The closure will be executed when initialization is required.
    #[inline]
    pub const fn new(init: F) -> LazyInit<F, T> {
        LazyInit {
            state: State::Uninitialized(init),
        }
    }

    /// Get a mutable reference to the inner value.
    ///
    /// If it is uninitialize, it will be initialized and then returned.
    #[inline]
    pub fn get(&mut self) -> &mut T {
        let mut inner;

        let res = match self.state {
            State::Initialized(ref mut x) => {
                return x;
            },
            State::Uninitialized(ref mut f) => {
                inner = f();
            },
        };

        self.state = State::Initialized(inner);

        if let State::Initialized(ref mut x) = self.state {
            x
        } else {
            // TODO find a better way.
            unreachable!();
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use core::cell::Cell;

    #[test]
    fn test_init() {
        let mut lazy = LazyInit::new(|| 300);

        assert_eq!(*lazy.get(), 300);
        *lazy.get() = 400;
        assert_eq!(*lazy.get(), 400);
    }

    fn test_laziness() {
        let mut is_called = Cell::new(false);
        let mut lazy = LazyInit::new(|| is_called.set(true));
        assert!(!is_called.get());
        lazy.get();
        assert!(is_called.get());
    }
}
