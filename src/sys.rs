use std::ptr::Unique;

/// Out of memory.
pub fn oom() -> ! {
    #[cfg(test)]
    panic!("Out of memory.");

    #[cfg(not(test))]
    ::alloc::oom();
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

/// Retrieve the end of the current data segment.
///
/// This will not change the state of the process in any way, and is thus safe.
pub fn segment_end() -> Result<*mut u8, Error> {
    unsafe {
        sys_brk(0)
    }.map(|x| x as *mut _)
}

/// Increment data segment of this process by some (signed) _n_, return a pointer to the new data
/// segment start.
///
/// This uses the system call BRK as backend.
pub fn inc_brk(n: usize) -> Result<Unique<u8>, Error> {
    let orig_seg_end = try!(segment_end()) as usize;
    if n == 0 {
        unsafe {
            return Ok(Unique::new(orig_seg_end as *mut u8))
        }
    }

    let expected_end = maybe!(orig_seg_end.checked_add(n) => return Err(Error::ArithOverflow));
    let new_seg_end = try!(unsafe { sys_brk(expected_end) });

    if new_seg_end != expected_end {
        // Reset the break.
        try!(unsafe { sys_brk(orig_seg_end) });

        Err(Error::OutOfMemory)
    } else {
        Ok(unsafe { Unique::new(orig_seg_end as *mut u8) })
    }
}

#[cfg(target_os = "redox")]
unsafe fn sys_brk(n: usize) -> Result<usize, Error> {
    use system::syscall;

    if let Ok(ret) = syscall::sys_brk(n) {
        Ok(ret)
    } else {
        Err(Error::Unknown)
    }
}

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
    use std::ptr;

    #[test]
    fn test_oom() {
        assert_eq!(inc_brk(9999999999999), Err(Error::OutOfMemory));
    }

    #[test]
    fn test_write() {
        let alloc_before = Box::new("hello from the outside.");
        let ptr = unsafe { (segment_end().unwrap() as *const u8).offset(-1) };
        let byte_end = unsafe { ptr::read(ptr) };

        let abc = "abc";
        let mem = inc_brk(8).unwrap() as *mut u64;
        unsafe {
            *mem = 90823;
            *mem = 2897309273;
            *mem = 293872;
            *mem = 0xDEADBEAFDEADBEAF;
            *mem = 99999;

            assert_eq!(*mem, 99999);
        }

        // Do some heap allocations.
        println!("test");
        let bx = Box::new("yo mamma is so nice.");
        println!("{}", bx);

        assert_eq!(*bx, "yo mamma is so nice.");
        assert_eq!(*alloc_before, "hello from the outside.");
        // Check that the stack frame is unaltered.
        assert_eq!(abc, "abc");
        assert_eq!(byte_end, unsafe { ptr::read(ptr) });
    }

    #[test]
    fn test_read() {
        let mem = inc_brk(8).unwrap() as *mut u64;
        unsafe {
            assert_eq!(*mem, 0);
        }
    }

    #[test]
    fn test_overflow() {
        assert_eq!(inc_brk(!0), Err(Error::ArithOverflow));
        assert_eq!(inc_brk(!0 - 2000), Err(Error::ArithOverflow));
    }

    #[test]
    fn test_empty() {
        assert_eq!(inc_brk(0), segment_end())
    }

    #[test]
    fn test_seq() {
        let a = inc_brk(4).unwrap() as usize;
        let b = inc_brk(5).unwrap() as usize;
        let c = inc_brk(6).unwrap() as usize;
        let d = inc_brk(7).unwrap() as usize;

        assert_eq!(a + 4, b);
        assert_eq!(b + 5, c);
        assert_eq!(c + 6, d);
    }

    #[test]
    fn test_thread() {
        use std::thread;

        let mut threads = Vec::new();

        for _ in 0..1000 {
            threads.push(thread::spawn(|| {
                inc_brk(9999).unwrap();
            }));
        }

        for i in threads {
            i.join().unwrap();
        }
    }

    #[test]
    fn test_segment_end() {
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
        assert_eq!(segment_end().unwrap(), segment_end().unwrap());
    }
}
