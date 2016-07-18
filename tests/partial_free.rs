extern crate ralloc;

mod util;

use std::ptr;

#[test]
fn partial_free() {
    util::multiply(|| {
        let mut alloc = ralloc::Allocator::new();

        let buf = alloc.alloc(63, 3);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(buf, 0, 63);
                *buf = 4;
            });

            util::acid(|| {
                alloc.free(buf.offset(8), 75);
                *buf = 5;
            });

            util::acid(|| {
                alloc.free(buf, 4);
                *buf.offset(4) = 3;
            });

            assert_eq!(*buf.offset(4), 3);
        }
    });
}

#[test]
fn partial_free_double() {
    util::multiply(|| {
        let mut alloc = ralloc::Allocator::new();

        let buf = alloc.alloc(64, 4);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(buf, 0, 64);
            });

            util::acid(|| {
                alloc.free(buf.offset(32), 32);
                *buf = 5;
            });

            assert_eq!(*buf, 5);

            util::acid(|| {
                *buf = 0xAA;
                alloc.free(buf, 32);
            });
        }
    });
}
