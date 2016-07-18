//! Vector primitive.

use prelude::*;

use core::{slice, ops, mem, ptr};

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
    /// Create a new empty vector.
    ///
    /// This won't allocate a buffer, thus it will have a capacity of zero.
    #[inline]
    pub const fn new() -> Vec<T> {
        Vec {
            ptr: Pointer::empty(),
            len: 0,
            cap: 0,
        }
    }

    /// Create a vector from a block.
    ///
    /// # Safety
    ///
    /// This is unsafe, since it won't initialize the buffer in any way, possibly breaking type
    /// safety, memory safety, and so on. Thus, care must be taken upon usage.
    #[inline]
    #[cfg(test)]
    pub unsafe fn from_raw_parts(block: Block, len: usize) -> Vec<T> {
        Vec {
            cap: block.size() / mem::size_of::<T>(),
            ptr: Pointer::new(*Pointer::from(block) as *mut T),
            len: len,
        }
    }

    /// Replace the inner buffer with a new one, and return the old.
    ///
    /// This will memcpy the vectors buffer to the new block, and update the pointer and capacity
    /// to match the given block.
    ///
    /// # Panics
    ///
    /// This panics if the vector is bigger than the block. Additional checks might be done in
    /// debug mode.
    pub fn refill(&mut self, block: Block) -> Block {
        // Calculate the new capacity.
        let new_cap = block.size() / mem::size_of::<T>();

        // Make some assertions.
        assert!(self.len <= new_cap, "Block not large enough to cover the vector.");
        self.check(&block);

        let old = mem::replace(self, Vec::new());

        // Update the fields of `self`.
        self.cap = new_cap;
        self.ptr = unsafe { Pointer::new(*Pointer::from(block) as *mut T) };
        self.len = old.len;
        unsafe {
            ptr::copy_nonoverlapping(*old.ptr, *self.ptr, old.len);
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
    #[allow(cast_possible_wrap)]
    pub fn push(&mut self, elem: T) -> Result<(), ()> {
        if self.len == self.cap {
            Err(())
        } else {
            // Place the element in the end of the vector.
            unsafe {
                // By the invariants of this type (the size is bounded by the address space), this
                // conversion isn't overflowing.
                ptr::write((*self.ptr).offset(self.len as isize), elem);
            }

            // Increment the length.
            self.len += 1;
            Ok(())
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

    /// Check the validity of a block with respect to the vector.
    ///
    /// Blocks not passing this checks might lead to logic errors when used as buffer for the
    /// vector.
    ///
    /// This is a NO-OP in release mode.
    #[inline]
    fn check(&self, block: &Block) {
        debug_assert!(block.size() % mem::size_of::<T>() == 0, "The size of T does not divide the \
                      block's size.");
        debug_assert!(self.len <= block.size() / mem::size_of::<T>(), "Block not large enough to \
                      cover the vector.");
    }
}

/// Cast this vector to the respective block.
impl<T: Leak> From<Vec<T>> for Block {
    fn from(from: Vec<T>) -> Block {
        unsafe { Block::from_raw_parts(from.ptr.cast(), from.cap * mem::size_of::<T>()) }
    }
}

impl<T: Leak> ops::Deref for Vec<T> {
    #[inline]
    type Target = [T];

    fn deref(&self) -> &[T] {
        unsafe {
            slice::from_raw_parts(*self.ptr as *const _, self.len)
        }
    }
}

impl<T: Leak> ops::DerefMut for Vec<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        unsafe {
            slice::from_raw_parts_mut(*self.ptr as *mut _, self.len)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use prelude::*;

    #[test]
    fn test_vec() {
        let mut buffer = [b'a'; 32];
        let mut vec = unsafe {
            Vec::from_raw_parts(
                Block::from_raw_parts(Pointer::new(&mut buffer[0] as *mut u8), 32),
                16
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
            assert_eq!(vec.refill(
                Block::from_raw_parts(Pointer::new(&mut buffer[0] as *mut u8), 32)).size(),
                32
            );
        }

        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc");

        for _ in 0..14 {
            vec.push(b'_').unwrap();
        }

        vec.push(b'!').unwrap_err();

        assert_eq!(&*vec, b".aaaaaaaaaaaaaaabc______________");
        assert_eq!(vec.capacity(), 32);
    }
}
