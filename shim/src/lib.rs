//! Symbols and externs that `ralloc` depends on.
//!
//! This crate provides implementation/import of these in Linux, BSD, and Mac OS.
//!
//! # Important
//!
//! You CANNOT use libc library calls, due to no guarantees being made about allocations of the
//! functions in the POSIX specification. Therefore, we use the system calls directly.

#![feature(linkage, core_intrinsics)]
#![no_std]
#![warn(missing_docs)]

#[macro_use]
extern crate syscall;

pub mod config;
pub mod debug;
pub mod log;
pub mod syscalls;
pub mod thread_destructor;
