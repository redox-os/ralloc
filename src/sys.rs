//! System primitives.

use ptr::Pointer;
use fail;

/// A system call error.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// Sir, we're running outta memory!
    OutOfMemory,
    /// An OS error occurred.
    Os,
}

impl Error {
    /// Handle this error with the appropriate method.
    pub fn handle(self) -> ! {
        match self {
            Error::OutOfMemory => fail::oom(),
            Error::Os => panic!("Unknown OS error.")
        }
    }
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    unsafe {
        #[cfg(not(target_os = "redox"))]
        syscall!(SCHED_YIELD);

        #[cfg(target_os = "redox")]
        ::system::syscall::unix::sys_yield();
    }
}

/// Retrieve the end of the current data segment.
///
/// This will not change the state of the process in any way, and is thus safe.
    #[inline]
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
#[inline]
pub unsafe fn inc_brk(n: usize) -> Result<Pointer<u8>, Error> {
    let orig_seg_end = try!(segment_end()) as usize;
    if n == 0 { return Ok(Pointer::new(orig_seg_end as *mut u8)) }

    let expected_end = try!(orig_seg_end.checked_add(n).ok_or(Error::OutOfMemory));
    let new_seg_end = try!(sys_brk(expected_end));

    if new_seg_end != expected_end {
        // Reset the break.
        try!(sys_brk(orig_seg_end));

        Err(Error::OutOfMemory)
    } else {
        Ok(Pointer::new(orig_seg_end as *mut u8))
    }
}

/// Redox syscall, BRK.
#[inline]
#[cfg(target_os = "redox")]
unsafe fn sys_brk(n: usize) -> Result<usize, Error> {
    use system::syscall;

    if let Ok(ret) = syscall::sys_brk(n) {
        Ok(ret)
    } else {
        Err(Error::Os)
    }
}

/// Unix syscall, BRK.
#[inline]
#[cfg(not(target_os = "redox"))]
unsafe fn sys_brk(n: usize) -> Result<usize, Error> {
    let ret = syscall!(BRK, n);

    if ret == !0 {
        Err(Error::Os)
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
    fn test_overflow() {
        unsafe {
            assert_eq!(inc_brk(!0).err(), Some(Error::OutOfMemory));
            assert_eq!(inc_brk(!0 - 2000).err(), Some(Error::OutOfMemory));
        }
    }
}
