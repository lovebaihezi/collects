//! Shared types for the v1 API endpoints.

use crate::database::{self, ContentRow, Visibility};
use serde::{Deserialize, Serialize};

// =============================================================================
// Generic Error Response
// =============================================================================

/// Generic error response.
#[derive(Debug, Serialize)]
pub struct V1ErrorResponse {
    pub error: String,
    pub message: String,
}

impl V1ErrorResponse {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            error: "not_found".to_string(),
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            error: "bad_request".to_string(),
            message: message.into(),
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            error: "internal_error".to_string(),
            message: message.into(),
        }
    }
}

// =============================================================================
// Contents API Types
// =============================================================================

/// Query parameters for listing contents.
#[derive(Debug, Deserialize, Default)]
pub struct V1ContentsListQuery {
    /// Maximum number of results to return (default: 50, max: 100)
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: Option<i64>,
    /// Filter by status: active, archived, trashed
    #[serde(default)]
    pub status: Option<String>,
}

/// A content item in API responses.
#[derive(Debug, Serialize)]
pub struct V1ContentItem {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub storage_backend: String,
    pub storage_profile: String,
    pub storage_key: String,
    pub content_type: String,
    pub file_size: i64,
    pub status: String,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ContentRow> for V1ContentItem {
    fn from(row: ContentRow) -> Self {
        Self {
            id: row.id.to_string(),
            title: row.title,
            description: row.description,
            storage_backend: row.storage_backend,
            storage_profile: row.storage_profile,
            storage_key: row.storage_key,
            content_type: row.content_type,
            file_size: row.file_size,
            status: row.status,
            visibility: row.visibility,
            trashed_at: row.trashed_at.map(|t| t.to_rfc3339()),
            archived_at: row.archived_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing contents.
#[derive(Debug, Serialize)]
pub struct V1ContentsListResponse {
    pub items: Vec<V1ContentItem>,
    pub total: usize,
}

/// Request body for updating content metadata.
#[derive(Debug, Deserialize)]
pub struct V1ContentsUpdateRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub visibility: Option<String>,
}

/// Request for view URL.
#[derive(Debug, Deserialize)]
pub struct V1ViewUrlRequest {
    pub disposition: String,
}

/// Response for view URL.
#[derive(Debug, Serialize)]
pub struct V1ViewUrlResponse {
    pub url: String,
    pub expires_at: String,
}

// =============================================================================
// Tags API Types
// =============================================================================

/// A tag item in API responses.
#[derive(Debug, Serialize)]
pub struct V1TagItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub created_at: String,
}

impl From<database::TagRow> for V1TagItem {
    fn from(row: database::TagRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            color: row.color,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

/// Response for listing tags.
#[derive(Debug, Serialize)]
pub struct V1TagsListResponse {
    pub items: Vec<V1TagItem>,
    pub total: usize,
}

/// Request body for creating a tag.
#[derive(Debug, Deserialize)]
pub struct V1TagCreateRequest {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

/// Request body for updating a tag.
#[derive(Debug, Deserialize)]
pub struct V1TagUpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<Option<String>>,
}

/// Request body for attaching tags to content.
#[derive(Debug, Deserialize)]
pub struct V1ContentTagsAttachRequest {
    pub tag_id: String,
}

// =============================================================================
// Groups API Types
// =============================================================================

/// Query parameters for listing groups.
#[derive(Debug, Deserialize)]
pub struct V1GroupsListQuery {
    /// Maximum number of results to return (default: 50, max: 100)
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: Option<i64>,
    /// Filter by status: active, archived, trashed
    #[serde(default)]
    pub status: Option<String>,
}

/// Response item for a group.
#[derive(Debug, Serialize)]
pub struct V1GroupItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub visibility: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<database::ContentGroupRow> for V1GroupItem {
    fn from(row: database::ContentGroupRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            description: row.description,
            visibility: row.visibility,
            status: row.status,
            trashed_at: row.trashed_at.map(|t| t.to_rfc3339()),
            archived_at: row.archived_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing groups.
#[derive(Debug, Serialize)]
pub struct V1GroupsListResponse {
    pub items: Vec<V1GroupItem>,
    pub total: usize,
}

/// Request body for creating a group.
#[derive(Debug, Deserialize)]
pub struct V1GroupCreateRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

fn default_visibility() -> String {
    "private".to_string()
}

/// Request body for updating a group.
#[derive(Debug, Deserialize)]
pub struct V1GroupUpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub visibility: Option<String>,
}

/// Response item for a group content item.
#[derive(Debug, Serialize)]
pub struct V1GroupContentItem {
    pub id: String,
    pub group_id: String,
    pub content_id: String,
    pub sort_order: i32,
    pub added_at: String,
}

impl From<database::ContentGroupItemRow> for V1GroupContentItem {
    fn from(row: database::ContentGroupItemRow) -> Self {
        Self {
            id: row.id.to_string(),
            group_id: row.group_id.to_string(),
            content_id: row.content_id.to_string(),
            sort_order: row.sort_order,
            added_at: row.added_at.to_rfc3339(),
        }
    }
}

/// Response for listing group contents.
#[derive(Debug, Serialize)]
pub struct V1GroupContentsListResponse {
    pub items: Vec<V1GroupContentItem>,
    pub total: usize,
}

/// Request body for adding content to a group.
#[derive(Debug, Deserialize)]
pub struct V1GroupAddContentRequest {
    pub content_id: String,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// Request body for reordering group contents.
#[derive(Debug, Deserialize)]
pub struct V1GroupReorderRequest {
    /// List of (content_id, sort_order) pairs
    pub items: Vec<V1GroupReorderItem>,
}

#[derive(Debug, Deserialize)]
pub struct V1GroupReorderItem {
    pub content_id: String,
    pub sort_order: i32,
}

// =============================================================================
// Uploads API Types
// =============================================================================

#[derive(Debug, Deserialize)]
pub struct V1UploadsInitRequest {
    pub filename: String,
    pub content_type: String,
    pub file_size: u64,
}

#[derive(Debug, Serialize)]
pub struct V1UploadsInitResponse {
    pub upload_id: String,
    pub storage_key: String,
    pub method: String,
    pub upload_url: String,
    pub expires_at: String,
}

// =============================================================================
// Me API Types
// =============================================================================

/// Response from the `/v1/me` endpoint containing authenticated user information.
#[derive(Debug, Serialize)]
pub struct V1MeResponse {
    /// The authenticated user's username.
    pub username: String,
    /// Token issued-at timestamp (Unix seconds).
    pub issued_at: i64,
    /// Token expiration timestamp (Unix seconds).
    pub expires_at: i64,
}

// =============================================================================
// Helper functions for parsing visibility
// =============================================================================

/// Parse visibility string to Visibility enum.
pub fn parse_visibility(s: &str) -> Option<Visibility> {
    match s {
        "private" => Some(Visibility::Private),
        "public" => Some(Visibility::Public),
        "restricted" => Some(Visibility::Restricted),
        _ => None,
    }
}
