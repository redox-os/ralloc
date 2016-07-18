extern crate ralloc;

mod util;

use std::ptr;

#[test]
fn manual() {
    util::multiply(|| {
        let mut alloc = ralloc::Allocator::new();

        let ptr1 = alloc.alloc(30, 3);
        let ptr2 = alloc.alloc(500, 20);

        assert_eq!(0, ptr1 as usize % 3);
        assert_eq!(0, ptr2 as usize % 20);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(ptr1, 0x22, 30);
            });
            util::acid(|| {
                for i in 0..500 {
                    *ptr2.offset(i) = i as u8;
                }
            });

            assert_eq!(*ptr1, 0x22);
            assert_eq!(*ptr1.offset(5), 0x22);

            assert_eq!(*ptr2, 0);
            assert_eq!(*ptr2.offset(15), 15);

            let ptr1 = alloc.realloc(ptr1, 30, 300, 3);
            for i in 0..300 {
                util::acid(|| {
                    *ptr1.offset(i) = i as u8;
                });
            }
            assert_eq!(*ptr1, 0);
            assert_eq!(*ptr1.offset(200), 200);

            util::acid(|| {
                alloc.free(ptr1, 30);
                alloc.free(ptr2, 500);
            });
        }
    });
}
