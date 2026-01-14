//! Presigned URL generation for storage backends.
//!
//! Provides S3 SigV4-compatible presigning for Cloudflare R2 using OpenDAL.

use std::time::Duration;

use super::cloudflare::CFDiskConfig;

/// Result of a presign operation.
#[derive(Debug, Clone)]
pub struct PresignedUrl {
    /// The presigned URL.
    pub url: String,
    /// When the URL expires (UTC).
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// Error type for presigning operations.
#[derive(Debug, thiserror::Error)]
pub enum PresignError {
    #[error("No configuration provided")]
    NoConfig,

    #[error("Presigning failed: {0}")]
    PresignFailed(String),

    #[error("Storage error: {0}")]
    StorageError(String),
}

/// Content disposition for GET presigned URLs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentDisposition {
    /// Display inline in browser.
    Inline,
    /// Download as attachment.
    Attachment,
}

impl TryFrom<&str> for ContentDisposition {
    type Error = ();

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "inline" => Ok(Self::Inline),
            "attachment" => Ok(Self::Attachment),
            _ => Err(()),
        }
    }
}

/// Presigner for Cloudflare R2 storage.
#[derive(Clone)]
pub struct R2Presigner {
    config: Option<CFDiskConfig>,
}

impl R2Presigner {
    /// Create a new presigner with the given configuration.
    pub fn new(config: CFDiskConfig) -> Self {
        Self {
            config: Some(config),
        }
    }

    /// Create a presigner for testing (no actual R2 operations).
    pub fn new_for_test() -> Self {
        Self { config: None }
    }

    /// Create an OpenDAL operator for R2.
    fn create_operator(&self) -> Result<opendal::Operator, PresignError> {
        let config = self.config.as_ref().ok_or(PresignError::NoConfig)?;

        let builder = opendal::services::S3::default()
            .bucket(&config.bucket)
            .region("auto")
            .access_key_id(&config.access_key_id)
            .secret_access_key(&config.secret_access_key)
            .endpoint(&format!(
                "https://{}.r2.cloudflarestorage.com",
                config.account_id
            ));

        opendal::Operator::new(builder)
            .map(|op| op.finish())
            .map_err(|e| PresignError::StorageError(e.to_string()))
    }

    /// Generate a presigned PUT URL for uploading a file.
    ///
    /// # Arguments
    /// * `storage_key` - The path/key where the file will be stored
    /// * `content_type` - MIME type of the file being uploaded
    /// * `expires_in` - How long until the URL expires
    ///
    /// # Returns
    /// A presigned URL that can be used to PUT the file directly to R2.
    pub async fn presign_put(
        &self,
        storage_key: &str,
        content_type: &str,
        expires_in: Duration,
    ) -> Result<PresignedUrl, PresignError> {
        // For test mode, return a mock URL
        if self.config.is_none() {
            let expires_at = chrono::Utc::now() + chrono::Duration::from_std(expires_in).unwrap();
            return Ok(PresignedUrl {
                url: format!("https://test.r2.example.com/{storage_key}?mock=true"),
                expires_at,
            });
        }

        let op = self.create_operator()?;
        let expires_at = chrono::Utc::now() + chrono::Duration::from_std(expires_in).unwrap();

        // Use OpenDAL's presign_write with content_type override
        let presigned = op
            .presign_write_with(storage_key, expires_in)
            .content_type(content_type)
            .await
            .map_err(|e| PresignError::PresignFailed(e.to_string()))?;

        Ok(PresignedUrl {
            url: presigned.uri().to_string(),
            expires_at,
        })
    }

