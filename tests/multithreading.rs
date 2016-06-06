extern crate ralloc;

use std::thread;

fn make_thread() -> thread::JoinHandle<()> {
    thread::spawn(|| {
        let mut vec = Vec::new();

        for i in 0..0xFFFF {
            vec.push(0);
            vec[i] = i;
        }

        for i in 0..0xFFFF {
            assert_eq!(vec[i], i);
        }
    })
}

#[test]
fn test() {
    let mut join = Vec::new();
    for _ in 0..50 {
        join.push(make_thread());
    }

    for i in join {
        i.join().unwrap();
    }

    ralloc::lock().debug_assert_no_leak();
}
