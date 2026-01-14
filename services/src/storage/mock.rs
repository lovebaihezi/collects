//! Mock file storage for testing.

use super::traits::FileStorage;
use super::types::{FileMetadata, FileStorageError, FileUploadRequest};

/// In-memory mock implementation of `FileStorage` for testing.
#[derive(Clone, Default)]
pub struct MockFileStorage {
    files: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, MockFile>>>,
}

#[derive(Clone)]
struct MockFile {
    content: Vec<u8>,
    metadata: FileMetadata,
}

impl MockFileStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.files.read().expect("lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.files.write().expect("lock poisoned").clear();
    }
}

impl FileStorage for MockFileStorage {
    type Error = FileStorageError;

    async fn upload_file(&self, request: FileUploadRequest) -> Result<FileMetadata, Self::Error> {
        let mut files = self.files.write().expect("lock poisoned");

        let filename = request
            .path
            .split('/')
            .next_back()
            .unwrap_or(&request.path)
            .to_owned();

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
            .ok_or_else(|| FileStorageError::NotFound(path.to_owned()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_upload_file() {
        let storage = MockFileStorage::new();
        let request = FileUploadRequest::new(
            "test/image.png",
            b"fake image content".to_vec(),
            "image/png",
        );

        let metadata = storage.upload_file(request).await.unwrap();

        assert_eq!(metadata.id, "test/image.png");
        assert_eq!(metadata.filename, "image.png");
        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn test_download_file() {
        let storage = MockFileStorage::new();
        let content = b"test content".to_vec();

        storage
            .upload_file(FileUploadRequest::new(
                "test/file.txt",
                content.clone(),
                "text/plain",
            ))
            .await
            .unwrap();

        let downloaded = storage.download_file("test/file.txt").await.unwrap();
        assert_eq!(downloaded, content);
    }

    #[tokio::test]
    async fn test_download_not_found() {
        let storage = MockFileStorage::new();
        let result = storage.download_file("nonexistent.txt").await;
        assert!(matches!(result, Err(FileStorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_file() {
        let storage = MockFileStorage::new();
        storage
            .upload_file(FileUploadRequest::new(
                "test.txt",
                b"content".to_vec(),
                "text/plain",
            ))
            .await
            .unwrap();

        assert!(storage.delete_file("test.txt").await.unwrap());
        assert!(storage.is_empty());
    }

    #[tokio::test]
    async fn test_list_files() {
        let storage = MockFileStorage::new();

        for (path, content_type) in [
            ("images/photo1.jpg", "image/jpeg"),
            ("images/photo2.jpg", "image/jpeg"),
            ("videos/clip.mp4", "video/mp4"),
        ] {
            storage
                .upload_file(FileUploadRequest::new(path, b"x".to_vec(), content_type))
                .await
                .unwrap();
        }

        assert_eq!(storage.list_files("images/").await.unwrap().len(), 2);
        assert_eq!(storage.list_files("").await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_file_exists() {
        let storage = MockFileStorage::new();
        assert!(!storage.file_exists("test.txt").await.unwrap());

        storage
            .upload_file(FileUploadRequest::new(
                "test.txt",
                b"content".to_vec(),
                "text/plain",
            ))
            .await
            .unwrap();

        assert!(storage.file_exists("test.txt").await.unwrap());
    }
}
