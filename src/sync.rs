//! Synchronization primitives.

use core::sync::atomic::{self, AtomicBool};
use core::ops;

use sys;

/// A mutual exclusive container.
///
/// This assures that only one holds mutability of the inner value. To get the inner value, you
/// need acquire the "lock". If you try to lock it while a lock is already held elsewhere, it will
/// block the thread until the lock is released.
// TODO soundness issue when T: Drop?
pub struct Mutex<T> {
    /// The inner value.
    inner: T,
    /// The lock boolean.
    ///
    /// This is true, if and only if the lock is currently held.
    locked: AtomicBool,
}

/// A mutex guard.
///
/// This acts as the lock.
pub struct MutexGuard<'a, T: 'a> {
    mutex: &'a Mutex<T>,
}

/// Release the mutex.
impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.mutex.locked.store(false, atomic::Ordering::SeqCst);
    }
}

impl<'a, T> ops::Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.mutex.inner
    }
}

impl<'a, T> ops::DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *(&self.mutex.inner as *const T as *mut T) }
    }
}

impl<T> Mutex<T> {
    /// Create a new mutex with some inner value.
    pub const fn new(inner: T) -> Mutex<T> {
        Mutex {
            inner: inner,
            locked: AtomicBool::new(false),
        }
    }

    /// Lock this mutex.
    ///
    /// If another lock is held, this will block the thread until it is released.
    pub fn lock(&self) -> MutexGuard<T> {
        // Lock the mutex.
        while self.locked.compare_and_swap(false, true, atomic::Ordering::SeqCst) {
            // ,___,
            // {O,o}
            // |)``)
            // SRSLY?
            sys::yield_now();
        }

        MutexGuard {
            mutex: self,
        }
    }
}

unsafe impl<T> Sync for Mutex<T> {}

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
