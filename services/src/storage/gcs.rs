//! Google Cloud Storage implementations.

use super::traits::OpenDALDisk;

/// Configuration for Google Cloud Storage.
#[derive(Clone)]
pub struct GDDiskConfig {
    pub bucket: String,
    pub credentials: String,
}

/// Google Cloud Storage connectivity checker.
#[derive(Clone)]
pub struct GDDisk {
    #[allow(dead_code)]
    config: Option<GDDiskConfig>,
}

impl GDDisk {
    pub fn new_for_test() -> Self {
        Self { config: None }
    }

    pub fn new(config: GDDiskConfig) -> Self {
        Self {
            config: Some(config),
        }
    }
}

impl Default for GDDisk {
    fn default() -> Self {
        Self::new_for_test()
    }
}

impl OpenDALDisk for GDDisk {
    async fn could_connected(&self) -> bool {
        let Some(config) = &self.config else {
            return true;
        };

        use opendal::Operator;

        let builder = opendal::services::Gcs::default()
            .bucket(&config.bucket)
            .credential(&config.credentials);

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
    async fn test_gddisk_could_connected_test_mode() {
        let disk = GDDisk::new_for_test();
        assert!(disk.could_connected().await);
    }
}
