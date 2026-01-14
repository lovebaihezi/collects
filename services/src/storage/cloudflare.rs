//! Cloudflare R2 storage implementations.

use super::mock::MockFileStorage;
use super::traits::{FileStorage, OpenDALDisk};
use super::types::{FileMetadata, FileStorageError, FileUploadRequest};

/// Configuration for Cloudflare R2.
#[derive(Clone)]
pub struct CFDiskConfig {
    pub account_id: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub bucket: String,
}

/// Cloudflare R2 connectivity checker.
#[derive(Clone)]
pub struct CFDisk {
    config: Option<CFDiskConfig>,
}

impl CFDisk {
    pub fn new_for_test() -> Self {
        Self { config: None }
    }

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
            return true;
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

        match Operator::new(builder) {
            Ok(op) => op.finish().check().await.is_ok(),
            Err(_) => false,
        }
    }
}

/// Cloudflare R2 file storage.
#[derive(Clone)]
pub struct CFFileStorage {
    #[allow(dead_code)]
    config: Option<CFDiskConfig>,
    mock: Option<MockFileStorage>,
}

impl CFFileStorage {
    pub fn new_for_test() -> Self {
        Self {
            config: None,
            mock: Some(MockFileStorage::new()),
        }
    }

    pub fn new(config: CFDiskConfig) -> Self {
        Self {
            config: Some(config),
            mock: None,
        }
    }

    #[cfg(not(test))]
    fn create_operator(&self) -> Result<opendal::Operator, FileStorageError> {
        let config = self.config.as_ref().ok_or_else(|| {
            FileStorageError::ConnectionError("No configuration provided".to_owned())
        })?;

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
            .map_err(|e| FileStorageError::StorageError(e.to_string()))
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

        #[cfg(not(test))]
        {
            let op = self.create_operator()?;
            op.write(&request.path, request.content.clone())
                .await
                .map_err(|e| FileStorageError::StorageError(e.to_string()))?;

            let filename = request
                .path
                .split('/')
                .next_back()
                .unwrap_or(&request.path)
                .to_owned();

            Ok(FileMetadata {
                id: request.path,
                filename,
                content_type: request.content_type,
                size: request.content.len() as u64,
                description: request.description,
            })
        }

        #[cfg(test)]
        Err(FileStorageError::ConnectionError(
            "No mock storage configured for test".to_owned(),
        ))
    }

    async fn download_file(&self, path: &str) -> Result<Vec<u8>, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.download_file(path).await;
        }

        #[cfg(not(test))]
        {
            let op = self.create_operator()?;
            op.read(path).await.map(|buf| buf.to_vec()).map_err(|e| {
                if e.kind() == opendal::ErrorKind::NotFound {
                    FileStorageError::NotFound(path.to_owned())
                } else {
                    FileStorageError::StorageError(e.to_string())
                }
            })
        }

        #[cfg(test)]
        Err(FileStorageError::ConnectionError(
            "No mock storage configured for test".to_owned(),
        ))
    }

    async fn delete_file(&self, path: &str) -> Result<bool, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.delete_file(path).await;
        }

        #[cfg(not(test))]
        {
            let op = self.create_operator()?;
            let exists = op
                .exists(path)
                .await
                .map_err(|e| FileStorageError::StorageError(e.to_string()))?;

            if exists {
                op.delete(path)
                    .await
                    .map_err(|e| FileStorageError::StorageError(e.to_string()))?;
                Ok(true)
            } else {
                Ok(false)
            }
        }

        #[cfg(test)]
        Err(FileStorageError::ConnectionError(
            "No mock storage configured for test".to_owned(),
        ))
    }

    async fn list_files(&self, prefix: &str) -> Result<Vec<FileMetadata>, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.list_files(prefix).await;
        }

        #[cfg(not(test))]
        {
            let op = self.create_operator()?;
            let entries = op
                .list(prefix)
                .await
                .map_err(|e| FileStorageError::StorageError(e.to_string()))?;

            let mut files = Vec::new();
            for entry in entries {
                let path = entry.path();
                if entry.metadata().is_file() {
                    let filename = path.split('/').next_back().unwrap_or(path).to_owned();
                    files.push(FileMetadata {
                        id: path.to_owned(),
                        filename,
                        content_type: "application/octet-stream".to_owned(),
                        size: entry.metadata().content_length(),
                        description: None,
                    });
                }
            }
            Ok(files)
        }

        #[cfg(test)]
        Err(FileStorageError::ConnectionError(
            "No mock storage configured for test".to_owned(),
        ))
    }

    async fn file_exists(&self, path: &str) -> Result<bool, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.file_exists(path).await;
        }

        #[cfg(not(test))]
        {
            let op = self.create_operator()?;
            op.exists(path)
                .await
                .map_err(|e| FileStorageError::StorageError(e.to_string()))
        }

        #[cfg(test)]
        Err(FileStorageError::ConnectionError(
            "No mock storage configured for test".to_owned(),
        ))
    }

    async fn get_file_metadata(&self, path: &str) -> Result<Option<FileMetadata>, Self::Error> {
        if let Some(mock) = &self.mock {
            return mock.get_file_metadata(path).await;
        }

        #[cfg(not(test))]
        {
            let op = self.create_operator()?;
            match op.stat(path).await {
                Ok(meta) => {
                    let filename = path.split('/').next_back().unwrap_or(path).to_owned();
                    Ok(Some(FileMetadata {
                        id: path.to_owned(),
                        filename,
                        content_type: meta
                            .content_type()
                            .unwrap_or("application/octet-stream")
                            .to_owned(),
                        size: meta.content_length(),
                        description: None,
                    }))
                }
                Err(e) if e.kind() == opendal::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(FileStorageError::StorageError(e.to_string())),
            }
        }

        #[cfg(test)]
        Err(FileStorageError::ConnectionError(
            "No mock storage configured for test".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cfdisk_could_connected_test_mode() {
        let disk = CFDisk::new_for_test();
        assert!(disk.could_connected().await);
    }

    #[tokio::test]
    async fn test_cf_file_storage_test_mode() {
        let storage = CFFileStorage::new_for_test();

        let request = FileUploadRequest::new("test/image.png", b"fake image".to_vec(), "image/png");
        let metadata = storage.upload_file(request).await.unwrap();
        assert_eq!(metadata.id, "test/image.png");

        let content = storage.download_file("test/image.png").await.unwrap();
        assert_eq!(content, b"fake image".to_vec());
    }
}
