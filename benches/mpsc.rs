#![feature(test)]

extern crate ralloc;
extern crate test;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

use std::sync::mpsc;
use std::thread;

#[bench]
fn bench_mpsc(b: &mut test::Bencher) {
    b.iter(|| {
        let (tx, rx) = mpsc::channel::<Box<u64>>();
        thread::spawn(move || {
            tx.send(Box::new(0xBABAFBABAF)).unwrap();
            tx.send(Box::new(0xDEADBEAF)).unwrap();
            tx.send(Box::new(0xDECEA5E)).unwrap();
            tx.send(Box::new(0xDEC1A551F1E5)).unwrap();
        });

        let (ty, ry) = mpsc::channel();
        for _ in 0..0xFF {
            let ty = ty.clone();
            thread::spawn(move || {
                ty.send(Box::new(0xFA11BAD)).unwrap();
            });
        }

        (rx, ry)
    });
}
