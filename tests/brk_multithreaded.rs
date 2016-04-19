extern crate ralloc;

use ralloc::sys::inc_brk;

use std::thread;

fn main() {
    let mut threads = Vec::new();

    for _ in 0..1000 {
        threads.push(thread::spawn(|| {
            inc_brk(9999).unwrap();
        }));
    }

    for i in threads {
        i.join().unwrap();
    }
}
