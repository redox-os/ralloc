extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

use std::ptr;

#[test]
fn manual() {
    util::multiply(|| {
        let ptr1 = ralloc::alloc(30, 3);
        let ptr2 = ralloc::alloc(500, 20);

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

            let ptr1 = ralloc::realloc(ptr1, 30, 300, 3);
            for i in 0..300 {
                util::acid(|| {
                    *ptr1.offset(i) = i as u8;
                });
            }
            assert_eq!(*ptr1, 0);
            assert_eq!(*ptr1.offset(200), 200);

            util::acid(|| {
                ralloc::free(ptr1, 30);
                ralloc::free(ptr2, 500);
            });
        }
    });
}
