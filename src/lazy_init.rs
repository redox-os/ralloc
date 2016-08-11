//! `LazyStatic` like initialization.

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
///
/// This container starts out simply containing an initializer (i.e., a function to construct the
/// value in question). When the value is requested, the initializer runs.
pub struct LazyInit<F, T> {
    /// The internal state.
    state: State<F, T>,
}

impl<F: FnMut() -> T, T> LazyInit<F, T> {
    /// Create a new to-be-initialized container.
    ///
    /// The closure will be executed when initialization is required, and is guaranteed to be
    /// executed at most once.
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
        let inner;

        match self.state {
            State::Initialized(ref mut x) => return x,
            State::Uninitialized(ref mut f) => inner = f(),
        }

        self.state = State::Initialized(inner);

        if let State::Initialized(ref mut x) = self.state {
            x
        } else {
            // TODO: Find a better way to deal with this case.
            unreachable!();
        }
    }

    /// Get the inner of the container.
    ///
    /// This won't mutate the container itself, since it consumes it. The initializer will (if
    /// necessary) be called and the result returned. If already initialized, the inner value will
    /// be moved out and returned.
    pub fn into_inner(self) -> T {
        match self.state {
            State::Initialized(x) => x,
            State::Uninitialized(mut f) => f(),
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

    #[test]
    fn test_laziness() {
        let is_called = Cell::new(false);
        let mut lazy = LazyInit::new(|| is_called.set(true));
        assert!(!is_called.get());
        lazy.get();
        assert!(is_called.get());
    }
}
