extern crate ralloc;

#[test]
fn minimal() {
    let mut a = Box::new(1);
    let mut b = Box::new(2);
    let mut c = Box::new(3);

    assert_eq!(*a, 1);
    assert_eq!(*b, 2);
    assert_eq!(*c, 3);
}
