use prelude::*;

use core::{ops, marker};

/// A thread-local container.
pub struct Cell<T> {
    /// The inner data.
    inner: T,
}

impl<T> Cell<T> {
    /// Create a new `Cell` wrapper.
    ///
    /// # Safety
    ///
    /// This is invariant-breaking (assumes thread-safety) and thus unsafe.
    pub const unsafe fn new(inner: T) -> Cell<T> {
        Cell { inner: inner }
    }

    /// Get a reference to the inner value.
    ///
    /// Due to the variable being thread-local, one should never transfer it across thread
    /// boundaries. The newtype returned ensures that.
    pub fn get(&'static self) -> Ref<T> {
        Ref::new(&self.inner)
    }
}

unsafe impl<T> marker::Sync for Cell<T> {}

/// A reference to a thread-local variable.
///
/// The purpose of this is to block sending it across thread boundaries.
pub struct Ref<T: 'static> {
    inner: &'static T,
}

impl<T> Ref<T> {
    /// Create a new thread-bounded reference.
    ///
    /// One might wonder why this is safe, and the answer is simple: this type doesn't guarantee
    /// that the internal pointer is from the current thread, it just guarantees that _future
    /// access_ through this struct is done in the current thread.
    pub fn new(x: &'static T) -> Ref<T> {
        Ref {
            inner: x,
        }
    }
}

impl<T> ops::Deref for Ref<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.inner
    }
}

impl<T> !Send for Ref<T> {}

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
        static $name: tls::Cell<$ty> = unsafe { tls::Cell::new($val) };
    }
}
