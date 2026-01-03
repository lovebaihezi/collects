//! Collect Files module.
//!
//! This module provides the integration between Collects and file storage,
//! allowing users to upload images and videos to collections.
//!
//! # Architecture
//!
//! The module follows the repository pattern with trait-based abstraction:
//! - `CollectFileStorage` trait: Generic interface for collect file operations
//! - `MockCollectFileStorage`: In-memory implementation for testing
//!
//! # Features
//!
//! - Upload multiple files to a collection with descriptions
//! - Associate files with specific collects
//! - Support for images and videos
//! - Draft and published states for files
//!
//! # Example
//!
//! ```rust,ignore
//! use collects_services::collect_files::{CollectFileStorage, MockCollectFileStorage, CollectFileUpload};
//!
//! async fn upload_files<S: CollectFileStorage>(storage: &S) {
//!     let upload = CollectFileUpload::new(
//!         "collect-123",
//!         "images/photo.jpg",
//!         vec![/* file bytes */],
//!         "image/jpeg",
//!     ).with_description("A vacation photo");
//!
//!     let file = storage.upload_file(upload).await.unwrap();
//!     println!("Uploaded file: {} to collect: {}", file.id, file.collect_id);
//! }
//! ```

use std::future::Future;

// ============================================================================
// Types
// ============================================================================

/// Status of a collect file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectFileStatus {
    /// File is in draft state (not yet published)
    #[default]
    Draft,
    /// File is published and visible
    Published,
    /// File is archived (soft deleted)
    Archived,
}

/// A file associated with a collect
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectFile {
    /// Unique identifier for this file
    pub id: String,
    /// ID of the collect this file belongs to
    pub collect_id: String,
    /// Storage path/key for the file
    pub storage_path: String,
    /// Original filename
    pub filename: String,
    /// MIME type of the file
    pub content_type: String,
    /// Size in bytes
    pub size: u64,
    /// Optional description
    pub description: Option<String>,
    /// File status (draft, published, archived)
    pub status: CollectFileStatus,
}

impl CollectFile {
    /// Creates a new CollectFile
    pub fn new(
        id: impl Into<String>,
        collect_id: impl Into<String>,
        storage_path: impl Into<String>,
        filename: impl Into<String>,
        content_type: impl Into<String>,
        size: u64,
    ) -> Self {
        Self {
            id: id.into(),
            collect_id: collect_id.into(),
            storage_path: storage_path.into(),
            filename: filename.into(),
            content_type: content_type.into(),
            size,
            description: None,
            status: CollectFileStatus::Draft,
        }
    }

    /// Sets the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the status
    pub fn with_status(mut self, status: CollectFileStatus) -> Self {
        self.status = status;
        self
    }
}

/// Request to upload a file to a collect
#[derive(Debug, Clone)]
pub struct CollectFileUpload {
    /// ID of the collect to upload to
    pub collect_id: String,
    /// Desired storage path/key
    pub path: String,
    /// File content
    pub content: Vec<u8>,
    /// MIME type
    pub content_type: String,
    /// Optional description
    pub description: Option<String>,
}

impl CollectFileUpload {
    /// Creates a new file upload request
    pub fn new(
        collect_id: impl Into<String>,
        path: impl Into<String>,
        content: Vec<u8>,
        content_type: impl Into<String>,
    ) -> Self {
        Self {
            collect_id: collect_id.into(),
            path: path.into(),
            content,
            content_type: content_type.into(),
            description: None,
        }
    }

    /// Adds a description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Result of a batch upload operation
#[derive(Debug, Clone)]
pub struct BatchUploadResult {
    /// Successfully uploaded files
    pub successful: Vec<CollectFile>,
    /// Failed uploads with error messages
    pub failed: Vec<(String, String)>, // (path, error_message)
}

impl BatchUploadResult {
    /// Creates a new empty batch result
    pub fn new() -> Self {
        Self {
            successful: Vec::new(),
            failed: Vec::new(),
        }
    }

    /// Returns true if all uploads succeeded
    pub fn all_succeeded(&self) -> bool {
        self.failed.is_empty()
    }

    /// Returns the total number of files processed
    pub fn total_processed(&self) -> usize {
        self.successful.len() + self.failed.len()
    }
}

impl Default for BatchUploadResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for collect file operations
#[derive(Debug, thiserror::Error)]
pub enum CollectFileError {
    /// File not found
    #[error("File not found: {0}")]
    NotFound(String),

