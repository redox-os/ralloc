//! **Ralloc:** The memory efficient allocator.
//!
//! This crates define the user space allocator for Redox, which emphasizes performance and memory
//! efficiency.
//!
//! # Ralloc seems to reimplement everything. Why?
//!
//! Memory allocators cannot depend on libraries or primitives, which allocates. This is a
//! relatively strong condition, which means that you are forced to rewrite primitives and make
//! sure no allocation ever happens.

#![cfg_attr(feature = "allocator", allocator)]
#![cfg_attr(feature = "clippy", feature(plugin))]
#![cfg_attr(feature = "clippy", plugin(clippy))]

#![no_std]

#![feature(allocator, const_fn, core_intrinsics, stmt_expr_attributes, drop_types_in_const,
           nonzero, optin_builtin_traits, type_ascription, question_mark, thread_local, linkage,
           try_from)]
#![warn(missing_docs, cast_precision_loss, cast_sign_loss, cast_possible_wrap,
        cast_possible_truncation, filter_map, if_not_else, items_after_statements,
        invalid_upcast_comparisons, mutex_integer, nonminimal_bool, shadow_same, shadow_unrelated,
        single_match_else, string_add, string_add_assign, wrong_pub_self_convention)]

#[macro_use]
#[no_link]
extern crate unborrow;
extern crate ralloc_shim as shim;

#[macro_use]
mod log;
#[macro_use]
#[cfg(feature = "tls")]
mod tls;
#[cfg(feature = "allocator")]
mod symbols;

mod allocator;
mod block;
mod bookkeeper;
mod brk;
mod cell;
mod fail;
mod lazy_init;
mod leak;
mod prelude;
mod ptr;
mod sync;
mod vec;

pub use allocator::{alloc, free, realloc, realloc_inplace};
pub use brk::sbrk;
pub use fail::set_oom_handler;
#[cfg(feature = "tls")]
pub use fail::set_thread_oom_handler;
