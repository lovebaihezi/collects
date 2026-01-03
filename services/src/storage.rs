//! OpenDAL Remote Storage Service
//!
//! This module provides a generic trait-based interface for remote storage services,
//! similar to how `SqlStorage` provides a generic interface for SQL databases.
//!
//! # Architecture
//!
//! The module follows the same pattern as the database module:
//! - `OpenDALDisk` trait: Generic interface for storage backends (connectivity check)
//! - `FileStorage` trait: Generic interface for file operations (upload, download, list, delete)
//! - `CFDisk`/`CFFileStorage`: Cloudflare R2 implementation
//! - `GDDisk`/`GCSFileStorage`: Google Cloud Storage implementation
//! - `MockFileStorage`: In-memory implementation for testing
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

// ============================================================================
// File Storage Types
// ============================================================================

/// Metadata for an uploaded file
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    /// Unique identifier for the file (typically a UUID or path)
    pub id: String,
    /// Original filename provided by the user
    pub filename: String,
    /// MIME type of the file (e.g., "image/png", "video/mp4")
    pub content_type: String,
    /// Size of the file in bytes
    pub size: u64,
    /// Optional description provided by the user
    pub description: Option<String>,
}

impl FileMetadata {
    /// Creates new file metadata
    pub fn new(
        id: impl Into<String>,
        filename: impl Into<String>,
        content_type: impl Into<String>,
        size: u64,
    ) -> Self {
        Self {
            id: id.into(),
            filename: filename.into(),
            content_type: content_type.into(),
            size,
            description: None,
        }
    }

    /// Sets the description for this file metadata
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Request to upload a file
#[derive(Debug, Clone)]
pub struct FileUploadRequest {
    /// The path/key where the file should be stored
    pub path: String,
    /// The file content as bytes
    pub content: Vec<u8>,
    /// The MIME type of the file
    pub content_type: String,
    /// Optional description for the file
    pub description: Option<String>,
}

impl FileUploadRequest {
    /// Creates a new file upload request
    pub fn new(path: impl Into<String>, content: Vec<u8>, content_type: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content,
            content_type: content_type.into(),
            description: None,
        }
    }

    /// Adds a description to the upload request
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Error type for file storage operations
#[derive(Debug, thiserror::Error)]
pub enum FileStorageError {
    /// File not found
    #[error("File not found: {0}")]
    NotFound(String),

    /// File already exists
    #[error("File already exists: {0}")]
    AlreadyExists(String),

    /// Invalid file type
    #[error("Invalid file type: {0}")]
    InvalidFileType(String),

    /// File too large
    #[error("File too large: {size} bytes exceeds maximum {max_size} bytes")]
    FileTooLarge { size: u64, max_size: u64 },

    /// Storage backend error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

// ============================================================================
// FileStorage Trait
// ============================================================================

/// Trait for file storage operations.
///
/// This trait provides an abstraction over file storage backends (Cloudflare R2, GCS, etc.),
/// allowing for different implementations while maintaining a consistent interface.
///
/// # Example
///
/// ```rust,ignore
/// use collects_services::storage::{FileStorage, MockFileStorage, FileUploadRequest};
///
/// async fn upload_file<S: FileStorage>(storage: &S) {
///     let request = FileUploadRequest::new(
///         "images/photo.jpg",
///         vec![/* file bytes */],
///         "image/jpeg",
///     );
///     let metadata = storage.upload_file(request).await.unwrap();
///     println!("Uploaded file: {}", metadata.id);
/// }
/// ```
pub trait FileStorage: Clone + Send + Sync + 'static {
    /// The error type for storage operations
    type Error: std::error::Error + Send + Sync + 'static;

    /// Uploads a file to the storage backend.
    ///
    /// # Arguments
    ///
    /// * `request` - The file upload request containing path, content, and metadata
    ///
    /// # Returns
    ///
    /// Returns `FileMetadata` on success, or an error if the upload fails.
    fn upload_file(
        &self,
        request: FileUploadRequest,
    ) -> impl Future<Output = Result<FileMetadata, Self::Error>> + Send;

