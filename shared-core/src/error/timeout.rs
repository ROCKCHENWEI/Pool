//! Timeout Utilities Module
//!
//! Provides timeout handling for async operations.

use anyhow::{anyhow, Result};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::time::{timeout, timeout_at};

/// Timeout configuration
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    /// Total timeout duration
    pub duration: Duration,
    /// Whether to enable timeout
    pub enabled: bool,
    /// Grace period before hard timeout
    pub grace_period: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(30),
            enabled: true,
            grace_period: Duration::from_secs(5),
        }
    }
}

impl TimeoutConfig {
    /// Create a new timeout config
    pub fn new(duration: Duration) -> Self {
        Self {
                duration,
                ..Default::default()
            }
        }

    /// Disable timeout
    pub fn disabled() -> Self {
        Self {
                duration: Duration::ZERO,
                enabled: false,
                grace_period: Duration::ZERO,
            }
        }

    /// Set grace period
    pub fn with_grace_period(mut self, grace_period: Duration) -> Self {
        self.grace_period = grace_period;
        self
    }
}

/// Timeout guard for async operations
pub struct TimeoutGuard {
    config: TimeoutConfig,
    started: AtomicBool,
    deadline: Pin<&'static tokio::time::Instant>,
}

impl TimeoutGuard {
    /// Create a new timeout guard
    pub fn new(config: TimeoutConfig) -> Self {
        Self {
            config,
                started: AtomicBool::new(false),
                deadline: Pin::new(tokio::time::Instant::now()),
            }
        }

    /// Start the timeout
    pub fn start(&self) {
        self.started.store(true, Ordering::SeqCst);
        let duration = self.config.duration;
        self.deadline.set(tokio::time::Instant::now() + duration);
    }

    /// Check if timed out
    pub fn is_timed_out(&self) -> bool {
        if !self.started.load(Ordering::SeqCst) {
            return false;
        }
        tokio::time::Instant::now() > *self.deadline.get()
    }

    /// Get remaining time
    pub fn remaining(&self) -> Option<Duration> {
        let deadline = *self.deadline.get();
        let now = tokio::time::Instant::now();
        if now >= deadline {
            None
        } else {
            Some(deadline - now)
        }
    }

    /// Reset the timeout
    pub fn reset(&self) {
        self.started.store(false, Ordering::SeqCst);
        self.deadline.set(tokio::time::Instant::now() + self.config.duration);
    }
}

/// Execute an    /// Run a future with timeout
    pub async fn run<F, T>(
        future: F,
        config: TimeoutConfig,
    ) -> Result<T>
    where
        F: Future<Output = T>,
    {
        if !config.enabled {
            return Ok(future.await);
        }

        match timeout(config.duration, future).await {
            Ok(result) => Ok(result),
            Err(_) => Err(anyhow!("Operation timed out")),
        }
    }

/// Execute
    /// Run a future with timeout,    pub async fn run_with_guard<F, T>(
        future: F,
        guard: &TimeoutGuard,
    ) -> Result<T>
    where
        F: Future<Output = T>,
    {
        if !guard.config.enabled {
            return Ok(future.await);
        }

        // Check timeout before starting
        if guard.is_timed_out() {
            return Err(anyhow!("Operation timed out before starting"));
        }

        match timeout(guard.config.duration, future).await {
            Ok(result) => {
                guard.reset();
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }

/// Execute with retry on timeout
    pub async fn run_with_retry<F, T, Fut>(
        future: F,
        config: TimeoutConfig,
        max_retries: u32,
    ) -> Result<T>
    where
        F: Future<Output = T> + Clone + Unpin,
    {
        let mut attempts = 0;

        loop {
            attempts += 1;

            match run(future.clone(), config.clone()).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if attempts >= max_retries {
                    return Err(anyhow!(
                        "Operation failed after {} attempts: {}",
                        attempts, e
                    ));
                }
                // Exponential backoff
                let delay = Duration::from_millis(100 * 2u64.pow(attempts as u32 - 1));
                sleep(delay).await;
            }
        }

        Err(anyhow!("Max retries exceeded"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_config_default() {
        let config = TimeoutConfig::default();
        assert!(config.enabled);
        assert_eq!(config.duration, Duration::from_secs(30));
    }

    #[test]
    fn test_timeout_config_disabled() {
        let config = TimeoutConfig::disabled();
        assert!(!config.enabled);
    }

    #[test]
    fn test_timeout_guard() {
        let config = TimeoutConfig::new(Duration::from_millis(100));
        let guard = TimeoutGuard::new(config);

        assert!(!guard.is_timed_out());
        guard.start();

        // Small delay should not timeout
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(!guard.is_timed_out());
    }

    #[tokio::test]
    async fn test_timeout_guard_expires() {
        let config = TimeoutConfig::new(Duration::from_millis(50));
        let guard = TimeoutGuard::new(config);
        guard.start();

        // Wait longer than timeout
        sleep(Duration::from_millis(100)).await;

        assert!(guard.is_timed_out());
    }
}
