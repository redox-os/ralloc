#![feature(test)]

extern crate ralloc;
extern crate test;

use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {
    b.iter(|| {
        let _bx1 = Box::new(0xF000D);
        let _bx2 = Box::new(0xF0002);

        "abc".to_owned().into_boxed_str()
    })
}
