//! Collect file storage trait.

use super::types::{BatchUploadResult, CollectFile, CollectFileStatus, CollectFileUpload};
use std::future::Future;

/// Trait for collect file storage operations.
///
/// See [module documentation](super) for usage examples.
pub trait CollectFileStorage: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn upload_file(
        &self,
        upload: CollectFileUpload,
    ) -> impl Future<Output = Result<CollectFile, Self::Error>> + Send;

    fn upload_files(
        &self,
        uploads: Vec<CollectFileUpload>,
    ) -> impl Future<Output = Result<BatchUploadResult, Self::Error>> + Send;

    fn get_file(
        &self,
        file_id: &str,
    ) -> impl Future<Output = Result<Option<CollectFile>, Self::Error>> + Send;

    fn list_files_for_collect(
        &self,
        collect_id: &str,
    ) -> impl Future<Output = Result<Vec<CollectFile>, Self::Error>> + Send;

    fn delete_file(&self, file_id: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    fn update_file_status(
        &self,
        file_id: &str,
        status: CollectFileStatus,
    ) -> impl Future<Output = Result<CollectFile, Self::Error>> + Send;

    fn update_file_description(
        &self,
        file_id: &str,
        description: Option<String>,
    ) -> impl Future<Output = Result<CollectFile, Self::Error>> + Send;

    fn download_file(
        &self,
        file_id: &str,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send;
}
