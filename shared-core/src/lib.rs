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

pub mod api;
pub mod comfyui;
pub mod db;
pub mod engine;
pub mod ffi;
pub mod models;
pub mod openclaw;

pub use api::{ApiGateway, GenerationTask, TaskStatus, VideoGenerationConfig, VideoGeneratorAdapter, KlingAdapter};
pub use comfyui::{ComfyUIClient, WorkflowTranslator};
pub use db::Database;
pub use engine::{NodeEngine, WorkflowExecutor};
pub use models::{Connection, Node, NodeParam, NodeType, Project, Shot, ShotStatus, Workflow};
pub use openclaw::NodeManager;

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
