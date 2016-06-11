# ralloc

Redox's fast & memory efficient userspace allocator.

## A note on its state.

It fully works, although it is relatively slow, since it haven't been optimized
yet. There is currently no known bugs, but it haven't been carefully reviewed
yet, so avoid using it in security critical programs.

I consider the state of the code quality very good.

## Using ralloc

Add ralloc to `Cargo.toml`:

```toml
[dependencies.ralloc]
git = "https://github.com/redox-os/ralloc.git"
```

then import it in your main file:

```rust
extern crate ralloc;
```

ralloc is now ready to roll!

Note that ralloc cannot coexist with another allocator, unless they're deliberately compatible.

## Features

### Custom out-of-memory handlers

You can set custom OOM handlers, by:

```rust
extern crate ralloc;

fn my_handler() -> ! {
    println!("Oh no. Blame somebody.");
}

fn main() {
    ralloc::lock().set_oom_handler(my_handler);
    // Do some stuff...
}
```

### Debug check: double free

Ooh, this one is a cool one. ralloc detects various memory bugs when compiled
with the `debug_tools` feature. These checks include double free checks:

```rust
extern crate ralloc;

fn main() {
    // We start by allocating some stuff.
    let a = Box::new(500u32);
    // Then we memcpy the pointer (this is UB).
    let b = Box::from_raw(&a as *mut u32);
    // Now both destructors are called. First a, then b, which is a double
    // free. Luckily, ralloc provides a nice message for you, when in debug
    // tools mode:
    //    Assertion failed: Double free.

    // Setting RUST_BACKTRACE allows you to get a stack backtrace, so that you
    // can find where the double free occurs.
}
```

### Debug check: memory leaks.

ralloc got memleak superpowers too! Enable `debug_tools` and do:

```rust
extern crate ralloc;

use std::mem;

fn main() {
    {
        // We start by allocating some stuff.
        let a = Box::new(500u32);
        // We then leak `a`.
        let b = mem::forget(a);
    }
    // The box is now leaked, and the destructor won't be called.

    // To debug this we insert a memory leak check in the end of our programs.
    // This will panic if a memory leak is found (and will be a NOOP without
    // `debug_tools`).
    ralloc::lock().debug_assert_no_leak();
}
```

### Partial deallocation

Many allocators limits deallocations to be allocated block, that is, you cannot
perform arithmetics or split it. ralloc does not have such a limitation:

```rust
extern crate ralloc;

use std::mem;

fn main() {
    // We allocate 200 bytes.
    let vec = vec![0u8; 200];
    // Cast it to a pointer.
    let ptr = vec.as_mut_ptr();

    // To avoid UB, we leak the vector.
    mem::forget(vec);

    // Now, we create two vectors, each being 100 bytes long, effectively
    // splitting the original vector in half.
    let a = Vec::from_raw_parts(ptr, 100, 100);
    let b = Vec::from_raw_parts(ptr.offset(100), 100, 100);

    // Now, the destructor of a and b is called... Without a segfault!
}
```

### Separate deallocation

Another cool feature is that you can deallocate things that weren't even
allocated buffers in the first place!

Consider that you got a unused static variable, that you want to put into the
allocation pool:

```rust
extern crate ralloc;

static mut BUFFER: [u8; 256] = [2; 256];

fn main() {
    // Throw `BUFFER` into the memory pool.
    unsafe {
        ralloc::lock().free(&mut BUFFER as *mut u8, 256);
    }

    // Do some allocation.
    assert_eq!(*Box::new(0xDEED), 0xDEED);
}
```

### Top notch security

If you are willing to trade a little performance, for extra security you can
compile ralloc with the `security` flag. This will, along with other things,
make frees zeroing.

In other words, an attacker cannot for example inject malicious code or data,
which can be exploited when forgetting to initialize the data you allocate.

### Lock reuse

Acquiring a lock sequentially multiple times can be expensive. Therefore,
ralloc allows you to lock the allocator once, and reuse that:

```rust
extern crate ralloc;

fn main() {
    // Get that lock!
    let lock = ralloc::lock();

    // All in one:
    let _ = lock.alloc(4, 2);
    let _ = lock.alloc(4, 2);
    let _ = lock.alloc(4, 2);

    // It is automatically released through its destructor.
}
```

### Security through the type system

ralloc makes heavy use of Rust's type system, to make safety guarantees.
Internally, ralloc has a primitive named `Block`. This is fairly simple,
denoting a contagious segment of memory, but what is interesting is how it is
checked at compile time to be unique. This is done through the affine type
system.

This is just one of many examples.

### Platform agnostic

ralloc is platform independent, with the only requirement of the following symbols:

1. `sbrk`: For extending the data segment size.
2. `sched_yield`: For the spinlock.
3. `memcpy`, `memcmp`, `memset`: Core memory routines.
4. `rust_begin_unwind`: For panicking.

### Local allocators

ralloc allows you to create non-global allocators, for e.g. thread specific purposes:

```rust
extern crate ralloc;

fn main() {
    // We create an allocator.
    let my_alloc = ralloc::Allocator::new();

    // Allocate some stuff through our local allocator.
    let _ = my_alloc.alloc(4, 2);
    let _ = my_alloc.alloc(4, 2);
    let _ = my_alloc.alloc(4, 2);
}
```

### Safe SBRK

ralloc provides a `sbrk`, which can be used safely without breaking the allocator.

### Logging

If you enable the `log` feature, you get detailed locking of the allocator, e.g.

```
|   : BRK'ing a block of size, 80, and alignment 8.            (at bookkeeper.rs:458)
|   : Pushing 0x5578dacb2000[0x0] and 0x5578dacb2050[0xffb8].  (at bookkeeper.rs:490)
|x  : Freeing 0x1[0x0].                                        (at bookkeeper.rs:409)
x|  : BRK'ing a block of size, 4, and alignment 1.             (at bookkeeper.rs:458)
x|  : Pushing 0x5578dacc2008[0x0] and 0x5578dacc200c[0xfffd].  (at bookkeeper.rs:490)
x|x : Reallocating 0x5578dacc2008[0x4] to size 8 with align 1. (at bookkeeper.rs:272)
x|x : Inplace reallocating 0x5578dacc2008[0x4] to size 8.      (at bookkeeper.rs:354)
_|x : Freeing 0x5578dacb2058[0xffb0].                          (at bookkeeper.rs:409)
_|x : Inserting block 0x5578dacb2058[0xffb0].                  (at bookkeeper.rs:635)
```

To the left, you can see the state of the block pool. `x` denotes a non-empty
block, `_` denotes an empty block, and `|` denotes the cursor.

The `a[b]` is a syntax for block on address `a` with size `b`.