    /// Downloads a file from the storage backend.
    ///
    /// # Arguments
    ///
    /// * `path` - The path/key of the file to download
    ///
    /// # Returns
    ///
    /// Returns the file content as bytes on success, or an error if not found.
    fn download_file(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Vec<u8>, Self::Error>> + Send;

    /// Deletes a file from the storage backend.
    ///
    /// # Arguments
    ///
    /// * `path` - The path/key of the file to delete
    ///
    /// # Returns
    ///
    /// Returns `true` if the file was deleted, `false` if it didn't exist.
    fn delete_file(&self, path: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Lists files in a directory/prefix.
    ///
    /// # Arguments
    ///
    /// * `prefix` - The prefix/directory to list files from
    ///
    /// # Returns
    ///
    /// Returns a list of file metadata for files matching the prefix.
    fn list_files(
        &self,
        prefix: &str,
    ) -> impl Future<Output = Result<Vec<FileMetadata>, Self::Error>> + Send;

    /// Checks if a file exists.
    ///
    /// # Arguments
    ///
    /// * `path` - The path/key of the file to check
    ///
    /// # Returns
    ///
    /// Returns `true` if the file exists, `false` otherwise.
    fn file_exists(&self, path: &str) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Gets metadata for a file without downloading it.
    ///
    /// # Arguments
    ///
    /// * `path` - The path/key of the file
    ///
    /// # Returns
    ///
    /// Returns `Some(FileMetadata)` if the file exists, `None` otherwise.
    fn get_file_metadata(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Option<FileMetadata>, Self::Error>> + Send;
}

// ============================================================================
// MockFileStorage Implementation (for testing)
// ============================================================================

/// In-memory mock implementation of `FileStorage` for testing.
///
/// This implementation stores files in a thread-safe `HashMap` and is suitable
/// for unit tests and integration tests that don't require real cloud storage.
///
/// # Example
///
/// ```rust,ignore
/// use collects_services::storage::{MockFileStorage, FileStorage, FileUploadRequest};
///
/// #[tokio::test]
/// async fn test_file_upload() {
///     let storage = MockFileStorage::new();
///
///     let request = FileUploadRequest::new(
///         "test/file.txt",
///         b"Hello, World!".to_vec(),
///         "text/plain",
///     );
///     let metadata = storage.upload_file(request).await.unwrap();
///     assert_eq!(metadata.filename, "file.txt");
/// }
/// ```
#[derive(Clone, Default)]
pub struct MockFileStorage {
    files: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, MockFile>>>,
}

/// Internal representation of a file in MockFileStorage
#[derive(Clone)]
struct MockFile {
    content: Vec<u8>,
    metadata: FileMetadata,
}

impl MockFileStorage {
    /// Creates a new empty `MockFileStorage`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of files in storage.
    pub fn len(&self) -> usize {
        self.files.read().expect("lock poisoned").len()
    }

    /// Returns `true` if the storage is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all files from storage.
    pub fn clear(&self) {
        self.files.write().expect("lock poisoned").clear();
    }
}

impl FileStorage for MockFileStorage {
    type Error = FileStorageError;

    async fn upload_file(&self, request: FileUploadRequest) -> Result<FileMetadata, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");

        // Extract filename from path
        let filename = request
            .path
            .split('/')
            .next_back()
            .unwrap_or(&request.path)
            .to_string();

        let metadata = FileMetadata {
            id: request.path.clone(),
            filename,
            content_type: request.content_type,
            size: request.content.len() as u64,
            description: request.description,
        };

        let file = MockFile {
            content: request.content,
            metadata: metadata.clone(),
        };

        files.insert(request.path, file);

        Ok(metadata)
    }

    async fn download_file(&self, path: &str) -> Result<Vec<u8>, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        files
            .get(path)
            .map(|f| f.content.clone())
            .ok_or_else(|| FileStorageError::NotFound(path.to_string()))
    }

    async fn delete_file(&self, path: &str) -> Result<bool, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");
        Ok(files.remove(path).is_some())
    }

    async fn list_files(&self, prefix: &str) -> Result<Vec<FileMetadata>, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        Ok(files
            .iter()
            .filter(|(path, _)| path.starts_with(prefix))
            .map(|(_, file)| file.metadata.clone())
            .collect())
    }

    async fn file_exists(&self, path: &str) -> Result<bool, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        Ok(files.contains_key(path))
    }

    async fn get_file_metadata(&self, path: &str) -> Result<Option<FileMetadata>, Self::Error> {
        let files = self.files.read().expect("lock poisoned");
        Ok(files.get(path).map(|f| f.metadata.clone()))
    }
}

// ============================================================================
// OpenDALDisk Trait (for connectivity checks)
// ============================================================================

/// Trait for OpenDAL-based remote storage services
/// Similar to SqlStorage, this provides a generic interface for different storage backends
pub trait OpenDALDisk: Clone + Send + Sync + 'static {
    /// Check if the storage service could be connected
    /// Returns true if connection is possible, false otherwise
    fn could_connected(&self) -> impl Future<Output = bool> + Send;
}

// ============================================================================
// Cloudflare R2 Implementations
// ============================================================================

