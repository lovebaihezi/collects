//! File storage types.

/// Metadata for an uploaded file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub description: Option<String>,
}

impl FileMetadata {
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

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Request to upload a file.
#[derive(Debug, Clone)]
pub struct FileUploadRequest {
    pub path: String,
    pub content: Vec<u8>,
    pub content_type: String,
    pub description: Option<String>,
}

impl FileUploadRequest {
    pub fn new(path: impl Into<String>, content: Vec<u8>, content_type: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content,
            content_type: content_type.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Error type for file storage operations.
#[derive(Debug, thiserror::Error)]
pub enum FileStorageError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("File already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid file type: {0}")]
    InvalidFileType(String),

    #[error("File too large: {size} bytes exceeds maximum {max_size} bytes")]
    FileTooLarge { size: u64, max_size: u64 },

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),
}
