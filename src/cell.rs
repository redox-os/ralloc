//! Cell primitives.

use core::cell::UnsafeCell;
use core::mem;

use take::take;

/// A move cell.
///
/// This allows you to take ownership and replace the internal data with a new value. The
/// functionality is similar to the one provided by [RFC #1659](https://github.com/rust-lang/rfcs/pull/1659).
// TODO: Use the features provided by the RFC.
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
        mem::replace(unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // This is safe due to never aliasing the value, but simply transferring ownership to
            // the caller.
            &mut *self.inner.get()
        }, new)
    }

    /// Get a reference to the inner value.
    ///
    /// Safety is enforced statically due to the guarantee of mutual exclusion in mutable
    /// references.
    pub fn get(&mut self) -> &mut T {
        mem::replace(unsafe {
            // LAST AUDIT: 2016-09-01 (Ticki).

            // This is safe due to the `&mut self`, enforcing the guarantee of uniqueness. This
            // will thus not alias it for the lifetime of that reference.
            &mut *self.inner.get()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_replace() {
        let cell = MoveCell::new(200);
        assert_eq!(cell.replace(300), 200);
        assert_eq!(cell.replace(4), 300);
    }

    #[test]
    fn test_get() {
        let mut cell = MoveCell::new(200);
        assert_eq!(*cell.get(), 200);

        *cell.get() = 300;
        assert_eq!(*cell.get(), 300);
    }
}
