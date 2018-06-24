extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

use std::thread;

fn make_thread() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        let mut vec = Vec::new();

        for i in 0..0xFFF {
            util::acid(|| {
                vec.push(0);
                vec[i] = i;
            });
        }

        for i in 0..0xFFF {
            assert_eq!(vec[i], i);
        }
    })
}

#[test]
#[ignore]
fn multithread_join_handle_vec() {
    util::multiply(|| {
        let mut join = Vec::new();

        for _ in 0..20 {
            util::acid(|| {
                join.push(make_thread());
            });
        }

        for i in join {
            i.join().unwrap();
        }
    });
}
