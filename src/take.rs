//! Functions for temporarily moving out of ownership.

// TODO: https://github.com/rust-lang/rfcs/pull/1736

use core::{mem, intrinsics};

/// A guarding type which will exit upon drop.
///
/// This is used for catching unwinding and transforming it into abort.
///
/// The destructor should never be called naturally (use `mem::forget()`), and only when unwinding.
struct ExitGuard;

impl Drop for ExitGuard {
    fn drop(&mut self) {
        // To make sure the user gets a meaningful error message (as opposed to a simple abort), we
        // log to the `ERROR` level.
        log!(ERROR, "Unwinding in a `take` closure.");

        // Just abort the program.
        unsafe { intrinsics::abort(); }
    }
}

/// Temporarily take ownership of the inner value of a reference.
///
/// This essentially works as a generalized version of `mem::replace`, which instead takes a
/// closure that will return the replacement value.
///
/// This will abort on panics.
#[inline]
pub fn replace_with<T, F>(val: &mut T, replace: F)
    where F: FnOnce(T) -> T {
    // Guard against unwinding.
    let guard = ExitGuard;

    unsafe {
        // Take out the value behind the pointer.
        let old = ptr::read(val);
        // Run the closure.
        let new = closure(old);
        // Put the result back.
        ptr::write(val, new);
    }

    // Forget the guard.
    mem::forget(guard);
}

#[cfg(test)]
mod test {
    use super;

    use core::cell::Cell;

    #[test]
    fn replace_with() {
        let mut x = Some("test");
        take::replace_with(&mut x, |_| None);
        assert!(x.is_none());
    }

    #[test]
    fn replace_with_2() {
        let is_called = Cell::new(false);
        let mut x = 2;
        take::replace_with(&mut x, |_| {
            is_called.set(true);
            3
        });
        assert!(is_called.get());
    }
}
