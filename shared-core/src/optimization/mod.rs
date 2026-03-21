//! Optimization Module
//!
//! This module provides performance optimization utilities for the Pool project:
//! - LRU cache for embeddings and API responses
//! - Async executor for concurrent task scheduling

pub mod async_executor;
pub mod cache;

pub use async_executor::{AsyncExecutor, TaskPriority, TaskHandle};
pub use cache::{LruCache, CacheEntry, CacheStats};

use std::sync::Arc;
use std::time::Duration;
use parking_lot::RwLock;

/// Global optimization configuration
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Maximum cache size in bytes
    pub max_cache_size: usize,
    /// Default cache TTL
    pub cache_ttl: Duration,
    /// Maximum concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Task timeout duration
    pub task_timeout: Duration,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            max_cache_size: 100 * 1024 * 1024, // 100MB
            cache_ttl: Duration::from_secs(3600), // 1 hour
            max_concurrent_tasks: 10,
            task_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Optimization manager that coordinates caching and task execution
pub struct OptimizationManager {
    config: OptimizationConfig,
    embedding_cache: Arc<RwLock<LruCache<String, Vec<f32>>>>,
    response_cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    executor: Arc<AsyncExecutor>,
}

impl OptimizationManager {
    /// Create a new optimization manager with the given configuration
    pub fn new(config: OptimizationConfig) -> Self {
        let embedding_cache = Arc::new(RwLock::new(
            LruCache::new(config.max_cache_size / 2, config.cache_ttl)
        ));
        let response_cache = Arc::new(RwLock::new(
            LruCache::new(config.max_cache_size / 2, config.cache_ttl)
        ));
        let executor = Arc::new(AsyncExecutor::new(config.max_concurrent_tasks));

        Self {
            config,
            embedding_cache,
            response_cache,
            executor,
        }
    }

    /// Get cached embedding by key
    pub fn get_embedding(&self, key: &str) -> Option<Vec<f32>> {
        self.embedding_cache.write().get(&key.to_string())
    }

    /// Store embedding in cache
    pub fn store_embedding(&self, key: String, embedding: Vec<f32>) {
        self.embedding_cache.write().insert(key, embedding);
    }

    /// Get cached API response
    pub fn get_response(&self, key: &str) -> Option<CacheEntry> {
        self.response_cache.write().get(&key.to_string())
    }

    /// Store API response in cache
    pub fn store_response(&self, key: String, entry: CacheEntry) {
        self.response_cache.write().insert(key, entry);
    }

    /// Get the async executor
    pub fn executor(&self) -> Arc<AsyncExecutor> {
        self.executor.clone()
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> CacheStats {
        let embedding_stats = self.embedding_cache.read().stats();
        let response_stats = self.response_cache.read().stats();

        CacheStats {
            hits: embedding_stats.hits + response_stats.hits,
            misses: embedding_stats.misses + response_stats.misses,
            size_bytes: embedding_stats.size_bytes + response_stats.size_bytes,
            entry_count: embedding_stats.entry_count + response_stats.entry_count,
        }
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.embedding_cache.write().clear();
        self.response_cache.write().clear();
    }
}

impl Default for OptimizationManager {
    fn default() -> Self {
        Self::new(OptimizationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_optimization_manager_creation() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let manager = OptimizationManager::default();
            let stats = manager.cache_stats();
            assert_eq!(stats.hits, 0);
            assert_eq!(stats.misses, 0);
        });
    }

    #[test]
    fn test_embedding_cache() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let manager = OptimizationManager::default();

            // Test miss
            assert!(manager.get_embedding("test").is_none());

            // Test store and hit
            let embedding = vec![0.1, 0.2, 0.3];
            manager.store_embedding("test".to_string(), embedding.clone());
            assert_eq!(manager.get_embedding("test"), Some(embedding));
        });
    }
}
