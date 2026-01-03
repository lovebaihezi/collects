//! Mock collect file storage for testing.

use super::traits::CollectFileStorage;
use super::types::{
    BatchUploadResult, CollectFile, CollectFileError, CollectFileStatus, CollectFileUpload,
};

/// In-memory mock implementation of `CollectFileStorage` for testing.
#[derive(Clone, Default)]
pub struct MockCollectFileStorage {
    files: std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, MockStoredFile>>>,
    next_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

#[derive(Clone)]
struct MockStoredFile {
    metadata: CollectFile,
    content: Vec<u8>,
}

impl MockCollectFileStorage {
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

        let file = storage.upload_file(upload).await.unwrap();

        assert_eq!(file.collect_id, "collect-123");
        assert_eq!(file.filename, "photo.jpg");
        assert_eq!(file.status, CollectFileStatus::Draft);
    }

    #[tokio::test]
    async fn test_upload_multiple_files() {
        let storage = MockCollectFileStorage::new();

        let uploads = vec![
            CollectFileUpload::new("collect-123", "a.jpg", b"a".to_vec(), "image/jpeg"),
            CollectFileUpload::new("collect-123", "b.jpg", b"b".to_vec(), "image/jpeg"),
            CollectFileUpload::new("collect-123", "c.mp4", b"c".to_vec(), "video/mp4"),
        ];

        let result = storage.upload_files(uploads).await.unwrap();

        assert!(result.all_succeeded());
        assert_eq!(result.total_processed(), 3);
    }

    #[tokio::test]
    async fn test_list_files_for_collect() {
        let storage = MockCollectFileStorage::new();

        let uploads = vec![
            CollectFileUpload::new("collect-a", "a1.jpg", b"a1".to_vec(), "image/jpeg"),
            CollectFileUpload::new("collect-a", "a2.jpg", b"a2".to_vec(), "image/jpeg"),
            CollectFileUpload::new("collect-b", "b1.jpg", b"b1".to_vec(), "image/jpeg"),
        ];

        storage.upload_files(uploads).await.unwrap();

        assert_eq!(
            storage
                .list_files_for_collect("collect-a")
                .await
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            storage
                .list_files_for_collect("collect-b")
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_delete_file() {
        let storage = MockCollectFileStorage::new();

        let upload =
            CollectFileUpload::new("collect-123", "test.txt", b"content".to_vec(), "text/plain");
        let file = storage.upload_file(upload).await.unwrap();

        assert!(storage.delete_file(&file.id).await.unwrap());
        assert!(storage.is_empty());
    }

    #[tokio::test]
    async fn test_update_file_status() {
        let storage = MockCollectFileStorage::new();

        let upload =
            CollectFileUpload::new("collect-123", "test.txt", b"content".to_vec(), "text/plain");
        let file = storage.upload_file(upload).await.unwrap();

        let updated = storage
            .update_file_status(&file.id, CollectFileStatus::Published)
            .await
            .unwrap();

        assert_eq!(updated.status, CollectFileStatus::Published);
    }

    #[tokio::test]
    async fn test_download_nonexistent_file() {
        let storage = MockCollectFileStorage::new();
        let result = storage.download_file("nonexistent").await;
        assert!(matches!(result, Err(CollectFileError::NotFound(_))));
    }
}
