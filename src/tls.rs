use core::{ops, marker};

/// Add `Sync` to an arbitrary type.
///
/// This primitive is used to get around the `Sync` requirement in `static`s (even thread local
/// ones! see rust-lang/rust#35035). Due to breaking invariants, creating a value of such type is
/// unsafe, and care must be taken upon usage.
///
/// In general, this should only be used when you know it won't be shared across threads (e.g. the
/// value is stored in a thread local variable).
pub struct Syncify<T>(T);

impl<T> Syncify<T> {
    /// Create a new `Syncify` wrapper.
    ///
    /// # Safety
    ///
    /// This is invariant-breaking and thus unsafe.
    const unsafe fn new(inner: T) -> Syncify<T> {
        Syncify(T)
    }
}

impl<T> ops::Deref for Syncify<T> {
    type Target = T;

    fn deref(&self) -> Syncify<T> {
        &self.0
    }
}

impl<T> ops::DerefMut for Syncify<T> {
    fn deref_mut(&mut self) -> Syncify<T> {
        &mut self.0
        // If you read this, you are reading a note from a desperate programmer, who are really
        // waiting for a upstream fix, cause holy shit. Why the heck would you have a `Sync`
        // bound on thread-local variables. These are entirely single-threaded, and there is no
        // reason for assuming anything else. Now that we're at it, have the world been destroyed
        // yet?
    }
}

unsafe impl<T> marker::Sync for Syncify<T> {}

/// Declare a thread-local static variable.
///
/// TLS works by copying the initial data on every new thread creation. This allows access to a
/// variable, which is only available for the current thread, meaning that there is no need for
/// syncronization.
///
/// For this reason, in contrast to other `static`s in Rust, this need not thread-safety, which is
/// what this macro "fixes".
macro_rules! tls {
    (static $name:ident: $type:ty = $val:expr) => { tls!(#[] static $name: $type = $val) };
    (#[$($attr:meta),*], static $name:ident: $type:ty = $val:expr) => {{
        use tls::Syncify;

        $(#[$attr])*
        #[thread_local]
        static $name: $type = unsafe { Syncify::new($val) };
    }}
}
