//! Pseudorandom number generators.
//!
//! `ralloc` makes use of probabilistic data structures which often requires randomness. This
//! module provides functions giving pseudorandom output based on `xorshift+`.

use core::cell::Cell;

tls! {
    /// The randomness state.
    ///
    /// This is updated when a new random integer is read.
    // TODO: Upon new threads, this should fetch from a global variable to ensure unique stream.
    // Currently, this doesn't matter, but it might for data structures used in the future.
    static STATE: Cell<[u64; 2]> = Cell::new([0xBADF00D1, 0xDEADBEEF]);
}

/// Get a pseudorandom integer.
///
/// Note that this is full-cycle, so apply a modulo when true equidistribution is needed.
#[inline]
pub fn get() -> u64 {
    STATE.with(|state| {
        // Fetch the state.
        let mut state = state.get();

        // Store the first and second part.
        let mut x = state[0];
        let y = state[1];

        // Put the second part into the first slot.
        state[0] = y;
        // Twist the first slot.
        x ^= x << 23;
        // Update the second slot.
        state[1] = x ^ y ^ (x >> 17) ^ (y >> 26);

        // Put back the state.
        STATE.set(state);

        // Generate the final integer.
        state[1].wrapping_add(y);
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn distinct() {
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
        assert!(get() != get());
    }
}
