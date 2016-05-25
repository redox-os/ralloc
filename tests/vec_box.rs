extern crate ralloc;

#[test]
fn test() {
    let mut vec = Vec::new();

    for i in 0..0xFFFF {
        vec.push(Box::new(i));
    }

    assert_eq!(*vec[0xDEAD], 0xDEAD);
    assert_eq!(*vec[0xBEAF], 0xBEAF);
    assert_eq!(*vec[0xABCD], 0xABCD);
    assert_eq!(*vec[0xFFAB], 0xFFAB);
    assert_eq!(*vec[0xAAAA], 0xAAAA);

    for i in 0xFFFF..0 {
        assert_eq!(*vec.pop().unwrap(), i);
    }

    for i in 0..0xFFFF {
        *vec[i] = 0;
        assert_eq!(*vec[i], 0);
    }
}
