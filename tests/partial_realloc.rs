extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

use std::ptr;

#[test]
fn partial_realloc() {
    util::multiply(|| {
        let buf = ralloc::alloc(63, 3);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(buf, 0, 63);
                *buf = 4;
            });

            ralloc::realloc(buf.offset(8), 75, 0, 23);
            *buf = 5;

            *ralloc::realloc(buf, 4, 10, 2) = 10;

            ralloc::free(buf, 4);
        }
    });
}
