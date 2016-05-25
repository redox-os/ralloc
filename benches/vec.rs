#![feature(test)]

extern crate ralloc;
extern crate test;

use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {
    b.iter(|| {
        let mut stuff = Vec::with_capacity(10);

        for i in 0..10000 { stuff.push(i) }

        stuff.reserve(100000);

        stuff
    })
}
