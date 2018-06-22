//! The global allocator.
//!
//! This contains primitives for the cross-thread allocator.

use prelude::*;

use core::{mem, ops};

use bookkeeper::{self, Allocator, Bookkeeper};
use {brk, sync};

use shim::config;

#[cfg(feature = "tls")]
use tls;

/// Alias for the wrapper type of the thread-local variable holding the local
/// allocator.
#[cfg(feature = "tls")]
type ThreadLocalAllocator =
    MoveCell<Option<LazyInit<fn() -> LocalAllocator, LocalAllocator>>>;

/// The global default allocator.
// TODO: Remove these filthy function pointers.
static GLOBAL_ALLOCATOR: sync::Mutex<
    LazyInit<fn() -> GlobalAllocator, GlobalAllocator>,
> = sync::Mutex::new(LazyInit::new(GlobalAllocator::init));
#[cfg(feature = "tls")]
tls! {
    /// The thread-local allocator.
    static THREAD_ALLOCATOR: ThreadLocalAllocator = MoveCell::new(Some(LazyInit::new(LocalAllocator::init)));
}

/// Temporarily get the allocator.
///
/// This is simply to avoid repeating ourself, so we let this take care of the
/// hairy stuff:
///
/// 1. Initialize the allocator if needed.
/// 2. If the allocator is not yet initialized, fallback to the global
/// allocator. 3. Unlock/move temporarily out of reference.
///
/// This is a macro due to the lack of generic closure, which makes it
/// impossible to have one closure for both cases (global and local).
// TODO: Instead of falling back to the global allocator, the thread dtor
// should be set such that it run after the TLS keys that might be declared.
macro_rules! get_allocator {
    (| $v:ident | $b:expr) => {{
        // Get the thread allocator, if TLS is enabled
        #[cfg(feature = "tls")]
        {
            THREAD_ALLOCATOR.with(|thread_alloc| {
                if let Some(mut thread_alloc_original) = thread_alloc.replace(None) {
                    let res = {
                        // Call the closure involved.
                        let $v = thread_alloc_original.get();
                        $b
                    };

                    // Put back the original allocator.
                    thread_alloc.replace(Some(thread_alloc_original));

                    res
                } else {
                    // The local allocator seems to have been deinitialized, for this reason we
                    // fallback to the global allocator.
                    log!(
                        WARNING,
                        "Accessing the allocator after deinitialization of the local allocator."
                    );

                    // Lock the global allocator.
                    let mut guard = GLOBAL_ALLOCATOR.lock();

                    // Call the block in question.
                    let $v = guard.get();
                    $b
                }
            })
        }

        // TLS is disabled, just use the global allocator.
        #[cfg(not(feature = "tls"))]
        {
            // Lock the global allocator.
            let mut guard = GLOBAL_ALLOCATOR.lock();

            // Call the block in question.
            let $v = guard.get();
            $b
        }
    }};
}

/// Derives `Deref` and `DerefMut` to the `inner` field.
///
/// This requires importing `core::ops`.
macro_rules! derive_deref {
    ($imp:ty, $target:ty) => {
        impl ops::Deref for $imp {
            type Target = $target;

            fn deref(&self) -> &$target {
                &self.inner
            }
        }

        impl ops::DerefMut for $imp {
            fn deref_mut(&mut self) -> &mut $target {
                &mut self.inner
            }
        }
    };
}

/// Global SBRK-based allocator.
///
/// This will extend the data segment whenever new memory is needed. Since this
/// includes leaving userspace, this shouldn't be used when other allocators
/// are available (i.e. the bookkeeper is local).
struct GlobalAllocator {
    // The inner bookkeeper.
    inner: Bookkeeper,
}

impl GlobalAllocator {
    /// Initialize the global allocator.
    fn init() -> GlobalAllocator {
        /// Logging...
        log!(NOTE, "Initializing the global allocator.");

        // The initial acquired segment.
        let (aligner, initial_segment, excessive) = brk::lock().canonical_brk(
            8 * bookkeeper::EXTRA_ELEMENTS * mem::size_of::<Block>(),
            mem::align_of::<Block>(),
        );

        // Initialize the new allocator.
        let mut res = GlobalAllocator {
            inner: Bookkeeper::new(unsafe {
                // LAST AUDIT: 2016-08-21 (Ticki).

                Vec::from_raw_parts(initial_segment, 0)
            }),
        };

        // Free the secondary space.
        res.push(aligner);
        res.push(excessive);

        res
    }
}

