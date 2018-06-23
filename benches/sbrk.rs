#![feature(test)]

extern crate ralloc;
extern crate test;

#[bench]
fn bench_sbrk(b: &mut test::Bencher) {
    b.iter(|| unsafe { ralloc::sbrk(200) });
}
