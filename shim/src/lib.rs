//! Symbols and externs that `ralloc` depends on.
//!
//! This crate provides implementation/import of these in Linux, BSD, and Mac OS.

#![cfg_attr(not(redox), feature(linkage))]
#![no_std]
#![warn(missing_docs)]

#[cfg(redox)]
mod redox;

#[cfg(redox)]
pub use redox::*;

#[cfg(not(redox))]
mod unix;

#[cfg(not(redox))]
pub use unix::*;
