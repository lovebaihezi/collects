# Remote Storage Module

This module provides trait-based abstractions for remote file storage using OpenDAL.

## Architecture

```
storage/
├── mod.rs          - Module exports
├── types.rs        - FileMetadata, FileUploadRequest, FileStorageError
├── traits.rs       - FileStorage, OpenDALDisk traits
├── mock.rs         - MockFileStorage (testing)
├── cloudflare.rs   - CFDisk, CFFileStorage (R2)
└── gcs.rs          - GDDisk (Google Cloud Storage)
```

## Traits

### `FileStorage`
Generic interface for file operations:
- `upload_file` - Upload with metadata
- `download_file` - Download content
- `delete_file` - Remove file
- `list_files` - List by prefix
- `file_exists` - Check existence
- `get_file_metadata` - Get metadata only

### `OpenDALDisk`
Connectivity check interface:
- `could_connected` - Test connection

## Implementations

| Implementation | Backend | Use Case |
|---------------|---------|----------|
| `MockFileStorage` | In-memory | Testing |
| `CFFileStorage` | Cloudflare R2 | Production |
| `GDDisk` | Google Cloud Storage | Production |

## Configuration

Credentials are stored in Google Cloud Secret Manager:

```bash
# Cloudflare R2
CF_ACCOUNT_ID=$(gcloud secrets versions access latest --secret=cf-account-id)
CF_ACCESS_KEY_ID=$(gcloud secrets versions access latest --secret=cf-access-key-id)
CF_SECRET_ACCESS_KEY=$(gcloud secrets versions access latest --secret=cf-secret-access-key)
CF_BUCKET=$(gcloud secrets versions access latest --secret=cf-bucket)
```

## Usage

```rust
use collects_services::storage::{FileStorage, MockFileStorage, FileUploadRequest};

async fn example<S: FileStorage>(storage: &S) {
    let request = FileUploadRequest::new("images/photo.jpg", content, "image/jpeg");
    let metadata = storage.upload_file(request).await?;
    println!("Uploaded: {}", metadata.id);
}
```
