//! Pointer wrappers.

use core::marker;
use core::ptr::NonNull;

/// A pointer wrapper type.
///
/// A wrapper around a raw non-null `*mut T` that indicates that the possessor of this wrapper owns
/// the referent.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Pointer<T> {
    /// The internal pointer.
    ptr: NonNull<T>,
    /// Associated phantom data.
    ///
    /// This indicates that we _own_ T.
    _phantom: marker::PhantomData<T>,
}

impl<T> Pointer<T> {
    /// Create a new `Pointer` from a raw pointer.
    ///
    /// # Safety
    ///
    /// This function is unsafe since a null pointer can cause UB, due to `Pointer` being
    /// non-nullable.
    #[inline]
    pub unsafe fn new(ptr: *mut T) -> Pointer<T> {
        // For the sake of nice debugging, make some assertions.
        debug_assert!(!ptr.is_null(), "Null pointer!");

        Pointer {
            ptr: NonNull::new_unchecked(ptr),
            _phantom: marker::PhantomData,
        }
    }

    /// Create an "empty" `Pointer`.
    ///
    /// This acts as a null pointer, although it is represented by 0x1 instead of 0x0.
    #[inline]
    pub const fn empty() -> Pointer<T> {
        Pointer {
            ptr: unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // 0x1 is non-zero.
                NonNull::new_unchecked(0x1 as *mut T)
            },
            _phantom: marker::PhantomData,
        }
    }

    /// Cast this pointer into a pointer to another type.
    ///
    /// This will simply transmute the pointer, leaving the actual data unmodified.
    #[inline]
    pub fn cast<U>(self) -> Pointer<U> {
        Pointer {
            ptr: unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // Casting the pointer will preserve its nullable state.
                NonNull::new_unchecked(self.get() as *mut U)
            },
            _phantom: marker::PhantomData,
        }
    }

    /// Offset this pointer.
    ///
    /// This will add some value multiplied by the size of T to the pointer.
    ///
    /// # Safety
    ///
    /// This is unsafe, due to OOB offsets being undefined behavior.
    #[inline]
    pub unsafe fn offset(self, diff: isize) -> Pointer<T> {
        Pointer::new(self.ptr.as_ptr().offset(diff))
    }

    pub fn get(&self) -> *mut T {
        self.ptr.as_ptr()
    }
}

impl<T> Default for Pointer<T> {
    fn default() -> Pointer<T> {
        Pointer::empty()
    }
}

unsafe impl<T: Send> Send for Pointer<T> {}
unsafe impl<T: Sync> Sync for Pointer<T> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pointer() {
        let mut x = [b'a', b'b'];

        unsafe {
            let ptr = Pointer::new(&mut x[0] as *mut u8);
            assert_eq!(*ptr.get(), b'a');
            assert_eq!(*ptr.clone().cast::<[u8; 1]>().get(), [b'a']);
            assert_eq!(*ptr.offset(1).get(), b'b');
        }

        let mut y = ['a', 'b'];

        unsafe {
            let ptr = Pointer::new(&mut y[0] as *mut char);
            assert_eq!(*ptr.clone().cast::<[char; 1]>().get(), ['a']);
            assert_eq!(*ptr.offset(1).get(), 'b');
        }
    }

    #[test]
    fn test_empty() {
        assert_eq!(Pointer::<u8>::empty().get() as usize, 1);
    }
}
