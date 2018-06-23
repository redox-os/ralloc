extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

#[test]
fn simple_string() {
    util::multiply(|| {
        assert_eq!(&String::from("you only live twice"), "you only live twice");
        assert_eq!(&String::from("wtf have you smoked"), "wtf have you smoked");
        assert_eq!(&String::from("get rekt m8"), "get rekt m8");
    });
}
