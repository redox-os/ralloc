//! Pointer wrappers.

use prelude::*;

use core::nonzero::NonZero;
use core::{ops, marker, mem};

/// A pointer wrapper type.
///
/// A wrapper around a raw non-null `*mut T` that indicates that the possessor of this wrapper owns
/// the referent.
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Pointer<T> {
    /// The internal pointer.
    ptr: NonZero<*mut T>,
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
            ptr: NonZero::new(ptr),
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
                NonZero::new(0x1 as *mut T)
            },
            _phantom: marker::PhantomData,
        }
    }

    /// Cast this pointer into a pointer to another type.
    ///
    /// This will simply transmute the pointer, leaving the actual data unmodified.
    ///
    /// # Why not `From`?
    ///
    /// `T` implements `From<T>`, making it (currently) impossible to implement this type of cast
    /// with `From`. [RFC #1658](https://github.com/rust-lang/rfcs/pull/1658) fixes this.
    #[inline]
    pub fn cast<U>(self) -> Pointer<U> {
        Pointer {
            ptr: unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // Casting the pointer will preserve its nullable state.
                NonZero::new(*self as *mut U)
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
        Pointer::new(self.ptr.offset(diff))
    }

    /// Is this pointer aligned to `align`?
    #[inline]
    pub fn aligned_to(&self, align: usize) -> bool {
        *self.ptr as usize % align == 0
    }
}

impl<T> Default for Pointer<T> {
    fn default() -> Pointer<T> {
        Pointer::empty()
    }
}

unsafe impl<T: Send> Send for Pointer<T> {}
unsafe impl<T: Sync> Sync for Pointer<T> {}

impl<T> ops::Deref for Pointer<T> {
    type Target = *mut T;

    #[inline]
    fn deref(&self) -> &*mut T {
        &self.ptr
    }
}

/// A safe, owning pointer (container).
///
/// The name is derives libstd's `Box`, which is centered around heap management, and will free the
/// inner memory on drop. This limitation somewhat limits the scope, so we use a primitive where
/// freeing and allocating the inner memory is managed by the user.
#[must_use = "`Jar` does not handle the destructor automatically, please free it into an arena to \
              avoid memory leaks."]
pub struct Jar<T: Leak> {
    /// The inner pointer.
    ///
    /// This has four guarantees:
    ///
    /// 1. It is valid and initialized.
    /// 2. The lifetime is tied to the ownership of the box (i.e. it is valid until manually
    ///    deallocated).
    /// 3. It is aligned to the alignment of `T`.
    /// 4. It is non-aliased.
    ptr: Pointer<T>,
}

impl<T: Leak> Jar<T> {
    /// Create a jar from a raw pointer.
    ///
    /// # Safety
    ///
    /// Make sure the pointer is valid, initialized, non-aliased, and aligned. If any of these
    /// invariants are broken, unsafety occurs.
    #[inline]
    pub unsafe fn from_raw(ptr: Pointer<T>) -> Jar<T> {
        debug_assert!(ptr.aligned_to(mem::align_of::<T>()), "`ptr` is unaligned to `T`.");

        Jar { ptr: ptr }
    }
}

impl<T: Leak> ops::Deref for Jar<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe {
            // LAST AUDIT: 2016-08-24 (Ticki).

            &*self.ptr
        }
    }
}

impl<T: Leak> ops::DerefMut for Jar<T> {
    #[inline]
    fn deref_mut(&self) -> &mut T {
        unsafe {
            // LAST AUDIT: 2016-08-24 (Ticki).

            &mut *self.ptr
        }
    }
}

impl<T: Leak> From<Jar<T>> for Pointer<T> {
    fn from(jar: Jar<T>) -> Pointer<T> {
        jar.ptr
    }
}

#[cfg(debug_assertions)]
impl<T: Leak> Drop for Jar<T> {
    fn drop(&mut self) {
        panic!("Leaking a `Jar`.");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pointer() {
        let mut x = [b'a', b'b'];

        unsafe {
            let ptr = Pointer::new(&mut x[0]);
            assert_eq!(**ptr, b'a');
            assert_eq!(**ptr.clone().cast::<[u8; 1]>(), [b'a']);
            assert_eq!(**ptr.offset(1), b'b');
        }

        let mut y = ['a', 'b'];

        unsafe {
            let ptr = Pointer::new(&mut y[0]);
            assert_eq!(**ptr.clone().cast::<[char; 1]>(), ['a']);
            assert_eq!(**ptr.offset(1), 'b');
        }
    }

    #[test]
    fn test_empty() {
        assert_eq!(*Pointer::<u8>::empty() as usize, 1);
    }
}
