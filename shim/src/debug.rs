//! Bindings to debuggers.

extern {
    /// Valgrind symbol to declare memory undefined.
    fn valgrind_make_mem_undefined(ptr: *const u8, size: usize);
    /// Valgrind symbol to declare memory freed.
    fn valgrind_freelike_block(ptr: *const u8, size: usize);
}

/// Mark this segment undefined to the debugger.
pub fn mark_undefined(ptr: *const u8, size: usize) {
    unsafe { valgrind_make_mem_undefined(ptr, size) }
}
/// Mark this segment free to the debugger.
pub fn mark_free(ptr: *const u8, size: usize) {
    unsafe { valgrind_freelike_block(ptr, size) }
}
