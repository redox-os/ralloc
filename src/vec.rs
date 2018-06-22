//! Vector primitive.

use prelude::*;

use core::{mem, ops, ptr, slice};

use leak::Leak;

/// A low-level vector primitive.
///
/// This does not perform allocation nor reallaction, thus these have to be done manually.
/// Moreover, no destructors are called, making it possible to leak memory.
pub struct Vec<T: Leak> {
    /// A pointer to the start of the buffer.
    ptr: Pointer<T>,
    /// The capacity of the buffer.
    ///
    /// This demonstrates the lengths before reallocation is necessary.
    cap: usize,
    /// The length of the vector.
    ///
    /// This is the number of elements from the start, that is initialized, and can be read safely.
    len: usize,
}

impl<T: Leak> Vec<T> {
    /// Create a vector from a block.
    ///
    /// # Safety
    ///
    /// This is unsafe, since it won't initialize the buffer in any way, possibly breaking type
    /// safety, memory safety, and so on. Thus, care must be taken upon usage.
    #[inline]
    pub unsafe fn from_raw_parts(block: Block, len: usize) -> Vec<T> {
        Vec {
            len: len,
            cap: block.size() / mem::size_of::<T>(),
            ptr: Pointer::from(block).cast(),
        }
    }

    /// Replace the inner buffer with a new one, and return the old.
    ///
    /// This will memcpy the vectors buffer to the new block, and update the pointer and capacity
    /// to match the given block.
    ///
    /// # Panics
    ///
    /// This panics if the vector is bigger than the block.
    pub fn refill(&mut self, block: Block) -> Block {
        log!(INTERNAL, "Refilling vector...");

        // Calculate the new capacity.
        let new_cap = block.size() / mem::size_of::<T>();

        // Make some assertions.
        assert!(
            self.len <= new_cap,
            "Block not large enough to cover the vector."
        );
        assert!(block.aligned_to(mem::align_of::<T>()), "Block not aligned.");

        let old = mem::replace(self, Vec::default());

        // Update the fields of `self`.
        self.cap = new_cap;
        self.ptr = Pointer::from(block).cast();
        self.len = old.len;
        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // Due to the invariants of `Block`, this copy is safe (the pointer is valid and
            // unaliased).
            ptr::copy_nonoverlapping(old.ptr.get(), self.ptr.get(), old.len);
        }

        Block::from(old)
    }

    /// Get the capacity of this vector.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Push an element to the end of this vector.
    ///
    /// On success, return `Ok(())`. On failure (not enough capacity), return `Err(())`.
    #[inline]
    pub fn push(&mut self, elem: T) -> Result<(), ()> {
        if self.len == self.cap {
            Err(())
        } else {
            // Place the element in the end of the vector.
            unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // By the invariants of this type (the size is bounded by the address space), this
                // conversion isn't overflowing.
                ptr::write((self.ptr.get()).offset(self.len as isize), elem);
            }

            // Increment the length.
            self.len += 1;
            Ok(())
        }
    }

    /// Pop an element from the vector.
    ///
    /// If the vector is empty, `None` is returned.
    #[inline]
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                // Decrement the length. This won't underflow due to the conditional above.
                self.len -= 1;

                // We use `ptr::read` since the element is unaccessible due to the decrease in the
                // length.
                Some(ptr::read(self.get_unchecked(self.len)))
            }
        }
    }

    /// Truncate this vector.
    ///
    /// This is O(1).
    ///
    /// # Panics
    ///
    /// Panics on out-of-bound.
    pub fn truncate(&mut self, len: usize) {
        // Bound check.
        assert!(len <= self.len, "Out of bound.");

        self.len = len;
    }

    /// Yield an iterator popping from the vector.
    pub fn pop_iter(&mut self) -> PopIter<T> {
        PopIter { vec: self }
    }
}

/// An iterator popping blocks from the bookkeeper.
pub struct PopIter<'a, T: 'a + Leak> {
    vec: &'a mut Vec<T>,
}

impl<'a, T: Leak> Iterator for PopIter<'a, T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.vec.pop()
    }
}

// TODO: Remove this in favour of `derive` when rust-lang/rust#35263 is fixed.
impl<T: Leak> Default for Vec<T> {
    fn default() -> Vec<T> {
        Vec {
            ptr: Pointer::empty(),
            cap: 0,
            len: 0,
        }
    }
}

/// Cast this vector to the respective block.
impl<T: Leak> From<Vec<T>> for Block {
    fn from(from: Vec<T>) -> Block {
        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // The invariants maintains safety.
            Block::from_raw_parts(from.ptr.cast(), from.cap * mem::size_of::<T>())
        }
    }
}

impl<T: Leak> ops::Deref for Vec<T> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &[T] {
        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // The invariants maintains safety.
            slice::from_raw_parts(self.ptr.get() as *const T, self.len)
        }
    }
}

impl<T: Leak> ops::DerefMut for Vec<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // The invariants maintains safety.
            slice::from_raw_parts_mut(self.ptr.get() as *mut T, self.len)
        }
    }
}

#[cfg(test)]
mod test {
    use prelude::*;

    #[test]
    fn test_vec() {
        let mut buffer = [b'a'; 32];
        let mut vec = unsafe {
            Vec::from_raw_parts(
                Block::from_raw_parts(Pointer::new(&mut buffer[0] as *mut u8), 32),
                16,
            )
        };

        assert_eq!(&*vec, b"aaaaaaaaaaaaaaaa");
        vec.push(b'b').unwrap();
        assert_eq!(&*vec, b"aaaaaaaaaaaaaaaab");
        vec.push(b'c').unwrap();
        assert_eq!(&*vec, b"aaaaaaaaaaaaaaaabc");
        vec[0] = b'.';
        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc");

        unsafe {
            assert_eq!(
                vec.refill(Block::from_raw_parts(
                    Pointer::new(&mut buffer[0] as *mut u8),
                    32
                )).size(),
                32
            );
        }

        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc");

        for _ in 0..14 {
            vec.push(b'_').unwrap();
        }
        assert_eq!(vec.pop().unwrap(), b'_');
        vec.push(b'@').unwrap();

        vec.push(b'!').unwrap_err();

        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc_____________@");
        assert_eq!(vec.capacity(), 32);

        for _ in 0..32 {
            vec.pop().unwrap();
        }

        assert!(vec.pop().is_none());
        assert!(vec.pop().is_none());
        assert!(vec.pop().is_none());
        assert!(vec.pop().is_none());
    }
}
