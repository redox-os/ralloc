extern crate ralloc;

use std::thread;

#[test]
fn test() {
    for i in 0..0xFFFF {
        let bx = Box::new("frakkkko");
        let join = thread::spawn(move || Box::new(!i));
        drop(bx);
        let bx = Box::new("frakkkko");
        join.join().unwrap();
        drop(bx);
    }

    ralloc::lock().debug_assert_no_leak();
}
