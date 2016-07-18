extern crate ralloc;

mod util;

use std::thread;

#[test]
fn join_thread() {
    util::multiply(|| {
        for i in 0..0xFFF {
            let bx = Box::new("frakkkko");
            let join = thread::spawn(move || Box::new(!i));
            drop(bx);

            util::acid(move || {
                let bx = Box::new("frakkkko");
                join.join().unwrap();
                drop(bx);
            });
        }
    });
}
