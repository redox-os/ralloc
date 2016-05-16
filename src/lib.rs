//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.

#![cfg_attr(feature = "allocator", allocator)]
#![no_std]

#![feature(allocator, const_fn, core_intrinsics, stmt_expr_attributes, unique, iter_arith)]

#![warn(missing_docs)]

#[cfg(target_os = "redox")]
extern crate system;
#[cfg(not(target_os = "redox"))]
#[macro_use]
extern crate syscall;

pub mod allocator;
pub mod block;
pub mod bookkeeper;
pub mod fail;
pub mod sys;
