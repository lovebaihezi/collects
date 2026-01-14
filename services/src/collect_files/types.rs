//! Collect file types.

/// Status of a collect file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectFileStatus {
    #[default]
    Draft,
    Published,
    Archived,
}

/// A file associated with a collect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectFile {
    pub id: String,
    pub collect_id: String,
    pub storage_path: String,
    pub filename: String,
    pub content_type: String,
    pub size: u64,
    pub description: Option<String>,
    pub status: CollectFileStatus,
}

impl CollectFile {
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

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_status(mut self, status: CollectFileStatus) -> Self {
        self.status = status;
        self
    }
}

/// Request to upload a file to a collect.
#[derive(Debug, Clone)]
pub struct CollectFileUpload {
    pub collect_id: String,
    pub path: String,
    pub content: Vec<u8>,
    pub content_type: String,
    pub description: Option<String>,
}

impl CollectFileUpload {
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

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// Result of a batch upload operation.
#[derive(Debug, Clone, Default)]
pub struct BatchUploadResult {
    pub successful: Vec<CollectFile>,
    pub failed: Vec<(String, String)>,
}

impl BatchUploadResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn all_succeeded(&self) -> bool {
        self.failed.is_empty()
    }

    pub fn total_processed(&self) -> usize {
        self.successful.len() + self.failed.len()
    }
}

/// Error type for collect file operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CollectFileError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("Collect not found: {0}")]
    CollectNotFound(String),

    #[error("Invalid file type: {0}")]
    InvalidFileType(String),

    #[error("File too large: {size} bytes exceeds maximum {max_size} bytes")]
    FileTooLarge { size: u64, max_size: u64 },

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),
}
