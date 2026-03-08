//! Database layer for Pool Core
//!
//! This module provides SQLite-based persistence for all data models.
//!
//! # Architecture
//!
//! - `schema` - SQL schema definitions
//! - `repository` - Database operations (CRUD)
//!
//! # Usage
//!
//! ```ignore
//! use pool_core::db::Database;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create in-memory database
//!     let db = Database::new(":memory:").await.unwrap();
//!
//!     // Check health
//!     assert!(db.is_healthy().await);
//! }
//! ```

mod repository;
mod schema;

pub use repository::Database;
pub use schema::SCHEMA;
