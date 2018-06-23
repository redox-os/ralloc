extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

#[test]
fn realloc_vec() {
    util::multiply(|| {
        let mut vec = Vec::new();

        vec.reserve(1);
        vec.reserve(2);
        util::acid(|| {
            vec.reserve(3);
            vec.reserve(100);
            vec.reserve(600);
        });
        vec.reserve(1000);
        vec.reserve(2000);

        vec.push(1);
        vec.push(2);
    });
}

#[test]
fn realloc_vec_2() {
    util::multiply(|| {
        let mut vec = Vec::with_capacity(4);

        vec.push(1);
        vec.push(2);
        vec.push(101);

        for x in 0..300 {
            util::acid(|| {
                vec.reserve_exact(x);
            });
        }
    });
}
