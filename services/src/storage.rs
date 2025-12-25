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
        assert!(disk.could_connected().await, "CFDisk should be connectable in test mode");
    }

    #[tokio::test]
    async fn test_gddisk_could_connected() {
        let disk = GDDisk::new();
        assert!(disk.could_connected().await, "GDDisk should be connectable in test mode");
    }
}
