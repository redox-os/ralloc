extern crate ralloc;

use ralloc::sys::{inc_brk, segment_end};

use std::ptr;

#[test]
fn test() {
    let alloc_before = Box::new("hello from the outside.");
    let ptr = unsafe { (segment_end().unwrap() as *const u8).offset(-1) };

    let abc = "abc";
    let mem = unsafe { *inc_brk(8).unwrap() as *mut u64 };
    unsafe {
        *mem = 90823;
        *mem = 2897309273;
        *mem = 293872;
        *mem = 0xDEADBEAFDEADBEAF;
        *mem = 99999;

        assert_eq!(*mem, 99999);
    }

    // Do some heap allocations.
    let bx = Box::new("yo mamma is so nice.");

    assert_eq!(*bx, "yo mamma is so nice.");
    assert_eq!(*alloc_before, "hello from the outside.");
    // Check that the stack frame is unaltered.
    assert_eq!(abc, "abc");
}
