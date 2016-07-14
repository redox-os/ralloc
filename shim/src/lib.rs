//! Symbols and externs that ralloc depends on.

#![crate_name="ralloc_shim"]
#![crate_type="lib"]
#![feature(lang_items)]
#![warn(missing_docs)]
#![no_std]

extern "C" {
    /// Cooperatively gives up a timeslice to the OS scheduler.
    pub fn sched_yield() -> isize;

    /// Increment data segment of this process by some, _n_, return a pointer to the new data segment
    /// start.
    ///
    /// This uses the system call BRK as backend.
    ///
    /// This is unsafe for multiple reasons. Most importantly, it can create an inconsistent state,
    /// because it is not atomic. Thus, it can be used to create Undefined Behavior.
    pub fn sbrk(n: isize) -> *mut u8;
}
