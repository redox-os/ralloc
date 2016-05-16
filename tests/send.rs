extern crate ralloc;

use std::thread;

#[test]
fn test() {
    for _ in 0..0xFFFF {
        let bx: Box<u64> = Box::new(0x11FE15C001);

        thread::spawn(move || {
            assert_eq!(*bx, 0x11FE15C001);
        });
    }
}
