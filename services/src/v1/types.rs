//! Shared types for the v1 API endpoints.

use crate::database::{self, ContentRow, ShareLinkRow, SharePermission, Visibility};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// =============================================================================
// Generic Error Response
// =============================================================================

/// Generic error response.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ErrorResponse {
    /// Error type identifier.
    pub error: String,
    /// Human-readable error message.
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
#[derive(Debug, Deserialize, Default, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ContentItem {
    /// Unique identifier (UUID).
    pub id: String,
    /// Content title.
    pub title: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Storage backend identifier.
    pub storage_backend: String,
    /// Storage profile identifier.
    pub storage_profile: String,
    /// Storage key/path.
    pub storage_key: String,
    /// MIME type of the content.
    pub content_type: String,
    /// File size in bytes.
    pub file_size: i64,
    /// Content status (active, archived, trashed).
    pub status: String,
    /// Content visibility (private, public, restricted).
    pub visibility: String,
    /// Content kind: "file" (uploaded to R2) or "text" (stored inline).
    pub kind: String,
    /// Inline text content (only present when kind="text").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    /// Timestamp when content was trashed (ISO 8601 format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<String>,
    /// Timestamp when content was archived (ISO 8601 format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    /// Timestamp when content was created (ISO 8601 format).
    pub created_at: String,
    /// Timestamp when content was last updated (ISO 8601 format).
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
            kind: row.kind,
            body: row.body,
            trashed_at: row.trashed_at.map(|t| t.to_rfc3339()),
            archived_at: row.archived_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing contents.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ContentsListResponse {
    /// List of content items.
    pub items: Vec<V1ContentItem>,
    /// Total number of items returned.
    pub total: usize,
}

/// Request body for updating content metadata.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ContentsUpdateRequest {
    /// New title (optional).
    #[serde(default)]
    pub title: Option<String>,
    /// New description (optional, pass null to clear).
    #[serde(default)]
    pub description: Option<Option<String>>,
    /// New visibility (optional: private, public, restricted).
    #[serde(default)]
    pub visibility: Option<String>,
    /// New body content (optional, only allowed when kind="text").
    #[serde(default)]
    pub body: Option<String>,
}

/// Request body for creating text content directly (without upload).
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ContentCreateRequest {
    /// Content title.
    pub title: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Text content body (required, max recommended 64KB).
    pub body: String,
    /// MIME type of the content (default: "text/plain").
    #[serde(default = "default_text_content_type")]
    pub content_type: String,
    /// Content visibility (private, public, restricted). Default: private.
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

fn default_text_content_type() -> String {
    "text/plain".to_string()
}

/// Response for creating text content.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ContentCreateResponse {
    /// The created content item.
    pub content: V1ContentItem,
}

/// Request for view URL.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ViewUrlRequest {
    /// Content disposition: inline or attachment.
    pub disposition: String,
}

/// Response for view URL.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ViewUrlResponse {
    /// Presigned URL for viewing/downloading content.
    pub url: String,
    /// URL expiration timestamp (ISO 8601 format).
    pub expires_at: String,
}

// =============================================================================
// Tags API Types
// =============================================================================

/// A tag item in API responses.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1TagItem {
    /// Unique identifier (UUID).
    pub id: String,
    /// Tag name.
    pub name: String,
    /// Optional hex color code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Timestamp when tag was created (ISO 8601 format).
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
#[derive(Debug, Serialize, ToSchema)]
pub struct V1TagsListResponse {
    /// List of tags.
    pub items: Vec<V1TagItem>,
    /// Total number of tags returned.
    pub total: usize,
}

/// Request body for creating a tag.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1TagCreateRequest {
    /// Tag name.
    pub name: String,
    /// Optional hex color code.
    #[serde(default)]
    pub color: Option<String>,
}

/// Request body for updating a tag.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1TagUpdateRequest {
    /// New tag name (optional).
    #[serde(default)]
    pub name: Option<String>,
    /// New color (optional, pass null to clear).
    #[serde(default)]
    pub color: Option<Option<String>>,
}

/// Request body for attaching tags to content.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ContentTagsAttachRequest {
    /// ID of the tag to attach.
    pub tag_id: String,
}

// =============================================================================
// Groups API Types
// =============================================================================