derive_deref!(GlobalAllocator, Bookkeeper);

impl Allocator for GlobalAllocator {
    #[inline]
    fn alloc_fresh(&mut self, size: usize, align: usize) -> Block {
        // Obtain what you need.
        let (alignment_block, res, excessive) =
            brk::lock().canonical_brk(size, align);

        // Add it to the list. This will not change the order, since the
        // pointer is higher than all the previous blocks (BRK extends
        // the data segment). Although, it is worth noting that the
        // stack is higher than the program break.
        self.push(alignment_block);
        self.push(excessive);

        res
    }

    fn on_new_memory(&mut self) {
        if self.total_bytes() > config::OS_MEMTRIM_LIMIT {
            // memtrim the fack outta 'em.

            // Pop the last block.
            let block = self
                .pop()
                .expect("The byte count on the global allocator is invalid.");

            // Check if the memtrim is worth it.
            if block.size() >= config::OS_MEMTRIM_WORTHY {
                /// Logging...
                log!(NOTE, "Memtrimming the global allocator.");

                // Release the block to the OS.
                if let Err(block) = brk::lock().release(block) {
                    // It failed, put the block back.
                    // TODO: This can be done faster.
                    self.push(block);
                }

            // Note that this block is the only block next to the program
            // break, due to the segments being as long as
            // possible. For that reason, repeating to push and
            // release would fail.
            } else {
                /// Logging...
                log!(WARNING, "Memtrimming for the global allocator failed.");

                // Push the block back.
                // TODO: This can be done faster.
                self.push(block);
            }
        }
    }
}

/// A local allocator.
///
/// This acquires memory from the upstream (global) allocator, which is
/// protected by a `Mutex`.
#[cfg(feature = "tls")]
pub struct LocalAllocator {
    // The inner bookkeeper.
    inner: Bookkeeper,
}

#[cfg(feature = "tls")]
impl LocalAllocator {
    /// Initialize the local allocator.
    #[cfg(feature = "tls")]
    fn init() -> LocalAllocator {
        /// The destructor of the local allocator.
        ///
        /// This will simply free everything to the global allocator.
        extern "C" fn dtor(alloc: &ThreadLocalAllocator) {
            /// Logging...
            log!(NOTE, "Deinitializing and freeing the local allocator.");

            // This is important! The thread destructors guarantee no other,
            // and thus one could use the allocator _after_ this
            // destructor have been finished. In fact, this is a
            // real problem, and happens when using `Arc` and
            // terminating the main thread, for this reason we place
            // `None` as a permanent marker indicating that the allocator is
            // deinitialized. After such a state is in place, all
            // allocation calls will be redirected to the global
            // allocator, which is of course still usable at this
            // moment.
            let alloc = alloc
                .replace(None)
                .expect("Thread-local allocator is already freed.");

            // Lock the global allocator.
            let mut global_alloc = GLOBAL_ALLOCATOR.lock();
            let global_alloc = global_alloc.get();

            // TODO: we know this is sorted, so we could abuse that fact to
            // faster insertion in the global allocator.

            alloc
                .into_inner()
                .inner
                .for_each(move |block| global_alloc.free(block));
        }

        /// Logging...
        log!(NOTE, "Initializing the local allocator.");

        // The initial acquired segment.
        let initial_segment = GLOBAL_ALLOCATOR.lock().get().alloc(
            8 * bookkeeper::EXTRA_ELEMENTS * mem::size_of::<Block>(),
            mem::align_of::<Block>(),
        );

        unsafe {
            // LAST AUDIT: 2016-08-21 (Ticki).

            // Register the thread destructor on the current thread.
            THREAD_ALLOCATOR.register_thread_destructor(dtor);

            LocalAllocator {
                inner: Bookkeeper::new(Vec::from_raw_parts(initial_segment, 0)),
            }
        }
    }
}

#[cfg(feature = "tls")]
derive_deref!(LocalAllocator, Bookkeeper);

