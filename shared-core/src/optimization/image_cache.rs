//! Image Cache Module
//!
//! Provides specialized caching for image data with memory management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use std::time::{Duration, Instant};

/// Image cache entry
#[derive(Debug, Clone)]
pub struct ImageCacheEntry {
    /// Image data
    pub data: Vec<u8>,
    /// Image width
    pub width: u32,
    /// Image height
    pub height: u32,
    /// Image format (PNG, JPEG, etc.)
    pub format: String,
    /// Creation timestamp
    pub created_at: Instant,
    /// Last access timestamp
    pub last_accessed: Instant,
    /// Access count
    pub access_count: u64,
}

impl ImageCacheEntry {
    /// Get size in bytes
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }

    /// Check if entry is expired
    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.last_accessed.elapsed() > ttl
    }
}

/// Image cache statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageCacheStats {
    pub total_entries: usize,
    pub total_size_bytes: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl ImageCacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        self.hits as f64 / total as f64
    }
}

/// Image cache with LRU eviction
pub struct ImageCache {
    entries: Arc<RwLock<HashMap<String, ImageCacheEntry>>>,
    max_size: usize,
    ttl: Duration,
    stats: Arc<RwLock<ImageCacheStats>>,
}

impl ImageCache {
    /// Create a new image cache
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            ttl,
            stats: Arc::new(RwLock::new(ImageCacheStats::default())),
        }
    }

    /// Get image from cache
    pub fn get(&self, key: &str) -> Option<ImageCacheEntry> {
        let mut entries = self.entries.write();
        let mut stats = self.stats.write();

        if let Some(entry) = entries.get_mut(key) {
            entry.last_accessed = Instant::now();
            entry.access_count += 1;
            stats.hits += 1;
            Some(entry.clone())
        } else {
            stats.misses += 1;
            None
        }
    }

    /// Store image in cache
    pub fn put(&self, key: String, entry: ImageCacheEntry) {
        let mut entries = self.entries.write();
        let mut stats = self.stats.write();

        let entry_size = entry.size_bytes();

        // Check if we need to evict entries
        let current_size: usize = entries.values().map(|e| e.size_bytes()).sum();
        if current_size + entry_size > self.max_size {
            self.evict_lru(&mut entries, &mut stats, entry_size);
        }

        entries.insert(key, entry);
        stats.total_entries = entries.len();
        stats.total_size_bytes = entries.values().map(|e| e.size_bytes()).sum();
    }

    /// Remove image from cache
    pub fn remove(&self, key: &str) -> Option<ImageCacheEntry> {
        let mut entries = self.entries.write();
        let mut stats = self.stats.write();

        let removed = entries.remove(key);
        if removed.is_some() {
            stats.total_entries = entries.len();
            stats.total_size_bytes = entries.values().map(|e| e.size_bytes()).sum();
        }
        removed
    }

    /// Clear expired entries
    pub fn clear_expired(&self) -> usize {
        let mut entries = self.entries.write();
        let mut stats = self.stats.write();

        let before = entries.len();
        entries.retain(|_, entry| !entry.is_expired(self.ttl));
        let removed = before - entries.len();

        stats.evictions += removed as u64;
        stats.total_entries = entries.len();
        stats.total_size_bytes = entries.values().map(|e| e.size_bytes()).sum();

        removed
    }

    /// Clear all entries
    pub fn clear(&self) {
        let mut entries = self.entries.write();
        let mut stats = self.stats.write();

        let removed = entries.len();
        entries.clear();

        stats.evictions += removed as u64;
        stats.total_entries = 0;
        stats.total_size_bytes = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> ImageCacheStats {
        self.stats.read().clone()
    }

    /// Evict LRU entries to make room
    fn evict_lru(
        &self,
        entries: &mut HashMap<String, ImageCacheEntry>,
        stats: &mut ImageCacheStats,
        required_space: usize,
    ) {
        // Sort by last accessed time
        let mut access_times: Vec<(String, Instant)> = entries
            .iter()
            .map(|(k, e)| (k.clone(), e.last_accessed))
            .collect();
        access_times.sort_by_key(|(_, t)| *t);

        let mut freed_space = 0;
        for (key, _) in access_times {
            if freed_space >= required_space {
                break;
            }

            if let Some(entry) = entries.remove(&key) {
                freed_space += entry.size_bytes();
                stats.evictions += 1;
            }
        }
    }
}

/// Prefetch queue for loading images in advance
pub struct ImagePrefetcher {
    queue: Arc<RwLock<Vec<String>>>,
    cache: Arc<ImageCache>,
}

impl ImagePrefetcher {
    /// Create a new prefetcher
    pub fn new(cache: Arc<ImageCache>) -> Self {
        Self {
            queue: Arc::new(RwLock::new(Vec::new())),
            cache,
        }
    }

    /// Add keys to prefetch queue
    pub fn queue(&self, keys: Vec<String>) {
        let mut queue = self.queue.write();
        queue.extend(keys);
    }

    /// Get next key to prefetch
    pub fn next(&self) -> Option<String> {
        let mut queue = self.queue.write();
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }

    /// Check if key is already cached
    pub fn is_cached(&self, key: &str) -> bool {
        self.cache.get(key).is_some()
    }

    /// Clear prefetch queue
    pub fn clear(&self) {
        let mut queue = self.queue.write();
        queue.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_cache_creation() {
        let cache = ImageCache::new(1024 * 1024, Duration::from_secs(60));
        let stats = cache.stats();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.total_size_bytes, 0);
    }

    #[test]
    fn test_image_cache_put_get() {
        let cache = ImageCache::new(1024 * 1024, Duration::from_secs(60));

        let entry = ImageCacheEntry {
            data: vec![0u8; 100],
            width: 10,
            height: 10,
            format: "PNG".to_string(),
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 0,
        };

        cache.put("test".to_string(), entry);

        let stats = cache.stats();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.total_size_bytes, 100);

        let retrieved = cache.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().access_count, 1);
    }

    #[test]
    fn test_image_cache_miss() {
        let cache = ImageCache::new(1024 * 1024, Duration::from_secs(60));

        let result = cache.get("nonexistent");
        assert!(result.is_none());

        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);
    }

    #[test]
    fn test_image_cache_eviction() {
        let cache = ImageCache::new(100, Duration::from_secs(60)); // Very small cache

        let entry1 = ImageCacheEntry {
            data: vec![0u8; 60],
            width: 10,
            height: 6,
            format: "PNG".to_string(),
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 0,
        };

        let entry2 = ImageCacheEntry {
            data: vec![0u8; 60],
            width: 10,
            height: 6,
            format: "PNG".to_string(),
            created_at: Instant::now(),
            last_accessed: Instant::now(),
            access_count: 0,
        };

        cache.put("first".to_string(), entry1);
        cache.put("second".to_string(), entry2); // Should evict first

        let stats = cache.stats();
        assert!(stats.evictions >= 1);
    }
}