    /// Collect not found
    #[error("Collect not found: {0}")]
    CollectNotFound(String),

    /// Invalid file type
    #[error("Invalid file type: {0}")]
    InvalidFileType(String),

    /// File too large
    #[error("File too large: {size} bytes exceeds maximum {max_size} bytes")]
    FileTooLarge { size: u64, max_size: u64 },

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Database error
    #[error("Database error: {0}")]
    DatabaseError(String),
}

// ============================================================================
// CollectFileStorage Trait
// ============================================================================

/// Trait for collect file storage operations.
///
/// This trait combines file storage with collect associations, providing
/// a complete interface for managing files within collections.
///
/// # Type Parameters
///
/// * `Error` - The error type for storage operations
///
/// # Example
///
/// ```rust,ignore
/// use collects_services::collect_files::{CollectFileStorage, MockCollectFileStorage};
///
/// async fn list_collect_files<S: CollectFileStorage>(storage: &S, collect_id: &str) {
///     let files = storage.list_files_for_collect(collect_id).await.unwrap();
///     for file in files {
///         println!("{}: {}", file.filename, file.content_type);
///     }
/// }
/// ```
pub trait CollectFileStorage: Clone + Send + Sync + 'static {
    /// The error type for storage operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Uploads a file to a collect.
    ///
    /// # Arguments
    ///
    /// * `upload` - The file upload request
    ///
    /// # Returns
    ///
    /// Returns the created `CollectFile` on success.
    fn upload_file(
        &self,
        upload: CollectFileUpload,
    ) -> impl Future<Output = Result<CollectFile, Self::Error>> + Send;

    /// Uploads multiple files to a collect.
    ///
    /// # Arguments
    ///
    /// * `uploads` - A list of file upload requests
    ///
    /// # Returns
    ///
    /// Returns a `BatchUploadResult` containing successful and failed uploads.
    fn upload_files(
        &self,
        uploads: Vec<CollectFileUpload>,
    ) -> impl Future<Output = Result<BatchUploadResult, Self::Error>> + Send;

    /// Gets a file by ID.
    ///
    /// # Arguments
    ///
    /// * `file_id` - The unique file identifier
    ///
    /// # Returns
    ///
    /// Returns `Some(CollectFile)` if found, `None` otherwise.
    fn get_file(
        &self,
        file_id: &str,
    ) -> impl Future<Output = Result<Option<CollectFile>, Self::Error>> + Send;

    /// Lists all files for a collect.
    ///
    /// # Arguments
    ///
    /// * `collect_id` - The collect identifier
    ///
    /// # Returns
    ///
    /// Returns a list of files in the collect.
    fn list_files_for_collect(
        &self,
        collect_id: &str,
    ) -> impl Future<Output = Result<Vec<CollectFile>, Self::Error>> + Send;