#[cfg(feature = "tls")]
impl Allocator for LocalAllocator {
    #[inline]
    fn alloc_fresh(&mut self, size: usize, align: usize) -> Block {
        // Get the block from the global allocator. Please note that we cannot
        // canonicalize `size`, due to freeing excessive blocks would change
        // the order.
        GLOBAL_ALLOCATOR.lock().get().alloc(size, align)
    }

    #[inline]
    fn on_new_memory(&mut self) {
        // The idea is to free memory to the global allocator to unify small
        // stubs and avoid fragmentation and thread accumulation.
        if self.total_bytes() < config::FRAGMENTATION_SCALE * self.len()
            || self.total_bytes() > config::LOCAL_MEMTRIM_LIMIT
        {
            // Log stuff.
            log!(NOTE, "Memtrimming the local allocator.");

            // Lock the global allocator.
            let mut global_alloc = GLOBAL_ALLOCATOR.lock();
            let global_alloc = global_alloc.get();

            while let Some(block) = self.pop() {
                // Pop'n'free.
                global_alloc.free(block);

                // Memtrim 'till we won't memtrim anymore.
                if self.total_bytes() < config::LOCAL_MEMTRIM_STOP {
                    break;
                }
            }
        }
    }
}

/// Allocate a block of memory.
///
/// # Errors
///
/// The OOM handler handles out-of-memory conditions.
#[inline]
pub fn alloc(size: usize, align: usize) -> *mut u8 {
    log!(
        CALL,
        "Allocating buffer of size {} (align {}).",
        size,
        align
    );

    get_allocator!(|alloc| Pointer::from(alloc.alloc(size, align)).get())
}

/// Free a buffer.
///
/// Note that this do not have to be a buffer allocated through ralloc. The
/// only requirement is that it is not used after the free.
///
/// # Important!
///
/// You should only allocate buffers allocated through `ralloc`. Anything else
/// is considered invalid.
///
/// # Errors
///
/// The OOM handler handles out-of-memory conditions.
///
/// # Safety
///
/// Rust assume that the allocation symbols returns correct values. For this
/// reason, freeing invalid pointers might introduce memory unsafety.
///
/// Secondly, freeing an used buffer can introduce use-after-free.
#[inline]
pub unsafe fn free(ptr: *mut u8, size: usize) {
    log!(CALL, "Freeing buffer of size {}.", size);

    get_allocator!(
        |alloc| alloc.free(Block::from_raw_parts(Pointer::new(ptr), size))
    )
}

/// Reallocate memory.
///
/// Reallocate the buffer starting at `ptr` with size `old_size`, to a buffer
/// starting at the returned pointer with size `size`.
///
/// # Important!
///
/// You should only reallocate buffers allocated through `ralloc`. Anything
/// else is considered invalid.
///
/// # Errors
///
/// The OOM handler handles out-of-memory conditions.
///
/// # Safety
///
/// Due to being able to potentially memcpy an arbitrary buffer, as well as
/// shrinking a buffer, this is marked unsafe.
#[inline]
pub unsafe fn realloc(
    ptr: *mut u8,
    old_size: usize,
    size: usize,
    align: usize,
) -> *mut u8 {
    log!(
        CALL,
        "Reallocating buffer of size {} to new size {}.",
        old_size,
        size
    );

    get_allocator!(|alloc| Pointer::from(alloc.realloc(
        Block::from_raw_parts(Pointer::new(ptr), old_size),
        size,
        align
    )).get())
}

/// Try to reallocate the buffer _inplace_.
///
/// In case of success, return the new buffer's size. On failure, return the
/// old size.
///
/// This can be used to shrink (truncate) a buffer as well.
///
/// # Safety
///
/// Due to being able to shrink (and thus free) the buffer, this is marked
/// unsafe.
#[inline]
pub unsafe fn realloc_inplace(
    ptr: *mut u8,
    old_size: usize,
    size: usize,
) -> Result<(), ()> {
    log!(
        CALL,
        "Inplace reallocating buffer of size {} to new size {}.",
        old_size,
        size
    );

    get_allocator!(|alloc| {
        if alloc
            .realloc_inplace(
                Block::from_raw_parts(Pointer::new(ptr), old_size),
                size,
            )
            .is_ok()
        {
            Ok(())
        } else {
            Err(())
        }
    })
}
