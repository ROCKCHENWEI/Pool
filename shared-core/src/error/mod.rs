//! Error Handling Module
//!
//! Provides comprehensive error handling, retry mechanisms, and timeout utilities.

pub mod retry;
pub mod timeout;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Error types for the pool-core library
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolError {
    /// Connection error
    ConnectionError { message: String },
    /// Timeout error
    TimeoutError { operation: String, duration_ms: u64 },
    /// Authentication error
    AuthenticationError { message: String },
    /// Rate limit error
    RateLimitError { retry_after_secs: Option<u64> },
    /// Invalid input error
    InvalidInputError { field: String, message: String },
    /// Processing error
    ProcessingError { message: String, details: Option<String> },
    /// Resource not found error
    NotFoundError { resource: String },
    /// Internal error
    InternalError { message: String },
}

impl std::fmt::Display for PoolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PoolError::ConnectionError { message } => write!(f, "Connection error: {}", message),
            PoolError::TimeoutError { operation, duration_ms } => {
                write!(f, "Timeout after {}ms for operation: {}", duration_ms, operation)
            }
            PoolError::AuthenticationError { message } => write!(f, "Authentication error: {}", message),
            PoolError::RateLimitError { retry_after_secs } => {
                write!(f, "Rate limited. Retry after: {:?}", retry_after_secs)
            }
            PoolError::InvalidInputError { field, message } => {
                write!(f, "Invalid input for '{}': {}", field, message)
            }
            PoolError::ProcessingError { message, details } => {
                if let Some(d) = details {
                    write!(f, "Processing error: {} ({})", message, d)
                } else {
                    write!(f, "Processing error: {}", message)
                }
            }
            PoolError::NotFoundError { resource } => write!(f, "Resource not found: {}", resource),
            PoolError::InternalError { message } => write!(f, "Internal error: {}", message),
        }
    }
}

impl std::error::Error for PoolError {}

impl From<PoolError> for anyhow::Error {
    fn from(err: PoolError) -> Self {
        anyhow!("{}", err)
    }
}

/// Error context for tracking error chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Error message
    pub message: String,
    /// Error type
    pub error_type: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional context
    pub context: serde_json::Value,
    /// Stack trace (optional)
    pub stack_trace: Option<String>,
}

impl ErrorContext {
    /// Create a new error context
    pub fn new(message: String, error_type: String) -> Self {
        Self {
            message,
            error_type,
            timestamp: chrono::Utc::now(),
            context: serde_json::json!({}),
            stack_trace: None,
        }
    }

    /// Add context to the error
    pub fn with_context(mut self, key: &str, value: &str) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.context {
            map.insert(key.to_string(), serde_json::json!(value));
        }
        self
    }

    /// Add stack trace
    pub fn with_stack_trace(mut self, trace: String) -> Self {
        self.stack_trace = Some(trace);
        self
    }

    /// Convert to JSON
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| anyhow!("Failed to serialize error context: {}", e))
    }
}

/// Result type alias for pool operations
pub type PoolResult<T> = std::result::Result<T, PoolError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_error_display() {
        let err = PoolError::ConnectionError {
            message: "Failed to connect".to_string(),
        };
        assert!(err.to_string().contains("Connection error"));
    }

    #[test]
    fn test_error_context() {
        let ctx = ErrorContext::new("Test error".to_string(), "TestType".to_string())
            .with_context("key", "value");

        assert_eq!(ctx.message, "Test error");
        assert!(ctx.to_json().is_ok());
    }
}
