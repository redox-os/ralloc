//! Memory primitives.

use core::cmp;
use core::ptr::Unique;

/// A contigious memory block.
pub struct Block {
    /// The size of this block, in bytes.
    pub size: usize,
    /// The pointer to the start of this block.
    pub ptr: Unique<u8>,
}

impl Block {
    /// Get a pointer to the end of this block, not inclusive.
    pub fn end(&self) -> Unique<u8> {
        // TODO, this might trigger an overflow, which could imply creating a null-pointer.
        let ptr = (self.size + *self.ptr as usize) as *mut _;
        debug_assert!(!ptr.is_null(), "Pointer is null.");

        unsafe {
            Unique::new(ptr)
        }
    }

    /// Is this block free?
    pub fn is_free(&self) -> bool {
        self.size != 0
    }

    /// Set this block as free.
    ///
    /// This will not deallocate, but it will simply set the size to zero, which is the
    /// representation of a freeed block.
    pub fn set_free(&mut self) {
        self.size = 0;
    }

    /// Is this block is left to `to`?
    pub fn left_to(&self, to: &Block) -> bool {
        self.size + *self.ptr as usize == *to.ptr as usize
    }
}

impl PartialOrd for Block {
    fn partial_cmp(&self, other: &Block) -> Option<cmp::Ordering> {
        self.ptr.partial_cmp(&other.ptr)
    }
}

impl Ord for Block {
    fn cmp(&self, other: &Block) -> cmp::Ordering {
        self.ptr.cmp(&other.ptr)
    }
}

impl cmp::PartialEq for Block {
    fn eq(&self, other: &Block) -> bool {
        self.size == other.size && *self.ptr == *other.ptr
    }
}

impl cmp::Eq for Block {}

#[cfg(test)]
mod test {
    use super::*;
    use core::ptr::Unique;

    #[test]
    fn test_end() {
        let a = Block {
            size: 10,
            ptr: unsafe { Unique::new(15 as *mut _) },
        };
        let b = Block {
            size: 15,
            ptr: unsafe { Unique::new(25 as *mut _) },
        };
        let c = Block {
            size: 75,
            ptr: unsafe { Unique::new(40 as *mut _) },
        };

        assert_eq!(*a.end(), *b.ptr);
        assert_eq!(*b.end(), *c.ptr);
    }

    #[test]
    fn test_left_to() {
        let a = Block {
            size: 10,
            ptr: unsafe { Unique::new(15 as *mut _) },
        };
        let b = Block {
            size: 15,
            ptr: unsafe { Unique::new(25 as *mut _) },
        };
        let c = Block {
            size: 75,
            ptr: unsafe { Unique::new(40 as *mut _) },
        };

        assert!(a.left_to(&b));
        assert!(b.left_to(&c));
        assert!(!c.left_to(&a));
        assert!(!a.left_to(&c));
        assert!(!b.left_to(&b));
        assert!(!b.left_to(&a));
    }

    #[test]
    fn test_cmp() {
        let a = Block {
            size: 10,
            ptr: unsafe { Unique::new(10 as *mut _) },
        };
        let b = Block {
            size: 15,
            ptr: unsafe { Unique::new(25 as *mut _) },
        };
        let c = Block {
            size: 75,
            ptr: unsafe { Unique::new(40 as *mut _) },
        };

        assert!(a < b);
        assert!(b < c);
        assert!(c > a);
        assert!(a == a);
        assert!(b == b);
        assert!(c == c);
        assert!(c >= c);
        assert!(c <= c);
        assert!(a <= c);
        assert!(b >= a);
    }
}
