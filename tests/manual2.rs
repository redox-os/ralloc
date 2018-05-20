extern crate ralloc;

mod util;

#[test]
fn manual2() {
    let ptr = ralloc::alloc(1723, 8);
    assert!(!ptr.is_null());
    for offset in 0..1723 {
        unsafe { *(ptr as *mut u8).offset(offset) = 0 as u8 };
    }
}
