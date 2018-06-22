//! Test automation.

use std::{mem, thread};

/// Magic trait for boxed `FnOnce`s.
///
/// This is a temporary replacement as the trait from libstd is stabilized.
trait FnBox {
    /// Call the closure.
    fn call_box(self: Box<Self>);
}

impl<F: FnOnce()> FnBox for F {
    fn call_box(self: Box<Self>) {
        (*self)()
    }
}

/// Like `std::thread::spawn`, but without the closure bounds.
unsafe fn spawn_unsafe<'a, F: FnOnce() + Send + 'a>(
    func: F,
) -> thread::JoinHandle<()> {
    let closure: Box<FnBox + 'a> = Box::new(func);
    let closure: Box<FnBox + Send> = mem::transmute(closure);
    thread::spawn(move || closure.call_box())
}

/// Spawn three threads and `join` them.
fn spawn_double<F: Fn() + Sync + Send>(func: F) {
    let handle;

    unsafe {
        handle = spawn_unsafe(|| func());
    }

    func();

    handle.join().unwrap();
}

/// "Multiply" a closure, by running it in multiple threads at the same time.
///
/// This will test for memory leaks, as well as acid wrapping.
#[allow(dead_code)]
pub fn multiply<F: Fn() + Sync + Send + 'static>(func: F) {
    spawn_double(|| spawn_double(|| acid(|| func())));

    // TODO assert no leaks.
}

/// Wrap a block in acid tests.
///
/// This performs a number of temporary allocations to try to detect
/// inconsistency.
///
/// The basic idea is that if the allocator is broken, it might allocate the
/// same memory twice, or corrupt when allocating. Thus, we allocate some
/// temporary segment and override it. This way we might be able to detect
/// memory corruption through asserting memory consistency after the closure is
/// completed.
#[allow(dead_code)]
pub fn acid<F: FnOnce()>(func: F) {
    let mut vec = vec!["something", "yep", "yup"];
    let mut _v = vec![Box::new(2), Box::new(5)];
    let mut bx = Box::new(2389);
    let abc = Box::new("abc");

    vec.shrink_to_fit();
    vec.extend(["lol", "lulz"].iter());
    vec.shrink_to_fit();
    vec.extend(["we", "are"].iter());

    func();

    *bx = 500;
    vec.push("heyaya");
    *bx = 55;

    assert_eq!(
        vec,
        [
            "something",
            "yep",
            "yup",
            "lol",
            "lulz",
            "we",
            "are",
            "heyaya"
        ]
    );
    assert_eq!(*bx, 55);
    assert_eq!(*abc, "abc");
}
