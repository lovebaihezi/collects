//! Remote storage service using OpenDAL.
//!
//! Provides trait-based abstractions for file storage backends (Cloudflare R2, GCS).
//! See `docs/storage.md` for detailed documentation.

mod cloudflare;
mod gcs;
mod mock;
mod traits;
mod types;

pub use cloudflare::{CFDisk, CFDiskConfig, CFFileStorage};
pub use gcs::{GDDisk, GDDiskConfig};
pub use mock::MockFileStorage;
pub use traits::{FileStorage, OpenDALDisk};
pub use types::{FileMetadata, FileStorageError, FileUploadRequest};

#[cfg(test)]
mod tests {
    use super::*;

    async fn check_storage_connection<T: OpenDALDisk>(storage: T) -> bool {
        storage.could_connected().await
    }

    #[tokio::test]
    async fn test_generic_storage_interface() {
        let cf_disk = CFDisk::new_for_test();
        let gd_disk = GDDisk::new_for_test();

        assert!(check_storage_connection(cf_disk).await);
        assert!(check_storage_connection(gd_disk).await);
    }

    async fn generic_upload<S: FileStorage>(
        storage: &S,
        path: &str,
        content: Vec<u8>,
    ) -> Result<FileMetadata, S::Error> {
        let request = FileUploadRequest::new(path, content, "application/octet-stream");
        storage.upload_file(request).await
    }

    #[tokio::test]
    async fn test_generic_file_storage_trait() {
        let storage = MockFileStorage::new();
        let metadata = generic_upload(&storage, "test/file.bin", b"binary data".to_vec())
            .await
            .unwrap();
        assert_eq!(metadata.id, "test/file.bin");
    }
}
