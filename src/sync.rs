//! Synchronization primitives.

use core::cell::UnsafeCell;
use core::ops;
use core::sync::atomic::{self, AtomicBool};

use shim;

/// A mutual exclusive container.
///
/// This assures that only one holds mutability of the inner value. To get the inner value, you
/// need acquire the "lock". If you try to lock it while a lock is already held elsewhere, it will
/// block the thread until the lock is released.
pub struct Mutex<T> {
    /// The inner value.
    inner: UnsafeCell<T>,
    /// The lock boolean.
    ///
    /// This is true, if and only if the lock is currently held.
    locked: AtomicBool,
}

impl<T> Mutex<T> {
    /// Create a new mutex with some inner value.
    #[inline]
    pub const fn new(inner: T) -> Mutex<T> {
        Mutex {
            inner: UnsafeCell::new(inner),
            locked: AtomicBool::new(false),
        }
    }

    /// Lock this mutex.
    ///
    /// If another lock is held, this will block the thread until it is released.
    #[inline]
    pub fn lock(&self) -> MutexGuard<T> {
        // Lock the mutex.
        #[cfg(not(feature = "unsafe_no_mutex_lock"))]
        while self
            .locked
            .compare_and_swap(false, true, atomic::Ordering::SeqCst)
        {
            // ,___,
            // {O,o}
            // |)``)
            // SRSLY?
            shim::syscalls::sched_yield();
        }

        MutexGuard { mutex: self }
    }
}

/// A mutex guard.
///
/// This acts as the lock.
#[must_use]
pub struct MutexGuard<'a, T: 'a> {
    /// The parent mutex.
    mutex: &'a Mutex<T>,
}

/// Release the mutex.
impl<'a, T> Drop for MutexGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        self.mutex.locked.store(false, atomic::Ordering::SeqCst);
    }
}

impl<'a, T> ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // Aliasing is allowed due to the lock representing mutual exclusive access.
            &*self.mutex.inner.get()
        }
    }
}

impl<'a, T> ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // Aliasing is allowed due to the lock representing mutual exclusive access.
            &mut *self.mutex.inner.get()
        }
    }
}

unsafe impl<T: Send> Send for Mutex<T> {}
unsafe impl<T: Send> Sync for Mutex<T> {}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_mutex() {
        let mutex = Mutex::new(3);
        assert_eq!(*mutex.lock(), 3);

        *mutex.lock() = 4;
        assert_eq!(*mutex.lock(), 4);

        *mutex.lock() = 0xFF;
        assert_eq!(*mutex.lock(), 0xFF);
    }
}
