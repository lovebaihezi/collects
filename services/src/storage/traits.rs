//! Storage trait definitions.

use super::types::{FileMetadata, FileUploadRequest};
use std::future::Future;

/// Generic interface for file storage operations.
///
/// See [module documentation](super) for usage examples.
pub trait FileStorage: Clone + Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    fn upload_file(
        &self,
        request: FileUploadRequest,
    ) -> impl Future<Output = Result<FileMetadata, Self::Error>> + Send;

    fn download_file(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send;

    fn delete_file(&self, path: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    fn list_files(
        &self,
        prefix: &str,
    ) -> impl Future<Output = Result<Vec<FileMetadata>, Self::Error>> + Send;

    fn file_exists(&self, path: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    fn get_file_metadata(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Option<FileMetadata>, Self::Error>> + Send;
}

/// Trait for OpenDAL-based connectivity checks.
pub trait OpenDALDisk: Clone + Send + Sync + 'static {
    fn could_connected(&self) -> impl Future<Output = bool> + Send;
}
