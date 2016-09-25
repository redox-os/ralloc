//! Allocator logging.

use core::{fmt, mem};

use config;

// TODO: Consider implementing flushing.

/// Write to the log.
///
/// This points to stderr, but could be changed arbitrarily.
pub fn write(lv: u8, kind: &str, args: fmt::Arguments, file: &str, line: u32) {
    if lv >= config::MIN_LOG_LEVEL {
        // The buffer. We add an extra slot, which is reserved for overflows. If the buffer is
        // filled, we will insert a "..." character to inform the user that there is more in this
        // message. We start out with all dots, so we don't have to set these up later on in case
        // of the buffer being full.
        let mut buffer = [b'.'; config::LOG_BUFFER_SIZE + 3];

        // The bytes of the buffer that are filled.
        let mut filled;

        {
            // We emulate the writing semantics by having a newtype which implements `fmt::Write`. All
            // this type does is holding a slice, which is updated when new bytes are written. This
            // slice will point to the buffer declared above.
            let mut writer = BufWriter {
                buffer: &mut buffer[..config::LOG_BUFFER_SIZE],
            };
            write!(writer, "{:10}{:60} (@ {}:{})", kind, args, file, line).unwrap();
            filled = config::LOG_BUFFER_SIZE - writer.buffer.len();
        }

        // Write the dots to the end if the buffer was full.
        if filled == config::LOG_BUFFER_SIZE {
            filled += 3;
        }

        // Finally, write it to the logging target.
        assert!(unsafe {
            syscall!(WRITE, config::LOG_TARGET, &buffer[0], filled)
        } != !0, "Failed to write to logging target.");
    }
}

/// A logging buffer.
///
/// This simply keeps track of the buffer by maintaining a slice representing the remaining part of
/// the buffer.
struct BufWriter<'a> {
    /// A view into the remaining part of the buffer.
    buffer: &'a mut [u8],
}

impl<'a> fmt::Write for BufWriter<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // Find the appropriate length of the copied subbuffer.
        let amt = cmp::min(s.len(), self.buffer.len());
        // Split the buffer.
        let buf = mem::replace(self.buffer, &mut [])[..amt];
        // Memcpy the content of the string.
        buf.copy_from_slice(&s.as_bytes()[..amt]);

        Ok(())
    }
}
