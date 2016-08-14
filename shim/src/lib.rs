//! Symbols and externs that `ralloc` depends on.
//!
//! This crate provides implementation/import of these in Linux, BSD, and Mac OS.
//!
//! # Important
//!
//! You CANNOT use libc library calls, due to no guarantees being made about allocations of the
//! functions in the POSIX specification. Therefore, we use the system calls directly.

#![feature(linkage, core_intrinsics)]
#![no_std]
#![warn(missing_docs)]

#[macro_use]
extern crate syscall;

use core::intrinsics;

/// Voluntarily give a time slice to the scheduler.
pub fn sched_yield() -> usize {
    unsafe { syscall!(SCHED_YIELD) }
}

/// The default OOM handler.
#[cold]
pub fn default_oom_handler() -> ! {
    // Log some message.
    log("\x1b[31;1mThe application ran out of memory. Aborting.\n");

    unsafe {
        intrinsics::abort();
    }
}

/// Change the data segment. See `man brk`.
///
/// # Note
///
/// This is the `brk` **syscall**, not the library function.
pub unsafe fn brk(ptr: *const u8) -> *const u8 {
    syscall!(BRK, ptr) as *const u8
}

/// Write to the log.
///
/// This points to stderr, but could be changed arbitrarily.
pub fn log(s: &str) -> usize {
    unsafe { syscall!(WRITE, 2, s.as_ptr(), s.len()) }
}

/// Thread destructors for Linux.
#[cfg(target_os = "linux")]
pub mod thread_destructor {
    extern {
        #[linkage = "extern_weak"]
        static __dso_handle: *mut u8;
        #[linkage = "extern_weak"]
        static __cxa_thread_atexit_impl: *const u8;
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
        type Dtor = unsafe extern fn(dtor: unsafe extern fn(*mut u8), arg: *mut u8, dso_handle: *mut u8) -> i32;

        mem::transmute::<*const u8, Dtor>(__cxa_thread_atexit_impl)(dtor, t, &__dso_handle as *const _ as *mut _);
    }
}

/// Thread destructors for Mac OS.
#[cfg(target_os = "macos")]
pub mod thread_destructor {
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
    extern {
        /// Valgrind symbol to declare memory undefined.
        fn valgrind_make_mem_undefined(ptr: *const u8, size: usize);
        /// Valgrind symbol to declare memory freed.
        fn valgrind_freelike_block(ptr: *const u8, size: usize);
    }

    /// Mark this segment undefined to the debugger.
    pub fn mark_undefined(ptr: *const u8, size: usize) {
        unsafe { valgrind_make_mem_undefined(ptr, size) }
    }
    /// Mark this segment free to the debugger.
    pub fn mark_free(ptr: *const u8, size: usize) {
        unsafe { valgrind_freelike_block(ptr, size) }
    }
}
