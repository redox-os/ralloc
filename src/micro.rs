//! Micro slots for caching small allocations.

// TODO needs tests and documentation.

use prelude::*;

use core::{marker, mem};

const CACHE_LINE_SIZE: usize = 128;
const CACHE_LINES: usize = 32;

/// A "microcache".
///
/// A microcache consists of some number of equal sized slots, whose state is stored as bitflags.
pub struct MicroCache {
    free: u32,
    lines: [CacheLine; CACHE_LINES],
}

impl MicroCache {
    pub const fn new() -> MicroCache {
        MicroCache {
            free: !0,
            lines: [CacheLine::new(); CACHE_LINES],
        }
    }

    pub fn alloc(&mut self, size: usize, align: usize) -> Result<Block, ()> {
        if size <= CACHE_LINE_SIZE && self.free != 0 {
            let ind = self.free.trailing_zeros();
            let line = &mut self.lines[ind as usize];
            let res = unsafe { line.take(size) };

            if res.aligned_to(align) {
                self.free ^= 1u32.wrapping_shl(ind);

                return Ok(res);
            } else {
                line.reset();
            }
        }

        Err(())
    }

    pub fn free(&mut self, mut block: Block) -> Result<(), Block> {
        let res = block.pop();
        let ptr: Pointer<u8> = block.into();
        let ind = (*ptr as usize - &self.lines as *const CacheLine as usize) / mem::size_of::<Block>();

        if let Some(line) = self.lines.get_mut(ind) {
            line.used -= res.size();
            if line.used == 0 {
                debug_assert!(self.free & 1u32.wrapping_shl(ind as u32) == 0, "Freeing a block \
                              already marked as free.");
                self.free ^= 1u32.wrapping_shl(ind as u32);
            }

            Ok(())
        } else {
            Err(res)
        }
    }
}

#[derive(Clone, Copy)]
struct CacheLine {
    /// The cache line's data.
    ///
    /// We use `u32` as a hack to be able to derive `Copy`.
    data: [u32; CACHE_LINE_SIZE / 4],
    used: usize,
    _static: marker::PhantomData<&'static mut [u8]>,
}

impl CacheLine {
    pub const fn new() -> CacheLine {
        CacheLine {
            data: [0; CACHE_LINE_SIZE / 4],
            used: 0,
            _static: marker::PhantomData,
        }
    }

    fn reset(&mut self) {
        self.used = 0;
    }

    unsafe fn take(&mut self, size: usize) -> Block {
        debug_assert!(self.used == 0, "Block not freed!");

        self.used = size;
        Block::from_raw_parts(Pointer::new(&mut self.data[0] as *mut u32 as *mut u8), size)
    }
}
