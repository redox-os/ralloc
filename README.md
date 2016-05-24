# ralloc

Redox's fast & memory efficient userspace allocator.

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

`ralloc` is now ready to roll!

## Features

### Custom out-of-memory handlers

You can set custom OOM handlers, by:

```rust
extern crate ralloc;
use fail::set_oom_handler;

fn my_handler() -> ! {
    println!("Oh no. Blame somebody.");
}

fn main() {
    set_oom_handler(my_handler);
    // Do some stuff...
}
```

### Debug check: double free

Ooh, this one is a cool one. `ralloc` detects various memory bugs when compiled
with `debug_assertions`. These checks include double free checks:

```rust
fn main() {
    // We start by allocating some stuff.
    let a = Box::new(500u32);
    // Then we memcpy the pointer (this is UB).
    let b = Box::from_raw(&a as *mut u32);
    // Now both destructors are called. First a, then b, which is a double
    // free. Luckily, ralloc provides a nice message for you, when in debug
    // mode:
    //    Assertion failed: Double free.

    // Setting RUST_BACKTRACE allows you to get a stack backtrace, so that you
    // can find where the double free occurs.
}
```

### Partial deallocation

Many allocators limits deallocations to be allocated block, that is, you cannot
perform arithmetics or split it. `ralloc` does not have such a limitation:

```rust
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

### Seperate deallocation

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
        ralloc::free(&mut BUFFER as *mut u8, 256);
    }

    // Do some allocation.
    assert_eq!(*Box::new(0xDEED), 0xDEED);
}
```

### Thread local allocator

TODO

### Safe SBRK

TODO

### Lock reuse

TODO

### Platform agnostic

TODO
