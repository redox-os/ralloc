//! Symbols and externs that `ralloc` depends on.
//!
//! This crate provides implementation/import of these in Linux, BSD, and Mac OS.

#![feature(linkage)]
#![no_std]
#![warn(missing_docs)]

extern crate libc;

pub use libc::sched_yield;

extern {
    /// Change the data segment. See `man sbrk`.
    pub fn sbrk(ptr: libc::intptr_t) -> *const libc::c_void;
    /// Write a buffer to a file descriptor.
    fn write(fd: libc::c_int, buff: *const libc::c_void, size: libc::size_t) -> libc::ssize_t;
}

/// Write to the log.
///
/// This points to stderr, but could be changed arbitrarily.
pub fn log(s: &str) -> libc::ssize_t {
    unsafe { write(2, s.as_ptr() as *const libc::c_void, s.len()) }
}

/// Thread destructors for Linux.
#[cfg(target_os = "linux")]
pub mod thread_destructor {
    use libc;

    extern {
        #[linkage = "extern_weak"]
        static __dso_handle: *mut u8;
        #[linkage = "extern_weak"]
        static __cxa_thread_atexit_impl: *const libc::c_void;
    }

    /// Does this platform support thread destructors?
    ///
    /// This will return true, if and only if `__cxa_thread_atexit_impl` is non-null.
    #[inline]
    pub fn is_supported() -> bool {
        !__cxa_thread_atexit_impl.is_null()
    }

    /// Register a thread destructor.
    ///
    /// # Safety
    ///
    /// This is unsafe due to accepting (and dereferencing) raw pointers, as well as running an
    /// arbitrary unsafe function.
    ///
    /// On older system without the `__cxa_thread_atexit_impl` symbol, this is unsafe to call, and will
    /// likely segfault.
    // TODO: Due to rust-lang/rust#18804, make sure this is not generic!
    pub unsafe fn register(t: *mut u8, dtor: unsafe extern fn(*mut u8)) {
        use core::mem;

        /// A thread destructor.
        type Dtor = unsafe extern fn(dtor: unsafe extern fn(*mut u8), arg: *mut u8, dso_handle: *mut u8) -> libc::c_int;

        mem::transmute::<*const libc::c_void, Dtor>(__cxa_thread_atexit_impl)(dtor, t, &__dso_handle as *const _ as *mut _);
    }
}

/// Thread destructors for Mac OS.
#[cfg(target_os = "macos")]
pub mod thread_destructor {
    use libc;

    /// Does this platform support thread destructors?
    ///
    /// This will always return true.
    #[inline]
    pub fn is_supported() -> bool { true }

    /// Register a thread destructor.
    ///
    /// # Safety
    ///
    /// This is unsafe due to accepting (and dereferencing) raw pointers, as well as running an
    /// arbitrary unsafe function.
    #[cfg(target_os = "macos")]
    pub unsafe fn register(t: *mut u8, dtor: unsafe extern fn(*mut u8)) {
        extern {
            fn _tlv_atexit(dtor: unsafe extern fn(*mut u8), arg: *mut u8);
        }

        _tlv_atexit(dtor, t);
    }
}

/// Debugging.
pub mod debug {
    use libc;

    extern {
        /// Valgrind symbol to declare memory undefined.
        fn valgrind_make_mem_undefined(ptr: *const libc::c_void, size: libc::size_t);
        /// Valgrind symbol to declare memory freed.
        fn valgrind_freelike_block(ptr: *const libc::c_void, size: libc::size_t);
    }

    /// Mark this segment undefined to the debugger.
    pub fn mark_undefined(ptr: *const libc::c_void, size: libc::size_t) {
        unsafe { valgrind_make_mem_undefined(ptr, size) }
    }
    /// Mark this segment free to the debugger.
    pub fn mark_free(ptr: *const libc::c_void, size: libc::size_t) {
        unsafe { valgrind_freelike_block(ptr, size) }
    }
}
