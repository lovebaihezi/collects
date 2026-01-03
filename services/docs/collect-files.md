# Collect Files Module

Manages file uploads associated with collections.

## Architecture

```
collect_files/
├── mod.rs      - Module exports
├── types.rs    - CollectFile, CollectFileUpload, etc.
├── traits.rs   - CollectFileStorage trait
└── mock.rs     - MockCollectFileStorage (testing)
```

## Types

### `CollectFile`
File metadata with collection association:
- `id` - Unique identifier
- `collect_id` - Parent collection
- `storage_path` - Storage location
- `filename` - Original name
- `content_type` - MIME type
- `size` - Bytes
- `description` - Optional
- `status` - Draft/Published/Archived

### `CollectFileUpload`
Upload request with collection context.

### `BatchUploadResult`
Results from multi-file uploads.

## Trait: `CollectFileStorage`

Operations:
- `upload_file` - Single upload
- `upload_files` - Batch upload
- `get_file` - Get by ID
- `list_files_for_collect` - List for collection
- `delete_file` - Remove
- `update_file_status` - Change status
- `update_file_description` - Update description
- `download_file` - Get content

## Usage

```rust
use collects_services::collect_files::{
    CollectFileStorage, MockCollectFileStorage, CollectFileUpload
};

async fn example<S: CollectFileStorage>(storage: &S, collect_id: &str) {
    let uploads = vec![
        CollectFileUpload::new(collect_id, "photo.jpg", content, "image/jpeg")
            .with_description("Vacation photo"),
    ];
    
    let result = storage.upload_files(uploads).await?;
    assert!(result.all_succeeded());
}
```
