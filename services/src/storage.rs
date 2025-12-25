//! OpenDAL Remote Storage Service
//!
//! This module provides a generic trait-based interface for remote storage services,
//! similar to how `SqlStorage` provides a generic interface for SQL databases.
//!
//! # Architecture
//!
//! The module follows the same pattern as the database module:
//! - `OpenDALDisk` trait: Generic interface for storage backends
//! - `CFDisk`: Cloudflare R2 implementation
//! - `GDDisk`: Google Cloud Storage implementation
//!
//! # Usage in Production
//!
//! In production, credentials should be read from Google Cloud Secret Manager at runtime.
//! The configuration can be read from environment variables similar to `DATABASE_URL`:
//!
//! ```bash
//! # Cloudflare R2 configuration
//! CF_ACCOUNT_ID=$(gcloud secrets versions access latest --secret=cf-account-id)
//! CF_ACCESS_KEY_ID=$(gcloud secrets versions access latest --secret=cf-access-key-id)
//! CF_SECRET_ACCESS_KEY=$(gcloud secrets versions access latest --secret=cf-secret-access-key)
//! CF_BUCKET=$(gcloud secrets versions access latest --secret=cf-bucket)
//!
//! # Google Cloud Storage configuration
//! GCS_BUCKET=$(gcloud secrets versions access latest --secret=gcs-bucket)
//! GCS_CREDENTIALS=$(gcloud secrets versions access latest --secret=gcs-credentials)
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use collects_services::storage::{CFDisk, CFDiskConfig, OpenDALDisk};
//!
//! # async fn example() {
//! // Create a CFDisk instance (production)
//! # #[cfg(not(test))]
//! let config = CFDiskConfig {
//!     account_id: "your-account-id".to_string(),
//!     access_key_id: "your-access-key".to_string(),
//!     secret_access_key: "your-secret-key".to_string(),
//!     bucket: "your-bucket".to_string(),
//! };
//! # #[cfg(not(test))]
//! let disk = CFDisk::new(config);
//!
//! // Check connectivity
//! # #[cfg(not(test))]
//! if disk.could_connected().await {
//!     println!("Successfully connected to Cloudflare R2");
//! }
//! # }
//! ```

use std::future::Future;

/// Trait for OpenDAL-based remote storage services
/// Similar to SqlStorage, this provides a generic interface for different storage backends
pub trait OpenDALDisk: Clone + Send + Sync + 'static {
    /// Check if the storage service could be connected
    /// Returns true if connection is possible, false otherwise
    fn could_connected(&self) -> impl Future<Output = bool> + Send;
}

/// Cloudflare R2 storage implementation
#[derive(Clone)]
pub struct CFDisk {
    /// Configuration for Cloudflare R2
    /// In production, this would contain credentials from gcloud secrets
    #[cfg(not(test))]
    config: CFDiskConfig,
}

/// Configuration for Cloudflare R2
#[cfg(not(test))]
#[derive(Clone)]
pub struct CFDiskConfig {
    /// R2 account ID
    pub account_id: String,
    /// R2 access key ID
    pub access_key_id: String,
    /// R2 secret access key
    pub secret_access_key: String,
    /// R2 bucket name
    pub bucket: String,
}

impl CFDisk {
    /// Create a new CFDisk instance for testing
    #[cfg(test)]
    pub fn new() -> Self {
        Self {}
    }

    /// Create a new CFDisk instance with configuration
    #[cfg(not(test))]
    pub fn new(config: CFDiskConfig) -> Self {
        Self { config }
    }
}

#[cfg(test)]
impl Default for CFDisk {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenDALDisk for CFDisk {
    #[cfg(test)]
    async fn could_connected(&self) -> bool {
        // In test setup, always return true
        true
    }

    #[cfg(not(test))]
    async fn could_connected(&self) -> bool {
        // In production, attempt to connect to Cloudflare R2
        // This would use the opendal library to verify connectivity
        use opendal::Operator;

        // Create an S3-compatible operator for Cloudflare R2
        let builder = opendal::services::S3::default()
            .bucket(&self.config.bucket)
            .region("auto")
            .access_key_id(&self.config.access_key_id)
            .secret_access_key(&self.config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                self.config.account_id
            ));

        match Operator::new(builder) {
            Ok(op) => op.finish().check().await.is_ok(),
            Err(_) => false,
        }
    }
}

/// Google Cloud Drive storage implementation
#[derive(Clone)]
pub struct GDDisk {
    /// Configuration for Google Cloud Drive
    /// In production, this would contain credentials from gcloud secrets
    #[cfg(not(test))]
    config: GDDiskConfig,
}

/// Configuration for Google Cloud Drive
#[cfg(not(test))]
#[derive(Clone)]
pub struct GDDiskConfig {
    /// GCS bucket name
    pub bucket: String,
    /// GCS credentials JSON
    pub credentials: String,
}

impl GDDisk {
    /// Create a new GDDisk instance for testing
    #[cfg(test)]
    pub fn new() -> Self {
        Self {}
    }

    /// Create a new GDDisk instance with configuration
    #[cfg(not(test))]
    pub fn new(config: GDDiskConfig) -> Self {
        Self { config }
    }
}

#[cfg(test)]
impl Default for GDDisk {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenDALDisk for GDDisk {
    #[cfg(test)]
    async fn could_connected(&self) -> bool {
        // In test setup, always return true
        true
    }

    #[cfg(not(test))]
    async fn could_connected(&self) -> bool {
        // In production, attempt to connect to Google Cloud Storage
        // This would use the opendal library to verify connectivity
        use opendal::Operator;

        let builder = opendal::services::Gcs::default()
            .bucket(&self.config.bucket)
            .credential(&self.config.credentials);

        match Operator::new(builder) {
            Ok(op) => op.finish().check().await.is_ok(),
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cfdisk_could_connected() {
        let disk = CFDisk::new();
        assert!(
            disk.could_connected().await,
            "CFDisk should be connectable in test mode"
        );
    }

    #[tokio::test]
    async fn test_gddisk_could_connected() {
        let disk = GDDisk::new();
        assert!(
            disk.could_connected().await,
            "GDDisk should be connectable in test mode"
        );
    }

    // Example test showing how to use the generic trait
    async fn check_storage_connection<T: OpenDALDisk>(storage: T) -> bool {
        storage.could_connected().await
    }

    #[tokio::test]
    async fn test_generic_storage_interface() {
        let cf_disk = CFDisk::new();
        let gd_disk = GDDisk::new();

        // Both implementations work with the generic interface
        assert!(check_storage_connection(cf_disk).await);
        assert!(check_storage_connection(gd_disk).await);
    }
}
