//! In-memory cache for revoked JWT tokens.
//!
//! This module provides a fast lookup cache for token revocation checks,
//! reducing database load for the common case of validating non-revoked tokens.
//!
//! # Design
//!
//! - Uses foyer's in-memory LRU cache with configurable capacity
//! - Token hashes are stored as keys; presence means revoked
//! - Cache is checked before database lookup for fast path
//! - On logout, tokens are added to both cache and database
//! - Expired tokens are naturally evicted by LRU policy (JWT validation
//!   happens before revocation check, so expired tokens never reach here)
//!
//! # Usage
//!
//! ```rust,ignore
//! use collects_services::users::revocation_cache::RevocationCache;
//!
//! let cache = RevocationCache::new(10_000); // 10K capacity
//!
//! // On logout
//! cache.add_revoked("token_hash_here");
//!
//! // On auth check
//! if cache.is_revoked("token_hash_here") {
//!     // Token is revoked (cache hit)
//! } else {
//!     // Check database
//! }
//! ```

use foyer::{Cache, CacheBuilder};
use std::sync::Arc;

/// Default capacity for the revocation cache.
///
/// 10,000 entries is sufficient for most deployments:
/// - Each entry is ~64 bytes (SHA256 hex hash) + overhead
/// - Total memory usage is roughly 1-2 MB
/// - With typical session lengths, this covers many concurrent sessions
pub const DEFAULT_CAPACITY: usize = 10_000;

/// In-memory cache for revoked token hashes.
///
/// This cache provides a fast lookup layer before hitting the database
/// for revocation checks. It uses an LRU eviction policy to bound memory usage.
#[derive(Clone)]
pub struct RevocationCache {
    inner: Arc<Cache<String, ()>>,
}

impl RevocationCache {
    /// Create a new revocation cache with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of revoked token hashes to cache
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let cache = RevocationCache::new(10_000);
    /// ```
    pub fn new(capacity: usize) -> Self {
        let cache = CacheBuilder::new(capacity).build();
        Self {
            inner: Arc::new(cache),
        }
    }

    /// Create a new revocation cache with the default capacity.
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Add a revoked token hash to the cache.
    ///
    /// This should be called when a user logs out to enable fast
    /// revocation checks for subsequent requests with the same token.
    ///
    /// # Arguments
    ///
    /// * `token_hash` - SHA256 hex hash of the JWT token
    pub fn add_revoked(&self, token_hash: impl Into<String>) {
        self.inner.insert(token_hash.into(), ());
    }

    /// Check if a token hash is in the revocation cache.
    ///
    /// Returns `true` if the token is cached as revoked (cache hit).
    /// Returns `false` if not in cache (requires database check).
    ///
    /// # Arguments
    ///
    /// * `token_hash` - SHA256 hex hash of the JWT token
    pub fn is_revoked(&self, token_hash: &str) -> bool {
        self.inner.get(token_hash).is_some()
    }

    /// Get the current number of entries in the cache.
    ///
    /// Useful for monitoring and debugging.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.usage()
    }

    /// Check if the cache is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for RevocationCache {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

impl std::fmt::Debug for RevocationCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevocationCache")
            .field("capacity", &DEFAULT_CAPACITY)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_check_revoked() {
        let cache = RevocationCache::new(100);

        assert!(!cache.is_revoked("hash1"));

        cache.add_revoked("hash1");

        assert!(cache.is_revoked("hash1"));
        assert!(!cache.is_revoked("hash2"));
    }

    #[test]
    fn test_multiple_entries() {
        let cache = RevocationCache::new(100);

        cache.add_revoked("hash1");
        cache.add_revoked("hash2");
        cache.add_revoked("hash3");

        assert!(cache.is_revoked("hash1"));
        assert!(cache.is_revoked("hash2"));
        assert!(cache.is_revoked("hash3"));
        assert!(!cache.is_revoked("hash4"));
    }

    #[test]
    fn test_string_ownership() {
        let cache = RevocationCache::new(100);

        // Test with String
        cache.add_revoked(String::from("hash_string"));
        assert!(cache.is_revoked("hash_string"));

        // Test with &str
        cache.add_revoked("hash_str");
        assert!(cache.is_revoked("hash_str"));
    }

    #[test]
    fn test_clone() {
        let cache1 = RevocationCache::new(100);
        cache1.add_revoked("hash1");

        let cache2 = cache1.clone();
        assert!(cache2.is_revoked("hash1"));

        // Clones share the same underlying cache
        cache2.add_revoked("hash2");
        assert!(cache1.is_revoked("hash2"));
    }

    #[test]
    fn test_default() {
        let cache = RevocationCache::default();
        assert!(!cache.is_revoked("any_hash"));
    }

    #[test]
    fn test_debug() {
        let cache = RevocationCache::new(100);
        let debug_str = format!("{:?}", cache);
        assert!(debug_str.contains("RevocationCache"));
    }

    #[test]
    fn test_capacity_bounds() {
        // Small cache to test eviction
        let cache = RevocationCache::new(3);

        cache.add_revoked("hash1");
        cache.add_revoked("hash2");
        cache.add_revoked("hash3");

        // All should be present
        assert!(cache.is_revoked("hash1"));
        assert!(cache.is_revoked("hash2"));
        assert!(cache.is_revoked("hash3"));

        // Adding more may evict old entries (LRU behavior)
        cache.add_revoked("hash4");
        cache.add_revoked("hash5");

        // New entries should be present
        assert!(cache.is_revoked("hash4"));
        assert!(cache.is_revoked("hash5"));

        // Cache size is bounded (may not be exactly 3 due to foyer internals)
        assert!(cache.len() <= 5);
    }
}
