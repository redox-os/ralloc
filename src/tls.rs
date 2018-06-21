//! Safe implementation of thread-local storage.
//!
//! This module provides lightweight abstractions for TLS similar to the ones provided by libstd.

use core::{marker, mem};

use shim::thread_destructor;

/// A thread-local container.
pub struct Key<T: 'static> {
    /// The inner data.
    inner: T,
}

impl<T: 'static> Key<T> {
    /// Create a new `Key` wrapper.
    ///
    /// # Safety
    ///
    /// This is invariant-breaking (assumes thread-safety) and thus unsafe.
    pub const unsafe fn new(inner: T) -> Key<T> {
        Key { inner: inner }
    }

    /// Obtain a reference temporarily.
    ///
    /// Due to [the lack of thread lifetimes](https://github.com/rust-lang/rfcs/pull/1705#issuecomment-238015901), we use a closure to make sure no leakage happens.
    ///
    /// Having a reference newtype would be unsound, due to the ability to leak a reference to
    /// another thread.
    #[inline]
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        // Logging.
        log!(INTERNAL, "Accessing TLS variable.");

        f(&self.inner)
    }

    /// Register a TLS destructor on the current thread.
    ///
    /// Note that this has to be registered for every thread, it is needed for.
    // TODO: Make this automatic on `Drop`.
    #[inline]
    pub fn register_thread_destructor(&self, dtor: extern "C" fn(&T)) {
        // Logging.
        log!(INTERNAL, "Registering thread destructor.");

        thread_destructor::register(&self.inner as *const T as *const u8 as *mut u8, unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // This is safe due to sharing memory layout.
            mem::transmute(dtor)
        });
    }
}

unsafe impl<T> marker::Sync for Key<T> {}

/// Declare a thread-local static variable.
///
/// TLS works by copying the initial data on every new thread creation. This allows access to a
/// variable, which is only available for the current thread, meaning that there is no need for
/// syncronization.
///
/// For this reason, in contrast to other `static`s in Rust, this need not thread-safety, which is
/// what this macro "fixes".
macro_rules! tls {
    (static $name:ident: $ty:ty = $val:expr;) => { tls! { #[] static $name: $ty = $val; } };
    (#[$($attr:meta),*] static $name:ident: $ty:ty = $val:expr;) => {
        $(#[$attr])*
        #[thread_local]
        static $name: tls::Key<$ty> = unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // This is secure due to being stored in a thread-local variable and thus being bounded
            // by the current thread.
            tls::Key::new($val)
        };
    }
}
