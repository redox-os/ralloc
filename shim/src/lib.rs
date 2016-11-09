//! Symbols and externs that `ralloc` depends on.
//!
//! This crate provides implementation/import of these in Linux, BSD, and Mac OS.

#![cfg_attr(not(redox), feature(linkage))]
#![no_std]
#![warn(missing_docs)]

#[cfg(target_os = "redox")]
mod redox;

#[cfg(target_os = "redox")]
pub use redox::*;

#[cfg(not(target_os = "redox"))]
mod unix;

#[cfg(not(target_os = "redox"))]
pub use unix::*;
