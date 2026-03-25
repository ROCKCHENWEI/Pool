//! Retry Mechanisms Module
//!
//! Provides configurable retry strategies for transient failures.

use crate::error::PoolError;
use anyhow::Result;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

/// Retry strategy configuration
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Multiplier for exponential backoff
    pub multiplier: f64,
    /// Whether to add jitter to delays
    pub jitter: bool,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryStrategy {
    /// Create a new retry strategy
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Set initial delay
    pub fn with_initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set maximum delay
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set multiplier
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Enable or disable jitter
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Calculate delay for a given attempt
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        let base_delay = self.initial_delay.as_secs_f64() * self.multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay.as_secs_f64());

        if self.jitter {
            // Add up to 25% jitter
            let jitter_range = capped_delay * 0.25;
            let jitter = (rand() % 1000) as f64 / 1000.0 * jitter_range;
            Duration::from_secs_f64(capped_delay + jitter)
        } else {
            Duration::from_secs_f64(capped_delay)
        }
    }

    /// Execute a function with retry logic
    pub async fn execute<F, Fut, T, E>(&self, mut operation: F) -> std::result::Result<T, E>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Debug + IsRetryable,
    {
        let mut last_error: Option<E> = None;
        let mut attempt = 0;

        loop {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    if !error.is_retryable() || attempt >= self.max_retries {
                        return Err(error);
                    }
                    last_error = Some(error);
                    let delay = self.calculate_delay(attempt);
                    sleep(delay).await;
                    attempt += 1;
                }
            }
        }
    }
}

/// Simple pseudo-random number generator for jitter
fn rand() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEED: AtomicU64 = AtomicU64::new(12345);
    let mut seed = SEED.load(Ordering::Relaxed);
    seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
    SEED.store(seed, Ordering::Relaxed);
    seed
}

/// Trait for determining if an error is retryable
pub trait IsRetryable {
    /// Check if the error is retryable
    fn is_retryable(&self) -> bool;
}

impl IsRetryable for PoolError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            PoolError::ConnectionError { .. }
                | PoolError::TimeoutError { .. }
                | PoolError::RateLimitError { .. }
        )
    }
}

impl IsRetryable for anyhow::Error {
    fn is_retryable(&self) -> bool {
        // Check if the error message contains retryable keywords
        let msg = self.to_string().to_lowercase();
        msg.contains("timeout")
            || msg.contains("connection")
            || msg.contains("rate limit")
            || msg.contains("temporarily unavailable")
    }
}

impl IsRetryable for String {
    fn is_retryable(&self) -> bool {
        true // Strings are always retryable as generic errors
    }
}

/// Retry policy presets
pub struct RetryPolicy;

impl RetryPolicy {
    /// Aggressive retry policy (many retries, short delays)
    pub fn aggressive() -> RetryStrategy {
        RetryStrategy {
            max_retries: 10,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            multiplier: 1.5,
            jitter: true,
        }
    }

    /// Standard retry policy
    pub fn standard() -> RetryStrategy {
        RetryStrategy::default()
    }

    /// Conservative retry policy (few retries, longer delays)
    pub fn conservative() -> RetryStrategy {
        RetryStrategy {
            max_retries: 2,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            multiplier: 3.0,
            jitter: true,
        }
    }

    /// No retry policy
    pub fn none() -> RetryStrategy {
        RetryStrategy {
            max_retries: 0,
            initial_delay: Duration::from_secs(0),
            max_delay: Duration::from_secs(0),
            multiplier: 1.0,
            jitter: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_strategy_default() {
        let strategy = RetryStrategy::default();
        assert_eq!(strategy.max_retries, 3);
        assert!(strategy.jitter);
    }

    #[test]
    fn test_retry_strategy_delay_calculation() {
        let strategy = RetryStrategy::new(5)
            .with_jitter(false)
            .with_multiplier(2.0);

        assert_eq!(strategy.calculate_delay(0), Duration::from_millis(100));
        assert_eq!(strategy.calculate_delay(1), Duration::from_millis(200));
        assert_eq!(strategy.calculate_delay(2), Duration::from_millis(400));
    }

    #[test]
    fn test_retry_strategy_max_delay_cap() {
        let strategy = RetryStrategy::new(10)
            .with_jitter(false)
            .with_max_delay(Duration::from_secs(1));

        // Even with high attempt, delay should be capped
        let delay = strategy.calculate_delay(20);
        assert!(delay <= Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_retry_success_on_second_attempt() {
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let strategy = RetryStrategy::new(3)
            .with_jitter(false);

        let result = strategy
            .execute(move || {
                let attempts = attempts_clone.clone();
                async move {
                    let count = attempts.fetch_add(1, Ordering::SeqCst);
                    if count < 1 {
                        Err("Temporary error")
                    } else {
                        Ok("Success")
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
