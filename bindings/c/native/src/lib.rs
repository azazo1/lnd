//! `liblnd` 的 C ABI 入口.
//!
//! 这个 crate 只负责导出稳定的 C ABI 和生成 `include/lnd.h`.
//! Rust 核心实现保留在根 crate `lnd` 中.

#[allow(unsafe_op_in_unsafe_fn)]
pub mod ffi;
