extern crate ralloc;

use std::thread;

fn make_thread() {
    thread::spawn(|| {
        let mut vec = Vec::new();

        for i in 0..0xFFFF {
            vec.push(0);
            vec[i] = i;
        }

        for i in 0..0xFFFF {
            assert_eq!(vec[i], i);
        }
    });
}

fn main() {
    for _ in 0..5 {
        make_thread();
    }
}
