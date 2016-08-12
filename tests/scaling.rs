extern crate ralloc;

mod util;

#[test]
fn big_alloc() {
    util::multiply(|| {
        let mut vec = Vec::new();
        let mut rand = 3u64;

        for _ in 0..0xBFFF {
            rand ^= 0xABFABFABFABF;
            rand = rand.rotate_left(3);

            util::acid(|| vec.push(rand));
        }
    });
}

#[test]
fn many_small_allocs() {
    util::multiply(|| {
        let mut vec = Vec::new();
        let mut rand = 3u64;

        for _ in 0..3000 {
            rand ^= 0xABFABFABFABF;
            rand = rand.rotate_left(3);

            util::acid(|| vec.push(Box::new(rand)));
        }
    });
}
