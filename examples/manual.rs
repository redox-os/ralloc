extern crate ralloc;

use std::ptr;

fn main() {
    let ptr1 = ralloc::alloc(30, 3);
    let ptr2 = ralloc::alloc(500, 20);

    assert_eq!(0, ptr1 as usize % 3);
    assert_eq!(0, ptr2 as usize % 20);

    unsafe {
        ptr::write_bytes(ptr1, 0x22, 30);
        for i in 0..500 {
            *ptr2.offset(i) = i as u8;
        }

        assert_eq!(*ptr1, 0x22);
        assert_eq!(*ptr1.offset(5), 0x22);

        assert_eq!(*ptr2, 0);
        assert_eq!(*ptr2.offset(15), 15);

        let ptr1 = ralloc::realloc(ptr1, 30, 300, 3);
        for i in 0..300 {
            *ptr1.offset(i) = i as u8;
        }
        assert_eq!(*ptr1, 0);
        assert_eq!(*ptr1.offset(200), 200);

        ralloc::free(ptr1, 30);
        ralloc::free(ptr2, 500);
    }
}