/// Configuration for Cloudflare R2
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

/// Cloudflare R2 storage implementation for connectivity checks
#[derive(Clone)]
pub struct CFDisk {
    /// Configuration for Cloudflare R2
    #[allow(dead_code)]
    config: Option<CFDiskConfig>,
}

impl CFDisk {
    /// Create a new CFDisk instance for testing (no config)
    pub fn new_for_test() -> Self {
        Self { config: None }
    }

    /// Create a new CFDisk instance with configuration
    pub fn new(config: CFDiskConfig) -> Self {
        Self {
            config: Some(config),
        }
    }
}

impl Default for CFDisk {
    fn default() -> Self {
        Self::new_for_test()
    }
}

impl OpenDALDisk for CFDisk {
    async fn could_connected(&self) -> bool {
        let Some(config) = &self.config else {
            // Test mode - always return true
            return true;
        };

        // In production, attempt to connect to Cloudflare R2
        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        match Operator::new(builder) {
            Ok(op) => op.finish().check().await.is_ok(),
            Err(_) => false,
        }
    }
}

/// Cloudflare R2 file storage implementation using OpenDAL
#[derive(Clone)]
pub struct CFFileStorage {
    /// Configuration for Cloudflare R2
    #[allow(dead_code)]
    config: Option<CFDiskConfig>,
    /// Mock storage for testing
    mock: Option<MockFileStorage>,
}

impl CFFileStorage {
    /// Create a new CFFileStorage instance for testing
    pub fn new_for_test() -> Self {
        Self {
            config: None,
            mock: Some(MockFileStorage::new()),
        }
    }

    /// Create a new CFFileStorage instance with configuration
    pub fn new(config: CFDiskConfig) -> Self {
        Self {
            config: Some(config),
            mock: None,
        }
    }
}

impl Default for CFFileStorage {
    fn default() -> Self {
        Self::new_for_test()
    }
}

impl FileStorage for CFFileStorage {
    type Error = FileStorageError;

    async fn upload_file(&self, request: FileUploadRequest) -> Result<FileMetadata, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.upload_file(request).await;
        }

        let Some(config) = &self.config else {
            return Err(FileStorageError::ConnectionError(
                "No configuration provided".to_string(),
            ));
        };

        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        let op = Operator::new(builder)
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?
            .finish();

        op.write(&request.path, request.content.clone())
            .await
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?;

        let filename = request
            .path
            .split('/')
            .next_back()
            .unwrap_or(&request.path)
            .to_string();

        Ok(FileMetadata {
            id: request.path,
            filename,
            content_type: request.content_type,
            size: request.content.len() as u64,
            description: request.description,
        })
    }

    async fn download_file(&self, path: &str) -> Result<Vec<u8>, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.download_file(path).await;
        }

        let Some(config) = &self.config else {
            return Err(FileStorageError::ConnectionError(
                "No configuration provided".to_string(),
            ));
        };

        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        let op = Operator::new(builder)
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?
            .finish();

        op.read(path).await.map(|buf| buf.to_vec()).map_err(|e| {
            if e.kind() == opendal::ErrorKind::NotFound {
                FileStorageError::NotFound(path.to_string())
            } else {
                FileStorageError::StorageError(e.to_string())
            }
        })
    }

    async fn delete_file(&self, path: &str) -> Result<bool, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.delete_file(path).await;
        }

        let Some(config) = &self.config else {
            return Err(FileStorageError::ConnectionError(
                "No configuration provided".to_string(),
            ));
        };

        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        let op = Operator::new(builder)
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?
            .finish();

        // Check if file exists first
        let exists = op.exists(path).await.unwrap_or(false);

        if exists {
            op.delete(path)
                .await
                .map_err(|e| FileStorageError::StorageError(e.to_string()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn list_files(&self, prefix: &str) -> Result<Vec<FileMetadata>, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.list_files(prefix).await;
        }

        let Some(config) = &self.config else {
            return Err(FileStorageError::ConnectionError(
                "No configuration provided".to_string(),
            ));
        };

        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        let op = Operator::new(builder)
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?
            .finish();

        let entries = op
            .list(prefix)
            .await
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?;

        let mut files = Vec::new();
        for entry in entries {
            let path = entry.path();
            if entry.metadata().is_file() {
                let filename = path.split('/').next_back().unwrap_or(path).to_string();
                files.push(FileMetadata {
                    id: path.to_string(),
                    filename,
                    content_type: "application/octet-stream".to_string(), // R2 doesn't store content-type in list
                    size: entry.metadata().content_length(),
                    description: None,
                });
            }
        }

        Ok(files)
    }

    async fn file_exists(&self, path: &str) -> Result<bool, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.file_exists(path).await;
        }

        let Some(config) = &self.config else {
            return Err(FileStorageError::ConnectionError(
                "No configuration provided".to_string(),
            ));
        };

        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        let op = Operator::new(builder)
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?
            .finish();

        Ok(op.exists(path).await.unwrap_or(false))
    }

    async fn get_file_metadata(&self, path: &str) -> Result<Option<FileMetadata>, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.get_file_metadata(path).await;
        }

        let Some(config) = &self.config else {
            return Err(FileStorageError::ConnectionError(
                "No configuration provided".to_string(),
            ));
        };

        use opendal::Operator;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        let op = Operator::new(builder)
            .map_err(|e| FileStorageError::StorageError(e.to_string()))?
            .finish();

        match op.stat(path).await {
            Ok(meta) => {
                let filename = path.split('/').next_back().unwrap_or(path).to_string();
                Ok(Some(FileMetadata {
                    id: path.to_string(),
                    filename,
                    content_type: meta
                        .content_type()
                        .unwrap_or("application/octet-stream")
                        .to_string(),
                    size: meta.content_length(),
                    description: None,
                }))
            }
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(FileStorageError::StorageError(e.to_string())),
        }
    }
}

