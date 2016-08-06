use core::cell::UnsafeCell;
use core::mem;

/// A move cell.
///
/// This allows you to take ownership and replace the internal data with a new value. The
/// functionality is similar to the one provided by [RFC #1659](https://github.com/rust-lang/rfcs/pull/1659).
// TODO use that rfc ^
pub struct MoveCell<T> {
    /// The inner data.
    inner: UnsafeCell<T>,
}

impl<T> MoveCell<T> {
    /// Create a new cell with some inner data.
    #[inline]
    pub const fn new(data: T) -> MoveCell<T> {
        MoveCell {
            inner: UnsafeCell::new(data),
        }
    }

    /// Replace the inner data and return the old.
    #[inline]
    pub fn replace(&self, new: T) -> T {
        mem::replace(unsafe { &mut *self.inner.get() }, new)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cell() {
        let cell = MoveCell::new(200);
        assert_eq!(cell.replace(300), 200);
        assert_eq!(cell.replace(4), 300);
    }
}
