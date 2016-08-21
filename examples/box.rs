extern crate ralloc;

fn main() {
    let a = Box::new(1);
    let b = Box::new(2);
    let c = Box::new(3);

    assert_eq!(*a, 1);
    assert_eq!(*b, 2);
    assert_eq!(*c, 3);
}
