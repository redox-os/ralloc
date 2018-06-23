//! This test is a more subtle one. It is one which can hit thread destructors
//! unexpectedly.

extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

use std::sync::Arc;
use std::thread;

#[test]
fn test_arc() {
    let numbers: Vec<_> = (0..100).collect();
    let shared_numbers = Arc::new(numbers);

    for _ in 0..10 {
        let child_numbers = shared_numbers.clone();

        thread::spawn(move || {
            let _local_numbers = &child_numbers[..];

            // Work with the local numbers
        });
    }
}