    /// Generate a presigned GET URL for downloading/viewing a file.
    ///
    /// # Arguments
    /// * `storage_key` - The path/key of the file to retrieve
    /// * `disposition` - Whether to display inline or download as attachment
    /// * `expires_in` - How long until the URL expires
    ///
    /// # Returns
    /// A presigned URL that can be used to GET the file from R2.
    pub async fn presign_get(
        &self,
        storage_key: &str,
        disposition: ContentDisposition,
        expires_in: Duration,
    ) -> Result<PresignedUrl, PresignError> {
        // For test mode, return a mock URL
        if self.config.is_none() {
            let expires_at = chrono::Utc::now() + chrono::Duration::from_std(expires_in).unwrap();
            let disp = match disposition {
                ContentDisposition::Inline => "inline",
                ContentDisposition::Attachment => "attachment",
            };
            return Ok(PresignedUrl {
                url: format!(
                    "https://test.r2.example.com/{storage_key}?mock=true&disposition={disp}"
                ),
                expires_at,
            });
        }

        let op = self.create_operator()?;
        let expires_at = chrono::Utc::now() + chrono::Duration::from_std(expires_in).unwrap();

        // Build the presign read request
        let mut presign_builder = op.presign_read_with(storage_key, expires_in);

        // Set content disposition header override
        let disposition_value = match disposition {
            ContentDisposition::Inline => "inline".to_owned(),
            ContentDisposition::Attachment => "attachment".to_owned(),
        };
        presign_builder = presign_builder.override_content_disposition(&disposition_value);

        let presigned = presign_builder
            .await
            .map_err(|e| PresignError::PresignFailed(e.to_string()))?;

        Ok(PresignedUrl {
            url: presigned.uri().to_string(),
            expires_at,
        })
    }

    /// Check if a file exists at the given storage key.
    ///
    /// Used to verify uploads completed successfully.
    pub async fn file_exists(&self, storage_key: &str) -> Result<bool, PresignError> {
        // For test mode, always return true
        if self.config.is_none() {
            return Ok(true);
        }

        let op = self.create_operator()?;
        op.exists(storage_key)
            .await
            .map_err(|e| PresignError::StorageError(e.to_string()))
    }

    /// Get metadata for a file at the given storage key.
    ///
    /// Returns None if the file doesn't exist.
    pub async fn get_metadata(
        &self,
        storage_key: &str,
    ) -> Result<Option<FileMetadata>, PresignError> {
        // For test mode, return mock metadata
        if self.config.is_none() {
            return Ok(Some(FileMetadata {
                content_type: "application/octet-stream".to_owned(),
                content_length: 1024,
            }));
        }

        let op = self.create_operator()?;
        match op.stat(storage_key).await {
            Ok(meta) => Ok(Some(FileMetadata {
                content_type: meta
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_owned(),
                content_length: meta.content_length(),
            })),
            Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(PresignError::StorageError(e.to_string())),
        }
    }
}

/// Metadata about a stored file.
#[derive(Debug, Clone)]
pub struct FileMetadata {
    /// MIME type of the file.
    pub content_type: String,
    /// Size of the file in bytes.
    pub content_length: u64,
}

/// Default expiration time for presigned URLs (15 minutes).
pub const DEFAULT_PRESIGN_EXPIRY: Duration = Duration::from_secs(15 * 60);

/// Maximum expiration time for presigned URLs (7 days, R2 limit).
pub const MAX_PRESIGN_EXPIRY: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_presigner_test_mode_put() {
        let presigner = R2Presigner::new_for_test();
        let result = presigner
            .presign_put("test/file.txt", "text/plain", Duration::from_secs(300))
            .await
            .unwrap();

        assert!(result.url.contains("test/file.txt"));
        assert!(result.url.contains("mock=true"));
        assert!(result.expires_at > chrono::Utc::now());
    }

    #[tokio::test]
    async fn test_presigner_test_mode_get() {
        let presigner = R2Presigner::new_for_test();
        let result = presigner
            .presign_get(
                "test/file.txt",
                ContentDisposition::Inline,
                Duration::from_secs(300),
            )
            .await
            .unwrap();

        assert!(result.url.contains("test/file.txt"));
        assert!(result.url.contains("disposition=inline"));
    }

    #[tokio::test]
    async fn test_presigner_test_mode_file_exists() {
        let presigner = R2Presigner::new_for_test();
        let exists = presigner.file_exists("any/path").await.unwrap();
        assert!(exists);
    }

    #[test]
    fn test_content_disposition_try_from() {
        assert_eq!(
            ContentDisposition::try_from("inline"),
            Ok(ContentDisposition::Inline)
        );
        assert_eq!(
            ContentDisposition::try_from("INLINE"),
            Ok(ContentDisposition::Inline)
        );
        assert_eq!(
            ContentDisposition::try_from("attachment"),
            Ok(ContentDisposition::Attachment)
        );
        assert_eq!(
            ContentDisposition::try_from("Attachment"),
            Ok(ContentDisposition::Attachment)
        );
        assert_eq!(ContentDisposition::try_from("invalid"), Err(()));
    }
}
