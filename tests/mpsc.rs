extern crate ralloc;

mod util;

use std::thread;
use std::sync::mpsc;

#[test]
fn mpsc_queue() {
    util::multiply(|| {
        {
            let (tx, rx) = mpsc::channel::<Box<u64>>();

            let handle = thread::spawn(move || {
                util::acid(|| {
                    tx.send(Box::new(0xBABAFBABAF)).unwrap();
                    tx.send(Box::new(0xDEADBEAF)).unwrap();
                    tx.send(Box::new(0xDECEA5E)).unwrap();
                    tx.send(Box::new(0xDEC1A551F1E5)).unwrap();
                });
            });
            assert_eq!(*rx.recv().unwrap(), 0xBABAFBABAF);
            assert_eq!(*rx.recv().unwrap(), 0xDEADBEAF);
            assert_eq!(*rx.recv().unwrap(), 0xDECEA5E);
            assert_eq!(*rx.recv().unwrap(), 0xDEC1A551F1E5);

            handle.join().unwrap();
        }

        let (tx, rx) = mpsc::channel();
        let mut handles = Vec::new();

        for _ in 0..10 {
            util::acid(|| {
                let tx = tx.clone();
                handles.push(thread::spawn(move || {
                    tx.send(Box::new(0xFA11BAD)).unwrap();
                }));
            });
        }

        for _ in 0..10 {
            assert_eq!(*rx.recv().unwrap(), 0xFA11BAD);
        }

        for i in handles {
            i.join().unwrap()
        }
    });
}