/// Query parameters for listing groups.
#[derive(Debug, Deserialize, Default, ToSchema)]
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
#[derive(Debug, Serialize, ToSchema)]
pub struct V1GroupItem {
    /// Unique identifier (UUID).
    pub id: String,
    /// Group name.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Group visibility (private, public).
    pub visibility: String,
    /// Group status (active, archived, trashed).
    pub status: String,
    /// Timestamp when group was trashed (ISO 8601 format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<String>,
    /// Timestamp when group was archived (ISO 8601 format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    /// Timestamp when group was created (ISO 8601 format).
    pub created_at: String,
    /// Timestamp when group was last updated (ISO 8601 format).
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
#[derive(Debug, Serialize, ToSchema)]
pub struct V1GroupsListResponse {
    /// List of groups.
    pub items: Vec<V1GroupItem>,
    /// Total number of groups returned.
    pub total: usize,
}

/// Request body for creating a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1GroupCreateRequest {
    /// Group name.
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: Option<String>,
    /// Group visibility (private, public). Default: private.
    #[serde(default = "default_visibility")]
    pub visibility: String,
}

fn default_visibility() -> String {
    "private".to_string()
}

/// Request body for updating a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1GroupUpdateRequest {
    /// New group name (optional).
    #[serde(default)]
    pub name: Option<String>,
    /// New description (optional, pass null to clear).
    #[serde(default)]
    pub description: Option<Option<String>>,
    /// New visibility (optional: private, public).
    #[serde(default)]
    pub visibility: Option<String>,
}

/// Response item for a group content item.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1GroupContentItem {
    /// Unique identifier (UUID).
    pub id: String,
    /// Group ID (UUID).
    pub group_id: String,
    /// Content ID (UUID).
    pub content_id: String,
    /// Sort order within the group.
    pub sort_order: i32,
    /// Timestamp when content was added to group (ISO 8601 format).
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
#[derive(Debug, Serialize, ToSchema)]
pub struct V1GroupContentsListResponse {
    /// List of group content items.
    pub items: Vec<V1GroupContentItem>,
    /// Total number of items returned.
    pub total: usize,
}

/// Request body for adding content to a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1GroupAddContentRequest {
    /// Content ID to add (UUID).
    pub content_id: String,
    /// Optional sort order.
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// Request body for reordering group contents.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1GroupReorderRequest {
    /// List of (content_id, sort_order) pairs.
    pub items: Vec<V1GroupReorderItem>,
}

/// A single item in a group reorder request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1GroupReorderItem {
    /// Content ID (UUID).
    pub content_id: String,
    /// New sort order.
    pub sort_order: i32,
}

// =============================================================================
// Uploads API Types
// =============================================================================

/// Request body for upload initialization.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1UploadsInitRequest {
    /// Original filename.
    pub filename: String,
    /// MIME type of the file.
    pub content_type: String,
    /// File size in bytes.
    pub file_size: u64,
}

/// Response for upload initialization.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1UploadsInitResponse {
    /// Unique upload identifier (UUID).
    pub upload_id: String,
    /// Storage key/path where file will be stored.
    pub storage_key: String,
    /// HTTP method to use for upload.
    pub method: String,
    /// Presigned URL for uploading the file.
    pub upload_url: String,
    /// URL expiration timestamp (ISO 8601 format).
    pub expires_at: String,
}

/// Request body for completing an upload.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1UploadsCompleteRequest {
    /// Upload ID from the init response.
    pub upload_id: String,
    /// Optional title for the content (defaults to filename).
    #[serde(default)]
    pub title: Option<String>,
    /// Optional description for the content.
    #[serde(default)]
    pub description: Option<String>,
}

/// Response for upload completion.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1UploadsCompleteResponse {
    /// The created content item.
    pub content: V1ContentItem,
}

// =============================================================================
// Me API Types
// =============================================================================

/// Response from the `/v1/me` endpoint containing authenticated user information.
#[derive(Debug, Serialize, ToSchema)]
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

/// Parse permission string to SharePermission enum.
pub fn parse_share_permission(s: &str) -> Option<SharePermission> {
    match s.to_lowercase().as_str() {
        "view" => Some(SharePermission::View),
        "download" => Some(SharePermission::Download),
        _ => None,
    }
}

// =============================================================================
// Share Links API Types
// =============================================================================

fn default_view_permission() -> String {
    "view".to_string()
}

/// Request body for creating a share link.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ShareLinkCreateRequest {
    /// Optional friendly name for the share link.
    #[serde(default)]
    pub name: Option<String>,
    /// Permission level: "view" or "download". Default: "view".
    #[serde(default = "default_view_permission")]
    pub permission: String,
    /// Optional password to protect the share link.
    #[serde(default)]
    pub password: Option<String>,
    /// Optional expiration timestamp (ISO 8601 format).
    #[serde(default)]
    pub expires_at: Option<String>,
    /// Optional maximum number of times the link can be accessed.
    #[serde(default)]
    pub max_access_count: Option<i32>,
}