// ============================================================================
// Google Cloud Storage Implementations
// ============================================================================

/// Configuration for Google Cloud Storage
#[derive(Clone)]
pub struct GDDiskConfig {
    /// GCS bucket name
    pub bucket: String,
    /// GCS credentials JSON
    pub credentials: String,
}

/// Google Cloud Storage implementation for connectivity checks
#[derive(Clone)]
pub struct GDDisk {
    /// Configuration for Google Cloud Storage
    #[allow(dead_code)]
    config: Option<GDDiskConfig>,
}

impl GDDisk {
    /// Create a new GDDisk instance for testing (no config)
    pub fn new_for_test() -> Self {
        Self { config: None }
    }

    /// Create a new GDDisk instance with configuration
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
            // Test mode - always return true
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // MockFileStorage Tests
    // ========================================================================

    #[tokio::test]
    async fn test_mock_upload_file() {
        let storage = MockFileStorage::new();

        let request = FileUploadRequest::new(
            "test/image.png",
            b"fake image content".to_vec(),
            "image/png",
        );

        let metadata = storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        assert_eq!(metadata.id, "test/image.png");
        assert_eq!(metadata.filename, "image.png");
        assert_eq!(metadata.content_type, "image/png");
        assert_eq!(metadata.size, 18); // "fake image content".len()
        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_upload_with_description() {
        let storage = MockFileStorage::new();

        let request =
            FileUploadRequest::new("photos/vacation.jpg", b"photo bytes".to_vec(), "image/jpeg")
                .with_description("My vacation photo");

        let metadata = storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        assert_eq!(metadata.description, Some("My vacation photo".to_string()));
    }

    #[tokio::test]
    async fn test_mock_download_file() {
        let storage = MockFileStorage::new();
        let content = b"test content".to_vec();

        let request = FileUploadRequest::new("test/file.txt", content.clone(), "text/plain");
        storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        let downloaded = storage
            .download_file("test/file.txt")
            .await
            .expect("download should succeed");
        assert_eq!(downloaded, content);
    }

    #[tokio::test]
    async fn test_mock_download_not_found() {
        let storage = MockFileStorage::new();

        let result = storage.download_file("nonexistent.txt").await;
        assert!(matches!(result, Err(FileStorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_mock_delete_file() {
        let storage = MockFileStorage::new();

        let request = FileUploadRequest::new("test/file.txt", b"content".to_vec(), "text/plain");
        storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        assert!(
            storage
                .file_exists("test/file.txt")
                .await
                .expect("should not error")
        );

        let deleted = storage
            .delete_file("test/file.txt")
            .await
            .expect("delete should succeed");
        assert!(deleted);

        assert!(
            !storage
                .file_exists("test/file.txt")
                .await
                .expect("should not error")
        );
    }

    #[tokio::test]
    async fn test_mock_delete_nonexistent() {
        let storage = MockFileStorage::new();

        let deleted = storage
            .delete_file("nonexistent.txt")
            .await
            .expect("should not error");
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_mock_list_files() {
        let storage = MockFileStorage::new();

        // Upload files in different directories
        let requests = vec![
            FileUploadRequest::new("images/photo1.jpg", b"p1".to_vec(), "image/jpeg"),
            FileUploadRequest::new("images/photo2.jpg", b"p2".to_vec(), "image/jpeg"),
            FileUploadRequest::new("videos/clip.mp4", b"v1".to_vec(), "video/mp4"),
        ];

        for req in requests {
            storage
                .upload_file(req)
                .await
                .expect("upload should succeed");
        }

        // List only images
        let images = storage
            .list_files("images/")
            .await
            .expect("list should succeed");
        assert_eq!(images.len(), 2);

        // List all files
        let all = storage.list_files("").await.expect("list should succeed");
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn test_mock_file_exists() {
        let storage = MockFileStorage::new();

        assert!(
            !storage
                .file_exists("test.txt")
                .await
                .expect("should not error")
        );

        let request = FileUploadRequest::new("test.txt", b"content".to_vec(), "text/plain");
        storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        assert!(
            storage
                .file_exists("test.txt")
                .await
                .expect("should not error")
        );
    }

    #[tokio::test]
    async fn test_mock_get_file_metadata() {
        let storage = MockFileStorage::new();

        let request =
            FileUploadRequest::new("docs/readme.md", b"# Hello".to_vec(), "text/markdown")
                .with_description("Project README");

        storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        let metadata = storage
            .get_file_metadata("docs/readme.md")
            .await
            .expect("should not error")
            .expect("metadata should exist");

        assert_eq!(metadata.id, "docs/readme.md");
        assert_eq!(metadata.filename, "readme.md");
        assert_eq!(metadata.content_type, "text/markdown");
        assert_eq!(metadata.size, 7);
        assert_eq!(metadata.description, Some("Project README".to_string()));
    }

    #[tokio::test]
    async fn test_mock_get_file_metadata_not_found() {
        let storage = MockFileStorage::new();

        let metadata = storage
            .get_file_metadata("nonexistent.txt")
            .await
            .expect("should not error");

        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn test_mock_clear() {
        let storage = MockFileStorage::new();

        let request = FileUploadRequest::new("test.txt", b"content".to_vec(), "text/plain");
        storage
            .upload_file(request)
            .await
            .expect("upload should succeed");

        assert!(!storage.is_empty());

        storage.clear();

        assert!(storage.is_empty());
    }

    // ========================================================================
    // CFDisk Tests
    // ========================================================================

    #[tokio::test]
    async fn test_cfdisk_could_connected_test_mode() {
        let disk = CFDisk::new_for_test();
        assert!(
            disk.could_connected().await,
            "CFDisk should be connectable in test mode"
        );
    }

    #[tokio::test]
    async fn test_gddisk_could_connected_test_mode() {
        let disk = GDDisk::new_for_test();
        assert!(
            disk.could_connected().await,
            "GDDisk should be connectable in test mode"
        );
    }

    // ========================================================================
    // CFFileStorage Tests (using mock backend)
    // ========================================================================

    #[tokio::test]
    async fn test_cf_file_storage_test_mode() {
        let storage = CFFileStorage::new_for_test();

        let request = FileUploadRequest::new("test/image.png", b"fake image".to_vec(), "image/png");

        let metadata = storage
            .upload_file(request)
            .await
            .expect("upload should succeed");
        assert_eq!(metadata.id, "test/image.png");

        let content = storage
            .download_file("test/image.png")
            .await
            .expect("download should succeed");
        assert_eq!(content, b"fake image".to_vec());
    }

    // ========================================================================
    // Generic Trait Tests
    // ========================================================================

    async fn check_storage_connection<T: OpenDALDisk>(storage: T) -> bool {
        storage.could_connected().await
    }

    #[tokio::test]
    async fn test_generic_storage_interface() {
        let cf_disk = CFDisk::new_for_test();
        let gd_disk = GDDisk::new_for_test();

        // Both implementations work with the generic interface
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
            .expect("upload should succeed");

        assert_eq!(metadata.id, "test/file.bin");
    }

    // ========================================================================
    // FileMetadata Tests
    // ========================================================================

    #[test]
    fn test_file_metadata_new() {
        let metadata = FileMetadata::new("id123", "photo.jpg", "image/jpeg", 1024);

        assert_eq!(metadata.id, "id123");
        assert_eq!(metadata.filename, "photo.jpg");
        assert_eq!(metadata.content_type, "image/jpeg");
        assert_eq!(metadata.size, 1024);
        assert!(metadata.description.is_none());
    }

    #[test]
    fn test_file_metadata_with_description() {
        let metadata = FileMetadata::new("id123", "photo.jpg", "image/jpeg", 1024)
            .with_description("A beautiful sunset");

        assert_eq!(metadata.description, Some("A beautiful sunset".to_string()));
    }
}
