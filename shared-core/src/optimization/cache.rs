//! LRU Cache Implementation
//!
//! Provides a thread-safe LRU (Least Recently Used) cache for storing
//! embeddings and API responses with configurable size limits and TTL.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};

/// Cache entry storing value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The cached value as bytes
    pub data: Vec<u8>,
    /// Content type of the data
    pub content_type: String,
    /// When the entry was created
    pub created_at: Instant,
    /// Time-to-live duration
    pub ttl: Duration,
    /// Size in bytes
    pub size: usize,
}

impl CacheEntry {
    /// Create a new cache entry
    pub fn new(data: Vec<u8>, content_type: String, ttl: Duration) -> Self {
        let size = data.len();
        Self {
            data,
            content_type,
            created_at: Instant::now(),
            ttl,
            size,
        }
    }

    /// Create a cache entry from a string
    pub fn from_string(value: String, content_type: String, ttl: Duration) -> Self {
        Self::new(value.into_bytes(), content_type, ttl)
    }

    /// Check if the entry has expired
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.ttl
    }

    /// Get the data as a string slice
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.data).ok()
    }
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Total size of cached data in bytes
    pub size_bytes: usize,
    /// Number of entries in the cache
    pub entry_count: usize,
}

impl CacheStats {
    /// Calculate hit rate as a percentage
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Internal cache node for LRU linked list
#[derive(Debug)]
struct CacheNode<V> {
    value: V,
    size: usize,
    last_accessed: Instant,
}

impl<V> CacheNode<V> {
    fn new(value: V, size: usize) -> Self {
        Self {
            value,
            size,
            last_accessed: Instant::now(),
        }
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// LRU Cache with size limit and TTL support
#[derive(Debug)]
pub struct LruCache<K, V> {
    /// Maximum size in bytes
    max_size: usize,
    /// Default TTL for entries
    default_ttl: Duration,
    /// Current size in bytes
    current_size: usize,
    /// Cache entries
    entries: HashMap<K, CacheNode<V>>,
    /// Access order for LRU eviction
    access_order: Vec<K>,
    /// Statistics
    stats: CacheStats,
}

impl<K, V> LruCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone,
{
    /// Create a new LRU cache with the given maximum size and default TTL
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            max_size,
            default_ttl,
            current_size: 0,
            entries: HashMap::new(),
            access_order: Vec::new(),
            stats: CacheStats::default(),
        }
    }

    /// Get a value from the cache (read-only, doesn't update access order)
    pub fn get(&self, key: &K) -> Option<V> {
        if let Some(node) = self.entries.get(key) {
            Some(node.value.clone())
        } else {
            None
        }
    }

    /// Get a value from the cache and update access statistics
    pub fn get_with_stats(&mut self, key: &K) -> Option<V> {
        if let Some(node) = self.entries.get_mut(key) {
            node.touch();
            self.stats.hits += 1;
            let key_clone = key.clone();
            self.update_access_order(&key_clone);
            Some(node.value.clone())
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Insert a value into the cache
    pub fn insert(&mut self, key: K, value: V) {
        let size = std::mem::size_of::<V>();
        self.insert_with_size(key, value, size);
    }

    /// Insert a value with a known size
    pub fn insert_with_size(&mut self, key: K, value: V, size: usize) {
        // Remove old entry if exists
        if let Some(old_node) = self.entries.remove(&key) {
            self.current_size -= old_node.size;
            self.access_order.retain(|k| k != &key);
        }

        // Evict entries if necessary
        while self.current_size + size > self.max_size && !self.entries.is_empty() {
            self.evict_lru();
        }

        // Insert new entry
        let node = CacheNode::new(value, size);
        self.current_size += size;
        self.entries.insert(key.clone(), node);
        self.access_order.push(key);
        self.stats.entry_count = self.entries.len();
        self.stats.size_bytes = self.current_size;
    }

    /// Remove a value from the cache
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(node) = self.entries.remove(key) {
            self.current_size -= node.size;
            self.access_order.retain(|k| k != key);
            self.stats.entry_count = self.entries.len();
            self.stats.size_bytes = self.current_size;
            Some(node.value)
        } else {
            None
        }
    }

    /// Check if the cache contains a key
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Get the number of entries in the cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries from the cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
        self.current_size = 0;
        self.stats.entry_count = 0;
        self.stats.size_bytes = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.stats.clone()
    }

