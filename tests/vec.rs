extern crate ralloc;

#[global_allocator]
static ALLOCATOR: ralloc::Allocator = ralloc::Allocator;

mod util;

#[test]
fn simple_vec() {
    util::multiply(|| {
        let mut vec = Vec::new();

        for i in 0..0xFFFF {
            // We're going to annoy the allocator by allocating a small chunk,
            // after which we push.
            let _bx = Box::new(4);
            vec.push(i);
        }

        assert_eq!(vec[0xDEAD], 0xDEAD);
        assert_eq!(vec[0xBEAF], 0xBEAF);
        assert_eq!(vec[0xABCD], 0xABCD);
        assert_eq!(vec[0xFFAB], 0xFFAB);
        assert_eq!(vec[0xAAAA], 0xAAAA);

        for i in 0xFFF..0 {
            util::acid(|| {
                assert_eq!(vec.pop(), Some(i));
            });
        }

        for i in 0..0xFFF {
            vec[i] = 0;
            assert_eq!(vec[i], 0);
        }
    });
}
