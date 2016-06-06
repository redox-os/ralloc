extern crate ralloc;

use std::thread;

#[test]
fn test() {
    let mut join = Vec::new();

    for _ in 0..0xFFFF {
        let bx: Box<u64> = Box::new(0x11FE15C001);

        join.push(thread::spawn(move || {
            assert_eq!(*bx, 0x11FE15C001);
        }));
    }

    for i in join {
        i.join().unwrap();
    }

    ralloc::lock().debug_assert_no_leak();
}
