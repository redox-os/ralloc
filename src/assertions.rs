use core::fmt::{Write, Result};

pub struct LibcWriter {
    pub fd: i32,
}

extern "C" {
    fn write(fd: i32, buff: *const u8, size: usize) -> isize;
}

impl Write for LibcWriter {
    fn write_str(&mut self, s: &str) -> Result {
        unsafe { write(self.fd, s.as_ptr(), s.len()) };
        Ok(()) //TODO write until done and return Error on error code
    }
}

macro_rules! assert {
    ($e:expr) => {{
        if !$e {
            use core::intrinsics;
            use assertions;
            use core::fmt::Write;

            let _ = writeln!(assertions::LibcWriter{fd: 2}, "assertion failed at {}:{}: {}", file!(), line!(),stringify!($e));
            #[allow(unused_unsafe)]
            unsafe{ intrinsics::abort()}
        }
    }};
    ($e:expr, $( $arg:tt )+) => {{
        if !$e {
            use core::intrinsics;
            use assertions;
            use core::fmt::Write;

            let _ = writeln!(assertions::LibcWriter{fd: 2}, "assertion failed at {}:{}: {}", file!(), line!(), stringify!($e));
            let _ = writeln!(assertions::LibcWriter{fd: 2}, $($arg)+);
            #[allow(unused_unsafe)]
            unsafe{ intrinsics::abort()}
        }
    }}
}

macro_rules! debug_assert {
    ($($arg:tt)*) => (if cfg!(debug_assertions) { assert!($($arg)*); })
}

macro_rules! assert_eq {
    ($left:expr , $right:expr) => ({
        match (&$left, &$right) {
            (left_val, right_val) => {
                assert!(*left_val == *right_val, "(left: `{:?}`, right: `{:?}`)", left_val, right_val)
            }
        }
    })
}

macro_rules! debug_assert {
    ($($arg:tt)*) => (if cfg!(debug_assertions) { assert!($($arg)*); })
}
