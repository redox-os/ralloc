//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.

#![cfg_attr(not(test), feature(oom))]
#![cfg_attr(test, feature(const_fn))]

#![feature(alloc)]
#![feature(heap_api)]
#![feature(stmt_expr_attributes)]
#![feature(unique)]

#![warn(missing_docs)]

#[cfg(target_os = "redox")]
extern crate system;
#[cfg(not(target_os = "redox"))]
#[macro_use]
extern crate syscall;

#[macro_use]
extern crate extra;
extern crate alloc;

pub mod block;
pub mod bookkeeper;
pub mod sys;
