extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

use std::ptr;

#[test]
fn partial_free() {
    util::multiply(|| {
        let buf = ralloc::alloc(63, 3);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(buf, 0, 63);
                *buf = 4;
            });

            util::acid(|| {
                ralloc::free(buf.offset(8), 55);
                *buf = 5;
            });

            util::acid(|| {
                ralloc::free(buf, 4);
                *buf.offset(4) = 3;
            });

            assert_eq!(*buf.offset(4), 3);
        }
    });
}

#[test]
fn partial_free_double() {
    util::multiply(|| {
        let buf = ralloc::alloc(64, 4);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(buf, 0, 64);
            });

            util::acid(|| {
                ralloc::free(buf.offset(32), 32);
                *buf = 5;
            });

            assert_eq!(*buf, 5);

            util::acid(|| {
                *buf = 0xAA;
                ralloc::free(buf, 32);
            });
        }
    });
}
