#![feature(test)]

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

extern crate ralloc;
extern crate test;

#[bench]
fn bench_vec(b: &mut test::Bencher) {
    b.iter(|| {
        let mut stuff = Vec::with_capacity(10);

        for i in 0..10000 {
            stuff.push(i)
        }

        stuff.reserve(100000);

        stuff
    });
}
