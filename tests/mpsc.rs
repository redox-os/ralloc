extern crate ralloc;

use std::thread;
use std::sync::mpsc;

fn main() {
    {
        let (tx, rx) = mpsc::channel::<Box<u64>>();
        thread::spawn(move || {
            tx.send(Box::new(0xBABAFBABAF)).unwrap();
            tx.send(Box::new(0xDEADBEAF)).unwrap();
            tx.send(Box::new(0xDECEA5E)).unwrap();
            tx.send(Box::new(0xDEC1A551F1E5)).unwrap();
        });
        assert_eq!(*rx.recv().unwrap(), 0xBABAFBABAF);
        assert_eq!(*rx.recv().unwrap(), 0xDEADBEAF);
        assert_eq!(*rx.recv().unwrap(), 0xDECEA5E);
        assert_eq!(*rx.recv().unwrap(), 0xDEC1A551F1E5);
    }

    let (tx, rx) = mpsc::channel();
    for _ in 0..0xFFFF {
        let tx = tx.clone();
        thread::spawn(move || {
            tx.send(Box::new(0xFA11BAD)).unwrap();
        });
    }
    for _ in 0..0xFFFF {
        assert_eq!(*rx.recv().unwrap(), 0xFA11BAD);
    }
}
