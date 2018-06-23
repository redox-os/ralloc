extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

#[inline(never)]
fn alloc_box() -> Box<u32> {
    Box::new(0xDEADBEAF)
}

#[test]
fn simple_box() {
    util::multiply(|| {
        let mut a = Box::new(1);
        let mut b = Box::new(2);
        let mut c = Box::new(3);

        assert_eq!(*a, 1);
        assert_eq!(*b, 2);
        assert_eq!(*c, 3);
        assert_eq!(*alloc_box(), 0xDEADBEAF);

        util::acid(|| {
            *a = 0;
            *b = 0;
            *c = 0;
        });
        assert_eq!(*a, 0);
        assert_eq!(*b, 0);
        assert_eq!(*c, 0);
    });
}
