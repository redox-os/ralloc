use core::cell::{UnsafeCell, Cell};
use core::ops;

/// An "uni-cell".
///
/// This is a mutually exclusive container, essentially acting as a single-threaded mutex.
pub struct UniCell<T> {
    /// The inner data.
    inner: UnsafeCell<T>,
    /// Is this data currently used?
    used: Cell<bool>,
}

impl<T> UniCell<T> {
    /// Create a new uni-cell with some inner data.
    #[inline]
    pub const fn new(data: T) -> UniCell<T> {
        UniCell {
            inner: UnsafeCell::new(data),
            used: Cell::new(false),
        }
    }

    /// Get an reference to the inner data.
    ///
    /// This will return `Err(())` if the data is currently in use.
    #[inline]
    pub fn get(&self) -> Result<Ref<T>, ()> {
        if self.used.get() {
            None
        } else {
            // Mark it as used.
            self.used.set(true);

            Some(Ref {
                cell: self,
            })
        }
    }

    /// Get the inner and mark the cell used forever.
    pub fn into_inner(&self) -> Option<T> {
        if self.used.get() {
            None
        } else {
            // Mark it as used forever.
            self.used.set(true);

            Some(ptr::read(self.inner.get()))
        }
    }
}

/// An reference to the inner value of an uni-cell.
pub struct Ref<T> {
    cell: UniCell<T>,
}

impl<T> ops::Deref for Ref<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &*self.cell.inner.get()
    }
}

impl<T> ops::DerefMut for Ref<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.cell.inner.get()
    }
}

impl<T> Drop for Ref<T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.used.set(false);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_inner() {
        assert_eq!(UniCell::new(101).get(), Ok(101));
        assert_eq!(UniCell::new("heh").get(), Ok("heh"));
    }

    #[test]
    fn test_double_get() {
        let cell = UniCell::new(500);

        assert_eq!(*cell.get().unwrap(), 500);

        {
            let tmp = cell.get();
            assert!(cell.get().is_err());
            {
                let tmp = cell.get();
                assert!(cell.get().is_err());
            }
            *tmp.unwrap() = 201;
        }

        assert_eq!(*cell.get().unwrap(), 201);
        *cell.get().unwrap() = 100;
        assert_eq!(*cell.get().unwrap(), 100);
    }
}
