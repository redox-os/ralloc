extern crate ralloc;

#[test]
fn test() {
    assert_eq!(&String::from("you only live twice"), "you only live twice");
    assert_eq!(&String::from("wtf have you smoked"), "wtf have you smoked");
    assert_eq!(&String::from("get rekt m8"), "get rekt m8");

    ralloc::lock().debug_assert_no_leak();
}
