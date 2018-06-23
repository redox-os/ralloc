extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

use std::thread;

#[test]
#[ignore]
fn cross_thread_drop() {
    util::multiply(|| {
        let mut join = Vec::new();

        for _ in 0..10 {
            let bx = Box::new(0x11FE15C001u64);

            join.push(thread::spawn(move || {
                util::acid(|| {
                    assert_eq!(*bx, 0x11FE15C001);
                });
            }));
        }

        for i in join {
            i.join().unwrap();
        }
    });
}

#[test]
fn cross_thread_drop_2() {
    util::multiply(|| {
        for _ in 0..10 {
            let bx =
                thread::spawn(|| Box::new(0x11FE15C001u64)).join().unwrap();

            thread::spawn(move || {
                util::acid(|| {
                    assert_eq!(*bx, 0x11FE15C001);
                });
            });
        }
    });
}
