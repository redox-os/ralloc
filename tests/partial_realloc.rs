extern crate ralloc;

mod util;

use std::ptr;

#[test]
fn partial_realloc() {
    util::multiply(|| {
        let mut alloc = ralloc::Allocator::new();
        let buf = alloc.alloc(63, 3);

        unsafe {
            util::acid(|| {
                ptr::write_bytes(buf, 0, 63);
                *buf = 4;
            });

            alloc.realloc(buf.offset(8), 75, 0, 23);
            *buf = 5;

            *alloc.realloc(buf, 4, 10, 2) = 10;

            alloc.free(buf, 4);
        }
    });
}
