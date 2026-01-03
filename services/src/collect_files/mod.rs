//! Collect file storage module.
//!
//! Manages file uploads for collections. See `docs/collect-files.md` for details.

mod mock;
mod traits;
mod types;

pub use mock::MockCollectFileStorage;
pub use traits::CollectFileStorage;
pub use types::{
    BatchUploadResult, CollectFile, CollectFileError, CollectFileStatus, CollectFileUpload,
};

#[cfg(test)]
mod tests {
    use super::*;

    async fn generic_upload<S: CollectFileStorage>(
        storage: &S,
        collect_id: &str,
    ) -> Result<CollectFile, S::Error> {
        let upload = CollectFileUpload::new(
            collect_id,
            "test/file.bin",
            b"binary data".to_vec(),
            "application/octet-stream",
        );
        storage.upload_file(upload).await
    }

    #[tokio::test]
    async fn test_generic_trait_usage() {
        let storage = MockCollectFileStorage::new();
        let file = generic_upload(&storage, "test-collect").await.unwrap();
        assert_eq!(file.collect_id, "test-collect");
    }
}
