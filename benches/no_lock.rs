#![feature(test)]

extern crate ralloc;
extern crate test;

use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {
    b.iter(|| {
        let mut lock = ralloc::lock();

        for _ in 0..100000 {
            let a = lock.alloc(200, 2);
            unsafe {
                let a = lock.realloc(a, 200, 300, 2);
                lock.free(a, 300);
            }
        }
    });
}