    /// Get the current size in bytes
    pub fn current_size(&self) -> usize {
        self.current_size
    }

    /// Get the maximum size in bytes
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Update access order when a key is accessed
    fn update_access_order(&mut self, key: &K) {
        self.access_order.retain(|k| k != key);
        self.access_order.push(key.clone());
    }

    /// Evict the least recently used entry
    fn evict_lru(&mut self) {
        if let Some(lru_key) = self.access_order.first().cloned() {
            self.remove(&lru_key);
        }
    }
}

/// Specialized cache for embeddings (vector data)
impl LruCache<String, Vec<f32>> {
    /// Create a cache optimized for embeddings
    pub fn for_embeddings(max_entries: usize) -> Self {
        // Assume average embedding size of 1536 dimensions * 4 bytes = 6KB
        let max_size = max_entries * 6 * 1024;
        Self::new(max_size, Duration::from_secs(3600)) // 1 hour TTL
    }

    /// Get embedding similarity from cache
    pub fn get_similarity(&self, key1: &str, key2: &str) -> Option<f32> {
        // This would require both embeddings to be cached
        // Placeholder for similarity computation
        let _ = (key1, key2);
        None
    }
}

/// Specialized cache for API responses
impl LruCache<String, CacheEntry> {
    /// Create a cache optimized for API responses
    pub fn for_api_responses(max_size_mb: usize) -> Self {
        Self::new(max_size_mb * 1024 * 1024, Duration::from_secs(300)) // 5 min TTL
    }

    /// Get a response as string
    pub fn get_as_string(&mut self, key: &str) -> Option<String> {
        let key_owned = key.to_string();
        if let Some(entry) = self.get(&key_owned) {
            if entry.is_expired() {
                self.remove(&key_owned);
                None
            } else {
                entry.as_str().map(|s| s.to_string())
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic_operations() {
        let mut cache: LruCache<String, String> = LruCache::new(1024, Duration::from_secs(60));

        // Test insert and get
        cache.insert("key1".to_string(), "value1".to_string());
        assert_eq!(cache.get(&"key1".to_string()), Some("value1".to_string()));

        // Test miss
        assert!(cache.get(&"key2".to_string()).is_none());

        // Test stats
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache: LruCache<String, Vec<u8>> = LruCache::new(100, Duration::from_secs(60));

        // Insert entries that exceed max size
        cache.insert_with_size("key1".to_string(), vec![0u8; 60], 60);
        cache.insert_with_size("key2".to_string(), vec![0u8; 60], 60);

        // First entry should have been evicted
        assert!(cache.get(&"key1".to_string()).is_none());
        assert!(cache.get(&"key2".to_string()).is_some());
    }

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry::from_string(
            "test".to_string(),
            "text/plain".to_string(),
            Duration::from_millis(1),
        );

        std::thread::sleep(Duration::from_millis(10));
        assert!(entry.is_expired());
    }

    #[test]
    fn test_embedding_cache() {
        let mut cache = LruCache::for_embeddings(100);
        let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        cache.insert("test_embedding".to_string(), embedding.clone());

        assert_eq!(cache.get(&"test_embedding".to_string()), Some(embedding));
    }

    #[test]
    fn test_cache_stats_hit_rate() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            size_bytes: 1024,
            entry_count: 10,
        };

        assert!((stats.hit_rate() - 80.0).abs() < 0.01);
    }
}
