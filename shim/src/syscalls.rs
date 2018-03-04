//! System calls.

/// Change the data segment. See `man brk`.
///
/// On success, the new program break is returned. On failure, the old program break is returned.
///
/// # Note
///
/// This is the `brk` **syscall**, not the library function.
#[cfg(not(target_os = "redox"))]
pub unsafe fn brk(ptr: *const u8) -> *const u8 {
    syscall!(BRK, ptr) as *const u8
}

/// Voluntarily give a time slice to the scheduler.
#[cfg(not(target_os = "redox"))]
pub fn sched_yield() -> usize {
    unsafe { syscall!(SCHED_YIELD) }
}

/// Change the data segment. See `man brk`.
///
/// On success, the new program break is returned. On failure, the old program break is returned.
///
/// # Note
///
/// This is the `brk` **syscall**, not the library function.
#[cfg(target_os = "redox")]
pub unsafe fn brk(ptr: *const u8) -> *const u8 {
    let old = ::syscall::brk(0).unwrap_or(0);
    ::syscall::brk(ptr as usize).unwrap_or(old) as *const u8
}

/// Voluntarily give a time slice to the scheduler.
#[cfg(target_os = "redox")]
pub fn sched_yield() -> usize {
    ::syscall::Error::mux(::syscall::sched_yield())
}