    /// Deletes a file.
    ///
    /// # Arguments
    ///
    /// * `file_id` - The file identifier to delete
    ///
    /// # Returns
    ///
    /// Returns `true` if deleted, `false` if not found.
    fn delete_file(&self, file_id: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Updates the status of a file.
    ///
    /// # Arguments
    ///
    /// * `file_id` - The file identifier
    /// * `status` - The new status
    ///
    /// # Returns
    ///
    /// Returns the updated `CollectFile` on success.
    fn update_file_status(
        &self,
        file_id: &str,
        status: CollectFileStatus,
    ) -> impl Future<Output = Result<CollectFile, Self::Error>> + Send;

    /// Updates the description of a file.
    ///
    /// # Arguments
    ///
    /// * `file_id` - The file identifier
    /// * `description` - The new description (None to remove)
    ///
    /// # Returns
    ///
    /// Returns the updated `CollectFile` on success.
    fn update_file_description(
        &self,
        file_id: &str,
        description: Option<String>,
    ) -> impl Future<Output = Result<CollectFile, Self::Error>> + Send;

    /// Downloads file content.
    ///
    /// # Arguments
    ///
    /// * `file_id` - The file identifier
    ///
    /// # Returns
    ///
    /// Returns the file content as bytes.
    fn download_file(
        &self,
        file_id: &str,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send;
}

// ============================================================================
// MockCollectFileStorage Implementation
// ============================================================================

/// In-memory mock implementation of `CollectFileStorage` for testing.
#[derive(Clone, Default)]
pub struct MockCollectFileStorage {
    files: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, MockStoredFile>>>,
    next_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

/// Internal representation of a stored file
#[derive(Clone)]
struct MockStoredFile {
    metadata: CollectFile,
    content: Vec<u8>,
}

impl MockCollectFileStorage {
    /// Creates a new empty storage
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of files in storage
    pub fn len(&self) -> usize {
        self.files.read().expect("lock poisoned").len()
    }

    /// Returns true if storage is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all files
    pub fn clear(&self) {
        self.files.write().expect("lock poisoned").clear();
    }

    fn generate_id(&self) -> String {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        format!("file-{id}")
    }
}

impl CollectFileStorage for MockCollectFileStorage {
    type Error = CollectFileError;

    async fn upload_file(&self, upload: CollectFileUpload) -> Result<CollectFile, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");

        let id = self.generate_id();
        let filename = upload
            .path
            .split('/')
            .next_back()
            .unwrap_or(&upload.path)
            .to_string();

        let file = CollectFile {
            id: id.clone(),
            collect_id: upload.collect_id,
            storage_path: upload.path,
            filename,
            content_type: upload.content_type,
            size: upload.content.len() as u64,
            description: upload.description,
            status: CollectFileStatus::Draft,
        };

        let stored = MockStoredFile {
            metadata: file.clone(),
            content: upload.content,
        };

        files.insert(id, stored);

        Ok(file)
    }

    async fn upload_files(
        &self,
        uploads: Vec<CollectFileUpload>,
    ) -> Result<BatchUploadResult, Self::Error> {
        let mut result = BatchUploadResult::new();

        for upload in uploads {
            let path = upload.path.clone();
            match self.upload_file(upload).await {
                Ok(file) => result.successful.push(file),
                Err(e) => result.failed.push((path, e.to_string())),
            }
        }

        Ok(result)
    }

    async fn get_file(&self, file_id: &str) -> Result<Option<CollectFile>, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        Ok(files.get(file_id).map(|f| f.metadata.clone()))
    }

    async fn list_files_for_collect(
        &self,
        collect_id: &str,
    ) -> Result<Vec<CollectFile>, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        Ok(files
            .values()
            .filter(|f| f.metadata.collect_id == collect_id)
            .map(|f| f.metadata.clone())
            .collect())
    }