/// Request body for updating a share link.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ShareLinkUpdateRequest {
    /// New name (optional, pass null to clear).
    #[serde(default)]
    pub name: Option<Option<String>>,
    /// New permission level: "view" or "download".
    #[serde(default)]
    pub permission: Option<String>,
    /// New password (optional, empty string removes password).
    #[serde(default)]
    pub password: Option<String>,
    /// New expiration (optional, pass null to clear).
    #[serde(default)]
    pub expires_at: Option<Option<String>>,
    /// New max access count (optional, pass null to clear).
    #[serde(default)]
    pub max_access_count: Option<Option<i32>>,
    /// Whether the link is active.
    #[serde(default)]
    pub is_active: Option<bool>,
}

/// A share link in API responses.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ShareLinkResponse {
    /// Unique identifier (UUID).
    pub id: String,
    /// Unique share token for URLs.
    pub token: String,
    /// Optional friendly name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Permission level: "view" or "download".
    pub permission: String,
    /// Whether the link is password protected.
    pub has_password: bool,
    /// Expiration timestamp (ISO 8601 format).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    /// Maximum number of accesses allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_access_count: Option<i32>,
    /// Current access count.
    pub access_count: i32,
    /// Whether the link is currently active.
    pub is_active: bool,
    /// Timestamp when the link was created (ISO 8601 format).
    pub created_at: String,
    /// Full shareable URL.
    pub share_url: String,
}

impl V1ShareLinkResponse {
    /// Create a response from a ShareLinkRow with a base URL for constructing share URLs.
    pub fn from_row(row: ShareLinkRow, base_url: &str) -> Self {
        Self {
            id: row.id.to_string(),
            token: row.token.clone(),
            name: row.name,
            permission: row.permission,
            has_password: row.password_hash.is_some(),
            expires_at: row.expires_at.map(|t| t.to_rfc3339()),
            max_access_count: row.max_access_count,
            access_count: row.access_count,
            is_active: row.is_active,
            created_at: row.created_at.to_rfc3339(),
            share_url: format!("{}/s/{}", base_url, row.token),
        }
    }
}

/// Response for listing share links.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1ShareLinksListResponse {
    /// List of share links.
    pub share_links: Vec<V1ShareLinkResponse>,
}

/// Request for creating a share link attached to a content item.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1ContentShareLinkCreateRequest {
    /// Optional friendly name for the share link.
    #[serde(default)]
    pub name: Option<String>,
    /// Permission level: "view" or "download". Default: "view".
    #[serde(default = "default_view_permission")]
    pub permission: String,
    /// Optional password to protect the share link.
    #[serde(default)]
    pub password: Option<String>,
    /// Optional expiration timestamp (ISO 8601 format).
    #[serde(default)]
    pub expires_at: Option<String>,
    /// Optional maximum number of times the link can be accessed.
    #[serde(default)]
    pub max_access_count: Option<i32>,
}

/// Request for creating a share link attached to a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1GroupShareLinkCreateRequest {
    /// Optional friendly name for the share link.
    #[serde(default)]
    pub name: Option<String>,
    /// Permission level: "view" or "download". Default: "view".
    #[serde(default = "default_view_permission")]
    pub permission: String,
    /// Optional password to protect the share link.
    #[serde(default)]
    pub password: Option<String>,
    /// Optional expiration timestamp (ISO 8601 format).
    #[serde(default)]
    pub expires_at: Option<String>,
    /// Optional maximum number of times the link can be accessed.
    #[serde(default)]
    pub max_access_count: Option<i32>,
}

// =============================================================================
// Public Access API Types
// =============================================================================

/// Request body with password for accessing a protected share link.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1SharePasswordRequest {
    /// Password for the share link.
    pub password: String,
}

/// Response for public share link metadata.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1PublicShareResponse {
    /// Type of shared content: "content" or "group".
    pub content_type: String,
    /// Title of the content or group.
    pub title: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Permission level: "view" or "download".
    pub permission: String,
    /// Number of files in the group (only for groups).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<i64>,
    /// Whether this share link requires a password.
    pub requires_password: bool,
}

/// Response containing a presigned view URL.
#[derive(Debug, Serialize, ToSchema)]
pub struct V1PublicViewUrlResponse {
    /// Presigned URL for viewing/downloading content.
    pub url: String,
    /// URL expiration timestamp (ISO 8601 format).
    pub expires_at: String,
}

/// Request for getting a view URL from a public share.
#[derive(Debug, Deserialize, ToSchema)]
pub struct V1PublicViewUrlRequest {
    /// Password if the share link is protected.
    #[serde(default)]
    pub password: Option<String>,
    /// Content disposition: "inline" or "attachment".
    #[serde(default = "default_inline_disposition")]
    pub disposition: String,
}

fn default_inline_disposition() -> String {
    "inline".to_string()
}
