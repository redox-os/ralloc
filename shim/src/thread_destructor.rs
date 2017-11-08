//! Thread destructors.
//!
//! This module supplies the ability to register destructors called upon thread exit.

pub use self::arch::*;

/// Thread destructors for Linux/BSD.
#[cfg(not(target_os = "macos"))]
pub mod arch {
    extern {
        #[linkage = "extern_weak"]
        static __dso_handle: *mut u8;
        #[linkage = "extern_weak"]
        static __cxa_thread_atexit_impl: *const u8;
    }

    /// Register a thread destructor.
    // TODO: Due to rust-lang/rust#18804, make sure this is not generic!
    pub fn register(t: *mut u8, dtor: unsafe extern fn(*mut u8)) {
        use core::mem;

        /// A thread destructor.
        type Dtor = unsafe extern fn(dtor: unsafe extern fn(*mut u8), arg: *mut u8, dso_handle: *mut u8) -> i32;

        unsafe {
            // Make sure the symbols exist.
            assert!(!__cxa_thread_atexit_impl.is_null());

            mem::transmute::<*const u8, Dtor>(__cxa_thread_atexit_impl)
                (dtor, t, &__dso_handle as *const _ as *mut _)
        };
    }
}

/// Thread destructors for Mac OS.
#[cfg(target_os = "macos")]
pub mod arch {
    extern {
        fn _tlv_atexit(dtor: unsafe extern fn(*mut u8), arg: *mut u8);
    }

    /// Register a thread destructor.
    pub fn register(t: *mut u8, dtor: unsafe extern fn(*mut u8)) {
        _tlv_atexit(dtor, t);
    }
}
