//! System primitives.

use core::ptr::Unique;

use fail;

/// Out of memory.
///
/// In release mode, this will simply abort the process (standard behavior). In debug mode, it will
/// panic, causing debugging to be easier.
pub fn oom() -> ! {
    fail::oom();
}

/// A system call error.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Sir, we're running outta memory!
    OutOfMemory,
    /// Arithmetic overflow.
    ArithOverflow,
    /// An unknown error occurred.
    Unknown,
}

impl Error {
    /// Handle this error with the appropriate method.
    pub fn handle(self) -> ! {
        match self {
            Error::OutOfMemory | Error::ArithOverflow => oom(),
            Error::Unknown => panic!("Unknown OS error.")
        }
    }
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    unsafe {
        #[cfg(target_os = "redox")]
        syscall!(SCHED_YIELD);

        #[cfg(not(target_os = "redox"))]
        ::system::syscall::unix::sys_yield();
    }
}

/// Retrieve the end of the current data segment.
///
/// This will not change the state of the process in any way, and is thus safe.
pub fn segment_end() -> Result<*const u8, Error> {
    unsafe {
        sys_brk(0)
    }.map(|x| x as *const _)
}

/// Increment data segment of this process by some, _n_, return a pointer to the new data segment
/// start.
///
/// This uses the system call BRK as backend.
///
/// This is unsafe for multiple reasons. Most importantly, it can create an inconsistent state,
/// because it is not atomic. Thus, it can be used to create Undefined Behavior.
pub unsafe fn inc_brk(n: usize) -> Result<Unique<u8>, Error> {
    let orig_seg_end = try!(segment_end()) as usize;
    if n == 0 { return Ok(Unique::new(orig_seg_end as *mut u8)) }

    let expected_end = try!(orig_seg_end.checked_add(n).ok_or(Error::ArithOverflow));
    let new_seg_end = try!(sys_brk(expected_end));

    if new_seg_end != expected_end {
        // Reset the break.
        try!(sys_brk(orig_seg_end));

        Err(Error::OutOfMemory)
    } else {
        Ok(Unique::new(orig_seg_end as *mut u8))
    }
}

/// Redox syscall, BRK.
#[cfg(target_os = "redox")]
unsafe fn sys_brk(n: usize) -> Result<usize, Error> {
    use system::syscall;

    if let Ok(ret) = syscall::sys_brk(n) {
        Ok(ret)
    } else {
        Err(Error::Unknown)
    }
}

/// Unix syscall, BRK.
#[cfg(not(target_os = "redox"))]
unsafe fn sys_brk(n: usize) -> Result<usize, Error> {
    let ret = syscall!(BRK, n);

    if ret == !0 {
        Err(Error::Unknown)
    } else {
        Ok(ret)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_oom() {
        unsafe {
            assert_eq!(inc_brk(9999999999999).err(), Some(Error::OutOfMemory));
        }
    }

    #[test]
    fn test_read() {
        unsafe {
            let mem = *inc_brk(8).unwrap() as *mut u64;
            assert_eq!(*mem, 0);
        }
    }

    #[test]
    fn test_overflow() {
        unsafe {
            assert_eq!(inc_brk(!0).err(), Some(Error::ArithOverflow));
            assert_eq!(inc_brk(!0 - 2000).err(), Some(Error::ArithOverflow));
        }
    }

    #[test]
    fn test_segment_end() {
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
    }
}
