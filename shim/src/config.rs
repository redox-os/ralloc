//! Configuration.
//!
//! This module contains anything which can be tweaked and customized to the users preferences.

use core::{intrinsics, cmp};

/// The memtrim limit.
///
/// Whenever this is exceeded, the allocator will try to free as much memory to the system
/// as it can.
pub const OS_MEMTRIM_LIMIT: usize = 200000000;
/// Minimum size before a block is worthy to memtrim.
pub const OS_MEMTRIM_WORTHY: usize = 4000;

/// The fragmentation scale constant.
///
/// This is used for determining the minimum avarage block size before locally memtrimming.
pub const FRAGMENTATION_SCALE: usize = 10;
/// The local memtrim limit.
///
/// Whenever an local allocator has more free bytes than this value, it will be memtrimmed.
pub const LOCAL_MEMTRIM_LIMIT: usize = 16384;
/// The local memtrim chock.
///
/// The local memtrimming will continue until the allocator has less memory (in bytes, of course)
/// than this value.
pub const LOCAL_MEMTRIM_STOP: usize = 1024;

/// The minimum log level.
pub const MIN_LOG_LEVEL: u8 = 0;

/// The default OOM handler.
#[cold]
pub fn default_oom_handler() -> ! {
    // Log some message.
    log("\x1b[31;1mThe application ran out of memory. Aborting.\x1b[m\n");

    unsafe {
        intrinsics::abort();
    }
}

/// Write to the log.
///
/// This points to stderr, but could be changed arbitrarily.
#[cfg(not(target_os = "redox"))]
pub fn log(s: &str) -> usize {
    unsafe { syscall!(WRITE, 2, s.as_ptr(), s.len()) }
}

/// Write to the log.
///
/// This points to stderr, but could be changed arbitrarily.
#[cfg(target_os = "redox")]
pub fn log(s: &str) -> usize {
    ::syscall::write(2, s.as_bytes()).unwrap_or(!0)
}

/// Canonicalize a fresh allocation.
///
/// The return value specifies how much _more_ space is requested to the fresh allocator.
// TODO: Move to shim.
#[inline]
pub fn extra_fresh(size: usize) -> usize {
    /// The multiplier.
    ///
    /// The factor determining the linear dependence between the minimum segment, and the acquired
    /// segment.
    const MULTIPLIER: usize = 2;
    /// The minimum extra size to be BRK'd.
    const MIN_EXTRA: usize = 64;
    /// The maximal amount of _extra_ bytes.
    const MAX_EXTRA: usize = 1024;

    cmp::max(MIN_EXTRA, cmp::min(MULTIPLIER * size, MAX_EXTRA))
}

/// Canonicalize a BRK request.
///
/// Syscalls can be expensive, which is why we would rather accquire more memory than necessary,
/// than having many syscalls acquiring memory stubs. Memory stubs are small blocks of memory,
/// which are essentially useless until merge with another block.
///
/// To avoid many syscalls and accumulating memory stubs, we BRK a little more memory than
/// necessary. This function calculate the memory to be BRK'd based on the necessary memory.
///
/// The return value specifies how much _more_ space is requested.
// TODO: Move to shim.
#[inline]
pub fn extra_brk(size: usize) -> usize {
    // TODO: Tweak this.
    /// The BRK multiplier.
    ///
    /// The factor determining the linear dependence between the minimum segment, and the acquired
    /// segment.
    const MULTIPLIER: usize = 2;
    /// The minimum extra size to be BRK'd.
    const MIN_EXTRA: usize = 1024;
    /// The maximal amount of _extra_ bytes.
    const MAX_EXTRA: usize = 65536;

    cmp::max(MIN_EXTRA, cmp::min(MULTIPLIER * size, MAX_EXTRA))
}
