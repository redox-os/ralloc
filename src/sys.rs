//! System primitives.

use core::ptr::Unique;

/// Out of memory.
///
/// In release mode, this will simply abort the process (standard behavior). In debug mode, it will
/// panic, causing debugging to be easier.
pub fn oom() -> ! {
    #[cfg(test)]
    panic!("Out of memory.");

    #[cfg(not(test))]
    {
        use fail;
        fail::oom();
    }
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
        #[cfg(unix)]
        syscall!(SCHED_YIELD);

        #[cfg(redox)]
        ::system::syscall::unix::sys_yield();
    }
}

/// Retrieve the end of the current data segment.
///
/// This will not change the state of the process in any way, and is thus safe.
pub fn segment_end() -> Result<*mut u8, Error> {
    unsafe {
        sys_brk(0)
    }.map(|x| x as *mut _)
}

/// Increment data segment of this process by some, _n_, return a pointer to the new data segment
/// start.
///
/// This uses the system call BRK as backend.
pub fn inc_brk(n: usize) -> Result<Unique<u8>, Error> {
    let orig_seg_end = try!(segment_end()) as usize;
    if n == 0 {
        unsafe {
            return Ok(Unique::new(orig_seg_end as *mut u8))
        }
    }

    let expected_end = try!(orig_seg_end.checked_add(n).ok_or(Error::ArithOverflow));
    let new_seg_end = try!(unsafe { sys_brk(expected_end) });

    if new_seg_end != expected_end {
        // Reset the break.
        try!(unsafe { sys_brk(orig_seg_end) });

        Err(Error::OutOfMemory)
    } else {
        Ok(unsafe { Unique::new(orig_seg_end as *mut u8) })
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
        assert_eq!(inc_brk(9999999999999).err(), Some(Error::OutOfMemory));
    }

    #[test]
    fn test_read() {
        let mem = *inc_brk(8).unwrap() as *mut u64;
        unsafe {
            assert_eq!(*mem, 0);
        }
    }

    #[test]
    fn test_overflow() {
        assert_eq!(inc_brk(!0).err(), Some(Error::ArithOverflow));
        assert_eq!(inc_brk(!0 - 2000).err(), Some(Error::ArithOverflow));
    }

    #[test]
    fn test_empty() {
        assert_eq!(*inc_brk(0).unwrap(), segment_end().unwrap())
    }

    #[test]
    fn test_seq() {
        let a = *inc_brk(4).unwrap() as usize;
        let b = *inc_brk(5).unwrap() as usize;
        let c = *inc_brk(6).unwrap() as usize;
        let d = *inc_brk(7).unwrap() as usize;

        assert_eq!(a + 4, b);
        assert_eq!(b + 5, c);
        assert_eq!(c + 6, d);
    }

    #[test]
    fn test_segment_end() {
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
    }
}
