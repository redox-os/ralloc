extern crate ralloc;

mod util;

#[test]
fn vec_box() {
    util::multiply(|| {
        let mut vec = Vec::new();

        for i in 0..0xFFF {
            util::acid(|| {
                vec.push(Box::new(i));
            });
        }

        assert_eq!(*vec[0xEAD], 0xEAD);
        assert_eq!(*vec[0xEAF], 0xEAF);
        assert_eq!(*vec[0xBCD], 0xBCD);
        assert_eq!(*vec[0xFAB], 0xFAB);
        assert_eq!(*vec[0xAAA], 0xAAA);

        for i in 0xFFF..0 {
            assert_eq!(*vec.pop().unwrap(), i);
        }

        for i in 0..0xFFF {
            *vec[i] = 0;
            assert_eq!(*vec[i], 0);
        }
    });
}
