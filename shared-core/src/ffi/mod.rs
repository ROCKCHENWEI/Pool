//! FFI (Foreign Function Interface) Module
//!
//! This module provides C-compatible bindings for cross-language interoperability.
//! It enables Swift (macOS) and C# (Windows) to call Rust shared-core functions.

mod swift;

pub use swift::*;
