//! Pool Core - Shared Rust Library
//!
//! This is the shared core library for the Pool project,
//! a localized AI video generation engine.
//!
//! This library provides:
//! - Data models and types
//! - Database operations
//! - Engine logic
//! - API interfaces
//! - ComfyUI integration
//! - OpenClaw integration
//! - FFI bindings for macOS (Swift) and Windows (C#)

pub mod models;

pub use models::{Connection, Node, NodeParam, NodeType, Project, Shot, ShotStatus, Workflow};

/// Placeholder function for library initialization
pub fn init() {
    // Library initialization logic will be added here
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        init();
    }
}