    async fn delete_file(&self, file_id: &str) -> Result<bool, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");
        Ok(files.remove(file_id).is_some())
    }

    async fn update_file_status(
        &self,
        file_id: &str,
        status: CollectFileStatus,
    ) -> Result<CollectFile, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");

        let stored = files
            .get_mut(file_id)
            .ok_or_else(|| CollectFileError::NotFound(file_id.to_string()))?;

        stored.metadata.status = status;

        Ok(stored.metadata.clone())
    }

    async fn update_file_description(
        &self,
        file_id: &str,
        description: Option<String>,
    ) -> Result<CollectFile, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");

        let stored = files
            .get_mut(file_id)
            .ok_or_else(|| CollectFileError::NotFound(file_id.to_string()))?;

        stored.metadata.description = description;

        Ok(stored.metadata.clone())
    }

    async fn download_file(&self, file_id: &str) -> Result<Vec<u8>, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        files
            .get(file_id)
            .map(|f| f.content.clone())
            .ok_or_else(|| CollectFileError::NotFound(file_id.to_string()))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_upload_single_file() {
        let storage = MockCollectFileStorage::new();

        let upload = CollectFileUpload::new(
            "collect-123",
            "images/photo.jpg",
            b"fake image data".to_vec(),
            "image/jpeg",
        )
        .with_description("A test photo");

        let file = storage
            .upload_file(upload)
            .await
            .expect("upload should succeed");

        assert_eq!(file.collect_id, "collect-123");
        assert_eq!(file.filename, "photo.jpg");
        assert_eq!(file.content_type, "image/jpeg");
        assert_eq!(file.description, Some("A test photo".to_string()));
        assert_eq!(file.status, CollectFileStatus::Draft);
    }

    #[tokio::test]
    async fn test_upload_multiple_files() {
        let storage = MockCollectFileStorage::new();

        let uploads = vec![
            CollectFileUpload::new(
                "collect-123",
                "images/photo1.jpg",
                b"photo1".to_vec(),
                "image/jpeg",
            ),
            CollectFileUpload::new(
                "collect-123",
                "images/photo2.jpg",
                b"photo2".to_vec(),
                "image/jpeg",
            ),
            CollectFileUpload::new(
                "collect-123",
                "videos/clip.mp4",
                b"video".to_vec(),
                "video/mp4",
            ),
        ];

        let result = storage
            .upload_files(uploads)
            .await
            .expect("batch upload should succeed");

        assert!(result.all_succeeded());
        assert_eq!(result.total_processed(), 3);
        assert_eq!(result.successful.len(), 3);
    }

    #[tokio::test]
    async fn test_list_files_for_collect() {
        let storage = MockCollectFileStorage::new();

        // Upload to different collects
        let uploads = vec![
            CollectFileUpload::new("collect-a", "a1.jpg", b"a1".to_vec(), "image/jpeg"),
            CollectFileUpload::new("collect-a", "a2.jpg", b"a2".to_vec(), "image/jpeg"),
            CollectFileUpload::new("collect-b", "b1.jpg", b"b1".to_vec(), "image/jpeg"),
        ];

        storage
            .upload_files(uploads)
            .await
            .expect("uploads should succeed");

        let files_a = storage
            .list_files_for_collect("collect-a")
            .await
            .expect("list should succeed");
        assert_eq!(files_a.len(), 2);

        let files_b = storage
            .list_files_for_collect("collect-b")
            .await
            .expect("list should succeed");
        assert_eq!(files_b.len(), 1);
    }

    #[tokio::test]
    async fn test_get_and_download_file() {
        let storage = MockCollectFileStorage::new();
        let content = b"test content".to_vec();

        let upload =
            CollectFileUpload::new("collect-123", "test.txt", content.clone(), "text/plain");

        let file = storage
            .upload_file(upload)
            .await
            .expect("upload should succeed");

        // Get metadata
        let retrieved = storage
            .get_file(&file.id)
            .await
            .expect("get should succeed")
            .expect("file should exist");
        assert_eq!(retrieved.id, file.id);

        // Download content
        let downloaded = storage
            .download_file(&file.id)
            .await
            .expect("download should succeed");
        assert_eq!(downloaded, content);
    }

    #[tokio::test]
    async fn test_delete_file() {
        let storage = MockCollectFileStorage::new();

        let upload =
            CollectFileUpload::new("collect-123", "test.txt", b"content".to_vec(), "text/plain");

        let file = storage
            .upload_file(upload)
            .await
            .expect("upload should succeed");

        assert!(!storage.is_empty());

        let deleted = storage
            .delete_file(&file.id)
            .await
            .expect("delete should succeed");
        assert!(deleted);

        assert!(storage.is_empty());

        // Delete non-existent
        let deleted = storage
            .delete_file("nonexistent")
            .await
            .expect("should not error");
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_update_file_status() {
        let storage = MockCollectFileStorage::new();

        let upload =
            CollectFileUpload::new("collect-123", "test.txt", b"content".to_vec(), "text/plain");

        let file = storage
            .upload_file(upload)
            .await
            .expect("upload should succeed");

        assert_eq!(file.status, CollectFileStatus::Draft);

        let updated = storage
            .update_file_status(&file.id, CollectFileStatus::Published)
            .await
            .expect("update should succeed");

        assert_eq!(updated.status, CollectFileStatus::Published);
    }

    #[tokio::test]
    async fn test_update_file_description() {
        let storage = MockCollectFileStorage::new();

        let upload =
            CollectFileUpload::new("collect-123", "test.txt", b"content".to_vec(), "text/plain");

        let file = storage
            .upload_file(upload)
            .await
            .expect("upload should succeed");

        assert!(file.description.is_none());

        let updated = storage
            .update_file_description(&file.id, Some("New description".to_string()))
            .await
            .expect("update should succeed");

        assert_eq!(updated.description, Some("New description".to_string()));

        // Remove description
        let updated = storage
            .update_file_description(&file.id, None)
            .await
            .expect("update should succeed");

        assert!(updated.description.is_none());
    }

    #[tokio::test]
    async fn test_get_nonexistent_file() {
        let storage = MockCollectFileStorage::new();

        let file = storage
            .get_file("nonexistent")
            .await
            .expect("should not error");

        assert!(file.is_none());
    }

    #[tokio::test]
    async fn test_download_nonexistent_file() {
        let storage = MockCollectFileStorage::new();

        let result = storage.download_file("nonexistent").await;

        assert!(matches!(result, Err(CollectFileError::NotFound(_))));
    }

    // Test generic trait usage
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

        let file = generic_upload(&storage, "test-collect")
            .await
            .expect("upload should succeed");

        assert_eq!(file.collect_id, "test-collect");
    }
}
