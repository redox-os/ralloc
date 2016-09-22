//! Fixed-size arenas.
//!
//! This module contains primitives for typed, fixed-size arenas, implemented as linked list. This
//! allows for cache-efficient allocation and deallocation of fixed blocks.

use prelude::*;

use core::{ptr, mem, marker};

use take;

/// A linked-list of pointers.
///
/// This is similar to a nodeless linked list. We use this internally to implement arenas.
///
/// Any linking pointer must point to a valid buffer of minimum pointer size.
#[derive(Default)]
struct PointerList {
    /// The head link of the list.
    head: Option<Pointer<PointerList>>,
}

impl PointerList {
    // Pop the head of the list.
    //
    // Return `None` if the list is empty.
    #[inline]
    fn pop(&mut self) -> Option<Pointer<u8>> {
        if let Some(head) = self.head {
            // Get the head pointer.
            let res = head.clone().cast();

            unsafe {
                // LAST AUDIT: 2016-08-24 (Ticki).

                // Set the head to the tail. Note that we keep this safe by maintaining the
                // invariants.
                *self = ptr::read(*head);
            }

            Some(res)
        } else {
            // The head is `None`, thus the list is empty.
            None
        }
    }

    /// Push a pointer to the top of the list.
    ///
    /// # Safety.
    ///
    /// This is unsafe due to holding the invariant that it is valid.
    #[inline]
    unsafe fn push(&mut self, ptr: Pointer<PointerList>) {
        take::replace_with(self, |x| {
            // Set the head to the pointer.
            x.head = Some(ptr.cast());
            // Move the list to `ptr`.
            **ptr.cast() = x;

            ptr
        })
    }
}

/// A typed arena.
///
/// This represented as a linked list of free blocks. The links them self are placed in the free
/// segments, making it entirely zero-cost.
///
/// `T` is guaranteed to be larger than pointer size (this is due to the necessity of being able to
/// fill in the free segments with pointer to the next segment).
pub struct Arena<T> {
    /// The internal list.
    list: PointerList,
    /// Phantom data.
    _phantom: marker::PhantomData<T>,
}

impl<T> Arena<T> {
    /// Create a new empty arena.
    ///
    /// # Panics
    ///
    /// This method will panic if a pointer is unable to fit the size of the type.
    #[inline]
    pub fn new() -> Arena<T> {
        // Make sure the links fit in.
        assert!(mem::size_of::<T>() >= mem::size_of::<PointerList>(), "Arena list is unable to \
                contain a link (type must be pointer sized or more).");

        Arena {
            list: PointerList::default(),
            _phantom: marker::PhantomData,
        }
    }

    /// Allocate a jar with some initial value.
    #[inline]
    pub fn alloc(&mut self, inner: T) -> Result<Jar<T>, ()> {
        if let Some(ptr) = self.list.pop() {
            // Note that this cast is valid due to the correctness of the `free` method (i.e. the
            // pointer is valid for `T`).
            let ptr = ptr.cast();

            // Gotta catch 'em bugs.
            debug_assert!(ptr.aligned_to(mem::align_of::<T>()), "Arena contains unaligned pointers.");

            unsafe {
                // LAST AUDIT: 2016-08-23 (Ticki).

                // Initialize the inner value. To avoid calling destructors on initialized values,
                // we user raw writes instead.
                mem::write(*ptr, inner);

                // Convert it to a `Jar` and we're ready to go!
                Ok(Jar::from_raw(ptr))
            }
        } else {
            // List empty :(.
            Err(())
        }
    }

    /// Free a jar to the arena.
    #[inline]
    pub fn free(&mut self, jar: Jar<T>) {
        unsafe {
            // LAST AUDIT: 2016-08-23 (Ticki).

            // TODO: Mark this as free.
            self.list.push(Pointer::from(jar).cast());
        }
    }

    /// Provide this arena with some block.
    ///
    /// This is used to fill the arena with memory from some source by essentially breaking the
    /// block up and linking each piece together.
    ///
    /// # Panics
    ///
    /// This will hit an assertion if `T`'s size doesn't divide the block's size.
    #[inline]
    pub fn provide(&mut self, block: Block) {
        // Some assertions...
        assert!(block.size() % mem::size_of::<T>() == 0, "`T`'s size does not divide the block's.");
        assert!(block.aligned_to(mem::align_of::<T>()) == 0, "Block is unaligned to `T`.");

        // We log for convenience.
        log!(DEBUG, "Providing {:?} to arena.", block);

        // Calculate the end pointer.
        let end = Pointer::from(block.empty_right()).cast();
        // Calculate the start pointer.
        let mut ptr: Pointer<PointerList>: Pointer::from(block).cast();

        loop {
            // We offset the current pointer to get the pointer to the next piece, which will
            // define the value we will put at `ptr`.
            // NB: We do not use `offset` because it asserts that we're inbound. Breaking this
            // invariant would lead to undefined behavior. Instead we do custom convert'n'add
            // arithmetic on the pointer.
            let next_ptr = Pointer::new((*ptr.clone() as usize + mem::size_of::<PointerList>()) as *mut PointerList);

            // If the new pointer goes beyond the end, we're done.
            if next_ptr == end {
                // We reached the end, so we leave a `None`.
                *ptr = PointerList {
                    head: None,
                }

                break;
            }

            // Make the piece point to the next piece.
            *ptr = PointerList {
                head: Some(next_ptr),
            };
            // Update the pointer counter.
            ptr = next_ptr;
        }
    }
}

#[cfg(test)]
mod test {
    use prelude::*;

    use brk;

    /// Helper method to make an artificial arena.
    fn make<T>() -> Arena<T> {
        let mut arena = Arena::new();
        arena.provide(brk::lock().sbrk(1024).unwrap());

        arena
    }

    #[test]
    fn test_integers() {
        let mut arena = make();

        let mut n = arena.alloc(200).unwrap();
        assert_eq!(*n, 200);
        *n = 400;
        assert_eq!(*n, 400);
        *n = 1;
        assert_eq!(*n, 1);
        arena.free(n);

        let mut n = arena.alloc(543).unwrap();
        assert_eq!(*n, 543);
        *n = 402;
        assert_eq!(*n, 402);
        *n = 2;
        assert_eq!(*n, 2);
        arena.free(n);
    }

    #[test]
    fn test_oom() {
        let mut arena = make();

        // Make the arena run dry.
        while arena.alloc('a').is_ok() {}

        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();
        arena.alloc(2).unwrap_err();

        let mut arena2 = make();

        while let Ok(x) = arena2.alloc('b') {
            arena.free(x);
        }

        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
        arena.alloc(2).unwrap();
    }

    #[test]
    fn test_cross_arena() {
        let mut arena1 = make();
        let mut arena2 = make();

        let mut n = arena1.alloc(200).unwrap();
        assert_eq!(*n, 200);
        *n = 400;
        assert_eq!(*n, 400);
        *n = 1;
        assert_eq!(*n, 1);
        arena2.free(n);

        let mut n = arena2.alloc(543).unwrap();
        assert_eq!(*n, 543);
        *n = 402;
        assert_eq!(*n, 402);
        *n = 2;
        assert_eq!(*n, 2);
        arena1.free(n);

        arena2.alloc(22).unwrap_err();
        arena1.alloc(22).unwrap();
    }
}
