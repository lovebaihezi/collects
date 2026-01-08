use crate::config::Config;
use crate::database::{
    ContentGroupItemRow, ContentGroupRow, ContentRow, ContentStatus, ContentsListParams,
    ContentsUpdate, GroupCreate, GroupStatus, GroupUpdate, GroupsListParams, SqlStorage,
    SqlStorageError, Visibility,
};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{
    Json, Router,
    extract::{Extension, Path, Query, Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{any, delete, get, patch, post},
};
use collects_utils::version_info::{RuntimeEnv, format_version_for_runtime_env};
use opentelemetry::{global, propagation::Extractor};
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod auth;
pub mod collect_files;
pub mod collects;
pub mod config;
pub mod database;
pub mod internal;
pub mod storage;
pub mod telemetry;
pub mod users;

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Creates routes with both SQL storage and User storage support.
///
/// This is the preferred method for creating routes as it supports
/// full user storage functionality including persistence.
pub async fn routes<S, U>(sql_storage: S, user_storage: U, config: Config) -> Router
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    let state = AppState::new(sql_storage, user_storage);

    // Build the protected internal routes with Zero Trust middleware if configured
    let internal_routes = internal::create_internal_routes::<S, U>(&config);

    // Minimal MVP v1 route group (stub implementations)
    let v1_routes = Router::new()
        .route("/me", get(v1_me::<S, U>))
        .route("/uploads/init", post(v1_uploads_init::<S, U>))
        // Contents endpoints
        .route("/contents", get(v1_contents_list::<S, U>))
        .route("/contents/{id}", get(v1_contents_get::<S, U>))
        .route("/contents/{id}", patch(v1_contents_update::<S, U>))
        .route("/contents/{id}/trash", post(v1_contents_trash::<S, U>))
        .route("/contents/{id}/restore", post(v1_contents_restore::<S, U>))
        .route("/contents/{id}/archive", post(v1_contents_archive::<S, U>))
        .route(
            "/contents/{id}/unarchive",
            post(v1_contents_unarchive::<S, U>),
        )
        .route(
            "/contents/{id}/view-url",
            post(v1_contents_view_url::<S, U>),
        )
        // Content-Tags endpoints
        .route(
            "/contents/{id}/tags",
            get(v1_content_tags_list::<S, U>).post(v1_content_tags_attach::<S, U>),
        )
        .route(
            "/contents/{id}/tags/{tag_id}",
            delete(v1_content_tags_detach::<S, U>),
        )
        // Tags endpoints
        .route(
            "/tags",
            get(v1_tags_list::<S, U>).post(v1_tags_create::<S, U>),
        )
        .route(
            "/tags/{id}",
            patch(v1_tags_update::<S, U>).delete(v1_tags_delete::<S, U>),
        )
        // Groups endpoints
        .route(
            "/groups",
            get(v1_groups_list::<S, U>).post(v1_groups_create::<S, U>),
        )
        .route(
            "/groups/{id}",
            get(v1_groups_get::<S, U>).patch(v1_groups_update::<S, U>),
        )
        .route("/groups/{id}/trash", post(v1_groups_trash::<S, U>))
        .route("/groups/{id}/restore", post(v1_groups_restore::<S, U>))
        .route("/groups/{id}/archive", post(v1_groups_archive::<S, U>))
        .route("/groups/{id}/unarchive", post(v1_groups_unarchive::<S, U>))
        .route(
            "/groups/{id}/contents",
            get(v1_group_contents_list::<S, U>).post(v1_group_contents_add::<S, U>),
        )
        .route(
            "/groups/{id}/contents/{content_id}",
            delete(v1_group_contents_remove::<S, U>),
        );

    Router::new()
        .route("/is-health", get(health_check::<S, U>))
        .nest("/v1", v1_routes)
        .nest("/internal", internal_routes)
        .nest("/auth", users::auth_routes::<S, U>())
        .fallback(any(catch_all))
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                // Check if the request has a trace context header
                let parent_context = global::get_text_map_propagator(|propagator| {
                    propagator.extract(&HeaderExtractor(request.headers()))
                });

                // Create a span for this request
                let span = tracing::info_span!(
                    "http_request",
                    http_request.method = ?request.method(),
                    http_request.uri = ?request.uri(),
                    http_request.version = ?request.version(),
                    http_request.user_agent = ?request.headers().get(axum::http::header::USER_AGENT),
                    otp_trace_id = tracing::field::Empty, // Placeholder for debugging
                );

                // Set the parent context for the span
                span.set_parent(parent_context);

                span
            }),
        )
        .layer(Extension(config))
        .with_state(state)
}

async fn health_check<S, U>(
    State(state): State<AppState<S, U>>,
    Extension(config): Extension<Config>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    let mut response = if state.sql_storage.is_connected().await {
        (StatusCode::OK, "OK").into_response()
    } else {
        (StatusCode::BAD_GATEWAY, "502").into_response()
    };

    let env_value = config.environment().to_string();
    response.headers_mut().insert(
        HeaderName::from_static("x-service-env"),
        HeaderValue::from_str(&env_value).expect("environment header is valid ASCII"),
    );

    let runtime_env: RuntimeEnv = config.environment().into();
    let version_value = format_version_for_runtime_env(runtime_env);
    response.headers_mut().insert(
        HeaderName::from_static("x-service-version"),
        HeaderValue::from_str(&version_value).expect("version header is valid ASCII"),
    );

    response
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}

/// Response from the `/v1/me` endpoint containing authenticated user information.
#[derive(Debug, Serialize)]
struct V1MeResponse {
    /// The authenticated user's username.
    username: String,
    /// Token issued-at timestamp (Unix seconds).
    issued_at: i64,
    /// Token expiration timestamp (Unix seconds).
    expires_at: i64,
}

/// Get the current authenticated user's information.
///
/// This endpoint requires a valid session JWT token in the Authorization header.
///
/// # Request
///
/// ```text
/// GET /v1/me
/// Authorization: Bearer <session_token>
/// ```
///
/// # Response
///
/// ```json
/// {
///     "username": "alice",
///     "issued_at": 1704067200,
///     "expires_at": 1704153600
/// }
/// ```
///
/// # Errors
///
/// - 401 Unauthorized: Missing or invalid token
async fn v1_me<S, U>(State(_state): State<AppState<S, U>>, auth: RequireAuth) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    (
        StatusCode::OK,
        Json(V1MeResponse {
            username: auth.username().to_string(),
            issued_at: auth.issued_at(),
            expires_at: auth.expires_at(),
        }),
    )
}

#[derive(Debug, Deserialize)]
struct V1UploadsInitRequest {
    filename: String,
    content_type: String,
    file_size: u64,
}

#[derive(Debug, Serialize)]
struct V1UploadsInitResponse {
    upload_id: String,
    storage_key: String,
    method: String,
    upload_url: String,
    expires_at: String,
}

async fn v1_uploads_init<S, U>(
    State(_state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(payload): Json<V1UploadsInitRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Use request fields to avoid dead-code warnings while this is still a stub.
    let _ = (&payload.content_type, payload.file_size, auth.username());

    let storage_key = format!("uploads/{}", payload.filename);

    (
        StatusCode::CREATED,
        Json(V1UploadsInitResponse {
            upload_id: "00000000-0000-0000-0000-000000000000".to_string(),
            storage_key,
            method: "put".to_string(),
            upload_url: "https://example.invalid/upload".to_string(),
            expires_at: "1970-01-01T00:00:00Z".to_string(),
        }),
    )
}

#[derive(Debug, Deserialize)]
struct V1ViewUrlRequest {
    disposition: String,
}

#[derive(Debug, Serialize)]
struct V1ViewUrlResponse {
    url: String,
    expires_at: String,
}

async fn v1_contents_view_url<S, U>(
    State(_state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(_id): Path<String>,
    Json(payload): Json<V1ViewUrlRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Use request fields to avoid dead-code warnings while this is still a stub.
    let _ = (payload.disposition, auth.username());

    (
        StatusCode::OK,
        Json(V1ViewUrlResponse {
            url: "https://example.invalid/view".to_string(),
            expires_at: "1970-01-01T00:00:00Z".to_string(),
        }),
    )
}

// =============================================================================
// Contents API Types
// =============================================================================

/// Query parameters for listing contents.
#[derive(Debug, Deserialize, Default)]
struct V1ContentsListQuery {
    /// Maximum number of results to return (default: 50, max: 100)
    #[serde(default)]
    limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    offset: Option<i64>,
    /// Filter by status: active, archived, trashed
    #[serde(default)]
    status: Option<String>,
}

/// A content item in API responses.
#[derive(Debug, Serialize)]
struct V1ContentItem {
    id: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    storage_backend: String,
    storage_profile: String,
    storage_key: String,
    content_type: String,
    file_size: i64,
    status: String,
    visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trashed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
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
struct V1ContentsListResponse {
    items: Vec<V1ContentItem>,
    total: usize,
}

/// Request body for updating content metadata.
#[derive(Debug, Deserialize)]
struct V1ContentsUpdateRequest {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    description: Option<Option<String>>,
    #[serde(default)]
    visibility: Option<String>,
}

// =============================================================================
// Tags API Types
// =============================================================================

/// A tag item in API responses.
#[derive(Debug, Serialize)]
struct V1TagItem {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    created_at: String,
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
struct V1TagsListResponse {
    items: Vec<V1TagItem>,
    total: usize,
}

/// Request body for creating a tag.
#[derive(Debug, Deserialize)]
struct V1TagCreateRequest {
    name: String,
    #[serde(default)]
    color: Option<String>,
}

/// Request body for updating a tag.
#[derive(Debug, Deserialize)]
struct V1TagUpdateRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    color: Option<Option<String>>,
}

/// Request body for attaching tags to content.
#[derive(Debug, Deserialize)]
struct V1ContentTagsAttachRequest {
    tag_id: String,
}

/// Generic error response.
#[derive(Debug, Serialize)]
struct V1ErrorResponse {
    error: String,
    message: String,
}

impl V1ErrorResponse {
    fn not_found(message: impl Into<String>) -> Self {
        Self {
            error: "not_found".to_string(),
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            error: "bad_request".to_string(),
            message: message.into(),
        }
    }

    fn internal_error(message: impl Into<String>) -> Self {
        Self {
            error: "internal_error".to_string(),
            message: message.into(),
        }
    }
}

// =============================================================================
// Groups API Types
// =============================================================================

/// Query parameters for listing groups.
#[derive(Debug, Deserialize, Default)]
struct V1GroupsListQuery {
    /// Maximum number of results to return (default: 50, max: 100)
    #[serde(default)]
    limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    offset: Option<i64>,
    /// Filter by status: active, archived, trashed
    #[serde(default)]
    status: Option<String>,
}

/// A group item in API responses.
#[derive(Debug, Serialize)]
struct V1GroupItem {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    visibility: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    trashed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl From<ContentGroupRow> for V1GroupItem {
    fn from(row: ContentGroupRow) -> Self {
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
struct V1GroupsListResponse {
    items: Vec<V1GroupItem>,
    total: usize,
}

/// Request body for creating a group.
#[derive(Debug, Deserialize)]
struct V1GroupCreateRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    visibility: Option<String>,
}

/// Request body for updating a group.
#[derive(Debug, Deserialize)]
struct V1GroupUpdateRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<Option<String>>,
    #[serde(default)]
    visibility: Option<String>,
}

/// Request body for adding content to a group.
#[derive(Debug, Deserialize)]
struct V1GroupContentsAddRequest {
    content_id: String,
    #[serde(default)]
    sort_order: Option<i32>,
}

/// A group content item in API responses.
#[derive(Debug, Serialize)]
struct V1GroupContentItem {
    id: String,
    group_id: String,
    content_id: String,
    sort_order: i32,
    added_at: String,
}

impl From<ContentGroupItemRow> for V1GroupContentItem {
    fn from(row: ContentGroupItemRow) -> Self {
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
struct V1GroupContentsListResponse {
    items: Vec<V1GroupContentItem>,
    total: usize,
}

// =============================================================================
// Contents API Handlers
// =============================================================================

/// List contents for the authenticated user.
///
/// GET /v1/contents?limit=50&offset=0&status=active
async fn v1_contents_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Query(query): Query<V1ContentsListQuery>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse status filter
    let status = query.status.as_deref().and_then(|s| match s {
        "active" => Some(ContentStatus::Active),
        "archived" => Some(ContentStatus::Archived),
        "trashed" => Some(ContentStatus::Trashed),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let params = ContentsListParams {
        limit,
        offset,
        status,
    };

    match state
        .sql_storage
        .contents_list_for_user(user.id, params)
        .await
    {
        Ok(rows) => {
            let items: Vec<V1ContentItem> = rows.into_iter().map(V1ContentItem::from).collect();
            let total = items.len();
            (
                StatusCode::OK,
                Json(V1ContentsListResponse { items, total }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list contents: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list contents")),
            )
                .into_response()
        }
    }
}

/// Get a specific content item by ID.
///
/// GET /v1/contents/:id
async fn v1_contents_get<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            // Verify ownership
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(V1ContentItem::from(row))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response()
        }
    }
}

/// Update content metadata.
///
/// PATCH /v1/contents/:id
async fn v1_contents_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1ContentsUpdateRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Parse visibility if provided
    let visibility = match payload.visibility.as_deref() {
        Some("private") => Some(Visibility::Private),
        Some("public") => Some(Visibility::Public),
        Some("restricted") => Some(Visibility::Restricted),
        Some(v) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid visibility: {}",
                    v
                ))),
            )
                .into_response();
        }
        None => None,
    };

    let changes = ContentsUpdate {
        title: payload.title,
        description: payload.description,
        visibility,
    };

    match state
        .sql_storage
        .contents_update_metadata(content_id, user.id, changes)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1ContentItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse {
                error: "forbidden".to_string(),
                message: "You do not have permission to update this content".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update content: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to update content")),
            )
                .into_response()
        }
    }
}

/// Move content to trash.
///
/// POST /v1/contents/:id/trash
async fn v1_contents_trash<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Trashed).await
}

/// Restore content from trash.
///
/// POST /v1/contents/:id/restore
async fn v1_contents_restore<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Active).await
}

/// Archive content.
///
/// POST /v1/contents/:id/archive
async fn v1_contents_archive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Archived).await
}

/// Unarchive content (restore to active).
///
/// POST /v1/contents/:id/unarchive
async fn v1_contents_unarchive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Active).await
}

/// Helper function to set content status.
async fn v1_contents_set_status<S, U>(
    state: AppState<S, U>,
    auth: RequireAuth,
    id: String,
    new_status: ContentStatus,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    let now = chrono::Utc::now();

    match state
        .sql_storage
        .contents_set_status(content_id, user.id, new_status, now)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1ContentItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse {
                error: "forbidden".to_string(),
                message: "You do not have permission to modify this content".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to set content status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to update content status",
                )),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Tags API Handlers
// =============================================================================

/// List tags for the authenticated user.
///
/// GET /v1/tags
async fn v1_tags_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    match state.sql_storage.tags_list_for_user(user.id).await {
        Ok(rows) => {
            let items: Vec<V1TagItem> = rows.into_iter().map(V1TagItem::from).collect();
            let total = items.len();
            (StatusCode::OK, Json(V1TagsListResponse { items, total })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list tags: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list tags")),
            )
                .into_response()
        }
    }
}

/// Create a new tag.
///
/// POST /v1/tags
async fn v1_tags_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(request): Json<V1TagCreateRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Validate tag name
    let name = request.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Tag name cannot be empty")),
        )
            .into_response();
    }

    let input = database::TagCreate {
        user_id: user.id,
        name: name.to_string(),
        color: request.color,
    };

    match state.sql_storage.tags_create(input).await {
        Ok(row) => (StatusCode::CREATED, Json(V1TagItem::from(row))).into_response(),
        Err(SqlStorageError::Conflict) => (
            StatusCode::CONFLICT,
            Json(V1ErrorResponse {
                error: "conflict".to_string(),
                message: "A tag with this name already exists".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to create tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to create tag")),
            )
                .into_response()
        }
    }
}

/// Update a tag.
///
/// PATCH /v1/tags/:id
async fn v1_tags_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(request): Json<V1TagUpdateRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    // Validate name if provided
    if let Some(ref name) = request.name
        && name.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Tag name cannot be empty")),
        )
            .into_response();
    }

    let input = database::TagUpdate {
        name: request.name.map(|n| n.trim().to_string()),
        color: request.color,
    };

    match state.sql_storage.tags_update(user.id, tag_id, input).await {
        Ok(Some(row)) => (StatusCode::OK, Json(V1TagItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not found")),
        )
            .into_response(),
        Err(SqlStorageError::Conflict) => (
            StatusCode::CONFLICT,
            Json(V1ErrorResponse {
                error: "conflict".to_string(),
                message: "A tag with this name already exists".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to update tag")),
            )
                .into_response()
        }
    }
}

/// Delete a tag.
///
/// DELETE /v1/tags/:id
async fn v1_tags_delete<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    match state.sql_storage.tags_delete(user.id, tag_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to delete tag")),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Content-Tags API Handlers
// =============================================================================

/// List tags attached to a content item.
///
/// GET /v1/contents/:id/tags
async fn v1_content_tags_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Content not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response();
        }
    }

    match state
        .sql_storage
        .content_tags_list_for_content(content_id)
        .await
    {
        Ok(rows) => {
            let items: Vec<V1TagItem> = rows.into_iter().map(V1TagItem::from).collect();
            let total = items.len();
            (StatusCode::OK, Json(V1TagsListResponse { items, total })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list content tags: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to list content tags",
                )),
            )
                .into_response()
        }
    }
}

/// Attach a tag to a content item.
///
/// POST /v1/contents/:id/tags
async fn v1_content_tags_attach<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(request): Json<V1ContentTagsAttachRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&request.tag_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Content not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response();
        }
    }

    match state
        .sql_storage
        .content_tags_attach(content_id, tag_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to attach tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to attach tag")),
            )
                .into_response()
        }
    }
}

/// Detach a tag from a content item.
///
/// DELETE /v1/contents/:id/tags/:tag_id
async fn v1_content_tags_detach<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path((id, tag_id_str)): Path<(String, String)>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&tag_id_str) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Content not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response();
        }
    }

    match state
        .sql_storage
        .content_tags_detach(content_id, tag_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not attached to content")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to detach tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to detach tag")),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Groups API Handlers
// =============================================================================

/// List groups for the authenticated user.
///
/// GET /v1/groups
async fn v1_groups_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Query(query): Query<V1GroupsListQuery>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse status filter
    let status = query.status.as_deref().and_then(|s| match s {
        "active" => Some(GroupStatus::Active),
        "archived" => Some(GroupStatus::Archived),
        "trashed" => Some(GroupStatus::Trashed),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let params = GroupsListParams {
        limit,
        offset,
        status,
    };

    match state
        .sql_storage
        .groups_list_for_user(user.id, params)
        .await
    {
        Ok(rows) => {
            let items: Vec<V1GroupItem> = rows.into_iter().map(V1GroupItem::from).collect();
            let total = items.len();
            (StatusCode::OK, Json(V1GroupsListResponse { items, total })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list groups: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list groups")),
            )
                .into_response()
        }
    }
}

/// Create a new group.
///
/// POST /v1/groups
async fn v1_groups_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(payload): Json<V1GroupCreateRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Validate name
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Name cannot be empty")),
        )
            .into_response();
    }

    // Parse visibility
    let visibility = match payload.visibility.as_deref() {
        Some("private") | None => Visibility::Private,
        Some("public") => Visibility::Public,
        Some("restricted") => Visibility::Restricted,
        Some(v) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid visibility: {}",
                    v
                ))),
            )
                .into_response();
        }
    };

    let input = GroupCreate {
        user_id: user.id,
        name: payload.name,
        description: payload.description,
        visibility,
    };

    match state.sql_storage.groups_create(input).await {
        Ok(row) => (StatusCode::CREATED, Json(V1GroupItem::from(row))).into_response(),
        Err(e) => {
            tracing::error!("Failed to create group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to create group")),
            )
                .into_response()
        }
    }
}

/// Get a group by ID.
///
/// GET /v1/groups/:id
async fn v1_groups_get<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            // Verify user owns the group
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(V1GroupItem::from(row))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response()
        }
    }
}

/// Update a group's metadata.
///
/// PATCH /v1/groups/:id
async fn v1_groups_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1GroupUpdateRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Parse visibility if provided
    let visibility = match payload.visibility.as_deref() {
        Some("private") => Some(Visibility::Private),
        Some("public") => Some(Visibility::Public),
        Some("restricted") => Some(Visibility::Restricted),
        Some(v) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid visibility: {}",
                    v
                ))),
            )
                .into_response();
        }
        None => None,
    };

    let changes = GroupUpdate {
        name: payload.name,
        description: payload.description,
        visibility,
    };

    match state
        .sql_storage
        .groups_update_metadata(group_id, user.id, changes)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1GroupItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse {
                error: "forbidden".to_string(),
                message: "You do not have permission to update this group".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to update group")),
            )
                .into_response()
        }
    }
}

/// Trash a group.
///
/// POST /v1/groups/:id/trash
async fn v1_groups_trash<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_groups_set_status(state, auth, id, GroupStatus::Trashed).await
}

/// Restore a group from trash.
///
/// POST /v1/groups/:id/restore
async fn v1_groups_restore<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_groups_set_status(state, auth, id, GroupStatus::Active).await
}

/// Archive a group.
///
/// POST /v1/groups/:id/archive
async fn v1_groups_archive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_groups_set_status(state, auth, id, GroupStatus::Archived).await
}

/// Unarchive a group.
///
/// POST /v1/groups/:id/unarchive
async fn v1_groups_unarchive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_groups_set_status(state, auth, id, GroupStatus::Active).await
}

/// Helper function to set group status.
async fn v1_groups_set_status<S, U>(
    state: AppState<S, U>,
    auth: RequireAuth,
    id: String,
    new_status: GroupStatus,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    let now = chrono::Utc::now();

    match state
        .sql_storage
        .groups_set_status(group_id, user.id, new_status, now)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1GroupItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse {
                error: "forbidden".to_string(),
                message: "You do not have permission to modify this group".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to set group status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to update group status",
                )),
            )
                .into_response()
        }
    }
}

/// List contents in a group.
///
/// GET /v1/groups/:id/contents
async fn v1_group_contents_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the group
    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Group not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response();
        }
    }

    match state.sql_storage.group_items_list(group_id).await {
        Ok(rows) => {
            let items: Vec<V1GroupContentItem> =
                rows.into_iter().map(V1GroupContentItem::from).collect();
            let total = items.len();
            (
                StatusCode::OK,
                Json(V1GroupContentsListResponse { items, total }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list group contents: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to list group contents",
                )),
            )
                .into_response()
        }
    }
}

/// Add content to a group.
///
/// POST /v1/groups/:id/contents
async fn v1_group_contents_add<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1GroupContentsAddRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&payload.content_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the group
    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Group not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response();
        }
    }

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Content not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response();
        }
    }

    let sort_order = payload.sort_order.unwrap_or(0);

    match state
        .sql_storage
        .group_items_add(group_id, content_id, sort_order)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to add content to group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to add content to group",
                )),
            )
                .into_response()
        }
    }
}

/// Remove content from a group.
///
/// DELETE /v1/groups/:id/contents/:content_id
async fn v1_group_contents_remove<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path((id, content_id_str)): Path<(String, String)>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&content_id_str) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the group
    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Group not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response();
        }
    }

    match state
        .sql_storage
        .group_items_remove(group_id, content_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not in group")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to remove content from group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to remove content from group",
                )),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::storage::MockUserStorage;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[derive(Clone)]
    struct MockSqlStorage {
        is_connected: bool,
    }

    impl SqlStorage for MockSqlStorage {
        async fn is_connected(&self) -> bool {
            self.is_connected
        }

        async fn contents_insert(
            &self,
            _input: crate::database::ContentsInsert,
        ) -> Result<crate::database::ContentRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.contents_insert: unimplemented".to_string(),
            ))
        }

        async fn contents_get(
            &self,
            _id: uuid::Uuid,
        ) -> Result<Option<crate::database::ContentRow>, crate::database::SqlStorageError> {
            Ok(None)
        }

        async fn contents_list_for_user(
            &self,
            _user_id: uuid::Uuid,
            _params: crate::database::ContentsListParams,
        ) -> Result<Vec<crate::database::ContentRow>, crate::database::SqlStorageError> {
            Ok(vec![])
        }

        async fn contents_update_metadata(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _changes: crate::database::ContentsUpdate,
        ) -> Result<Option<crate::database::ContentRow>, crate::database::SqlStorageError> {
            Ok(None)
        }

        async fn contents_set_status(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _new_status: crate::database::ContentStatus,
            _now: chrono::DateTime<chrono::Utc>,
        ) -> Result<Option<crate::database::ContentRow>, crate::database::SqlStorageError> {
            Ok(None)
        }

        async fn groups_create(
            &self,
            _input: crate::database::GroupCreate,
        ) -> Result<crate::database::ContentGroupRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.groups_create: unimplemented".to_string(),
            ))
        }

        async fn groups_get(
            &self,
            _id: uuid::Uuid,
        ) -> Result<Option<crate::database::ContentGroupRow>, crate::database::SqlStorageError>
        {
            Ok(None)
        }

        async fn groups_list_for_user(
            &self,
            _user_id: uuid::Uuid,
            _params: crate::database::GroupsListParams,
        ) -> Result<Vec<crate::database::ContentGroupRow>, crate::database::SqlStorageError>
        {
            Ok(vec![])
        }

        async fn groups_update_metadata(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _changes: crate::database::GroupUpdate,
        ) -> Result<Option<crate::database::ContentGroupRow>, crate::database::SqlStorageError>
        {
            Ok(None)
        }

        async fn groups_set_status(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _new_status: crate::database::GroupStatus,
            _now: chrono::DateTime<chrono::Utc>,
        ) -> Result<Option<crate::database::ContentGroupRow>, crate::database::SqlStorageError>
        {
            Ok(None)
        }

        async fn group_items_add(
            &self,
            _group_id: uuid::Uuid,
            _content_id: uuid::Uuid,
            _sort_order: i32,
        ) -> Result<(), crate::database::SqlStorageError> {
            Ok(())
        }

        async fn group_items_remove(
            &self,
            _group_id: uuid::Uuid,
            _content_id: uuid::Uuid,
        ) -> Result<bool, crate::database::SqlStorageError> {
            Ok(false)
        }

        async fn group_items_list(
            &self,
            _group_id: uuid::Uuid,
        ) -> Result<Vec<crate::database::ContentGroupItemRow>, crate::database::SqlStorageError>
        {
            Ok(vec![])
        }

        async fn tags_create(
            &self,
            _input: crate::database::TagCreate,
        ) -> Result<crate::database::TagRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.tags_create: unimplemented".to_string(),
            ))
        }

        async fn tags_list_for_user(
            &self,
            _user_id: uuid::Uuid,
        ) -> Result<Vec<crate::database::TagRow>, crate::database::SqlStorageError> {
            Ok(vec![])
        }

        async fn tags_delete(
            &self,
            _user_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
        ) -> Result<bool, crate::database::SqlStorageError> {
            Ok(false)
        }

        async fn tags_update(
            &self,
            _user_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
            _input: crate::database::TagUpdate,
        ) -> Result<Option<crate::database::TagRow>, crate::database::SqlStorageError> {
            Ok(None)
        }

        async fn content_tags_attach(
            &self,
            _content_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
        ) -> Result<(), crate::database::SqlStorageError> {
            Ok(())
        }

        async fn content_tags_detach(
            &self,
            _content_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
        ) -> Result<bool, crate::database::SqlStorageError> {
            Ok(false)
        }

        async fn content_tags_list_for_content(
            &self,
            _content_id: uuid::Uuid,
        ) -> Result<Vec<crate::database::TagRow>, crate::database::SqlStorageError> {
            Ok(vec![])
        }

        async fn share_links_create(
            &self,
            _input: crate::database::ShareLinkCreate,
        ) -> Result<crate::database::ShareLinkRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.share_links_create: unimplemented".to_string(),
            ))
        }

        async fn share_links_get_by_token(
            &self,
            _token: &str,
        ) -> Result<Option<crate::database::ShareLinkRow>, crate::database::SqlStorageError>
        {
            Ok(None)
        }

        async fn share_links_list_for_owner(
            &self,
            _owner_id: uuid::Uuid,
        ) -> Result<Vec<crate::database::ShareLinkRow>, crate::database::SqlStorageError> {
            Ok(vec![])
        }

        async fn share_links_deactivate(
            &self,
            _owner_id: uuid::Uuid,
            _share_link_id: uuid::Uuid,
        ) -> Result<bool, crate::database::SqlStorageError> {
            Ok(false)
        }

        async fn content_shares_create_for_user(
            &self,
            _input: crate::database::ContentShareCreateForUser,
        ) -> Result<crate::database::ContentShareRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.content_shares_create_for_user: unimplemented".to_string(),
            ))
        }

        async fn content_shares_create_for_link(
            &self,
            _input: crate::database::ContentShareCreateForLink,
        ) -> Result<crate::database::ContentShareRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.content_shares_create_for_link: unimplemented".to_string(),
            ))
        }

        async fn group_shares_create_for_user(
            &self,
            _input: crate::database::GroupShareCreateForUser,
        ) -> Result<crate::database::ContentGroupShareRow, crate::database::SqlStorageError>
        {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.group_shares_create_for_user: unimplemented".to_string(),
            ))
        }

        async fn group_shares_create_for_link(
            &self,
            _input: crate::database::GroupShareCreateForLink,
        ) -> Result<crate::database::ContentGroupShareRow, crate::database::SqlStorageError>
        {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.group_shares_create_for_link: unimplemented".to_string(),
            ))
        }

        async fn otp_record_attempt(
            &self,
            _input: crate::database::OtpAttemptRecord,
        ) -> Result<(), crate::database::SqlStorageError> {
            // Mock: silently succeed
            Ok(())
        }

        async fn otp_is_rate_limited(
            &self,
            _username: &str,
            _ip_address: Option<std::net::IpAddr>,
            _config: &crate::database::OtpRateLimitConfig,
        ) -> Result<bool, crate::database::SqlStorageError> {
            // Mock: never rate limited
            Ok(false)
        }
    }

    #[tokio::test]
    async fn test_health_check_connected() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/is-health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    // Helper to generate a valid test token
    fn generate_test_token() -> String {
        crate::users::otp::generate_session_token(
            "testuser",
            "test-jwt-secret-key-for-local-development",
        )
        .unwrap()
    }

    // MVP v1 API: Protected endpoints require Bearer token authentication.

    #[tokio::test]
    async fn test_v1_me_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Without auth token, should return 401 Unauthorized
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_me_with_valid_auth() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Parse response body and verify username
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["username"], "testuser");
    }

    #[tokio::test]
    async fn test_v1_uploads_init_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/uploads/init")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"filename":"photo.jpg","content_type":"image/jpeg","file_size":1234}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Without auth token, should return 401 Unauthorized
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_uploads_init_with_valid_auth() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/uploads/init")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from(
                        r#"{"filename":"photo.jpg","content_type":"image/jpeg","file_size":1234}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_v1_contents_view_url_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000000/view-url")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"disposition":"inline"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Without auth token, should return 401 Unauthorized
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_view_url_with_valid_auth() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000000/view-url")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from(r#"{"disposition":"inline"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_v1_me_with_invalid_token_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header("Authorization", "Bearer invalid-token-that-is-not-a-jwt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Invalid token should return 401 Unauthorized
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Verify the error response contains expected fields
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid_token");
    }

    #[tokio::test]
    async fn test_v1_me_with_wrong_secret_token_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        // Generate token with a different secret
        let token =
            crate::users::otp::generate_session_token("testuser", "different-secret").unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/me")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // Token signed with wrong secret should return 401
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid_token");
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Invalid token signature")
        );
    }

    #[tokio::test]
    async fn test_health_check_includes_headers() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/is-health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let env_header = response
            .headers()
            .get("x-service-env")
            .and_then(|v| v.to_str().ok());
        assert_eq!(env_header, Some("local"));

        let version_header = response
            .headers()
            .get("x-service-version")
            .and_then(|v| v.to_str().ok());
        // Local environment uses "main:{commit}" format - using shared function
        let expected_version = format_version_for_runtime_env(RuntimeEnv::Local);
        assert_eq!(version_header, Some(expected_version.as_str()));
    }

    #[tokio::test]
    async fn test_health_check_disconnected() {
        let sql_storage = MockSqlStorage {
            is_connected: false,
        };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/is-health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn test_env_to_runtime_env_conversion() {
        // Test that all Env variants convert correctly to RuntimeEnv
        assert_eq!(RuntimeEnv::from(&config::Env::Local), RuntimeEnv::Local);
        assert_eq!(RuntimeEnv::from(&config::Env::Prod), RuntimeEnv::Prod);
        assert_eq!(
            RuntimeEnv::from(&config::Env::Internal),
            RuntimeEnv::Internal
        );
        assert_eq!(RuntimeEnv::from(&config::Env::Test), RuntimeEnv::Test);
        assert_eq!(
            RuntimeEnv::from(&config::Env::TestInternal),
            RuntimeEnv::TestInternal
        );
        assert_eq!(RuntimeEnv::from(&config::Env::Pr), RuntimeEnv::Pr);
        assert_eq!(RuntimeEnv::from(&config::Env::Nightly), RuntimeEnv::Nightly);
    }

    // =========================================================================
    // Contents API Tests
    // =========================================================================

    #[tokio::test]
    async fn test_v1_contents_list_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_list_with_valid_auth() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["items"].is_array());
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn test_v1_contents_list_with_query_params() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents?limit=10&offset=5&status=active")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_v1_contents_get_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_get_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_contents_get_invalid_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents/not-a-uuid")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_contents_update_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title": "New Title"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_update_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"title": "New Title"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_contents_update_invalid_visibility() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"visibility": "invalid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_contents_trash_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/trash")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_trash_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/trash")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_contents_restore_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/restore")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_archive_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/archive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_unarchive_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/unarchive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_list_user_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        // User storage is empty, so "testuser" from the token won't be found
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // User not found in storage returns 401
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // =========================================================================
    // Tags API Tests
    // =========================================================================

    #[tokio::test]
    async fn test_v1_tags_list_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tags")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_tags_list_with_valid_auth() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tags")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_v1_tags_create_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tags")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "test-tag"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_tags_create_empty_name() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/tags")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "   "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_tags_update_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/tags/00000000-0000-0000-0000-000000000001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "updated-tag"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_tags_update_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/tags/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "updated-tag"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_tags_update_invalid_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/tags/not-a-uuid")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "updated-tag"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_tags_delete_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/tags/00000000-0000-0000-0000-000000000001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_tags_delete_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/tags/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_tags_delete_invalid_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/tags/not-a-uuid")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // =========================================================================
    // Content-Tags API Tests
    // =========================================================================

    #[tokio::test]
    async fn test_v1_content_tags_list_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_content_tags_list_content_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_content_tags_attach_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"tag_id": "00000000-0000-0000-0000-000000000002"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_content_tags_attach_content_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"tag_id": "00000000-0000-0000-0000-000000000002"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_content_tags_attach_invalid_tag_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"tag_id": "not-a-uuid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_content_tags_detach_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags/00000000-0000-0000-0000-000000000002")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_content_tags_detach_content_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags/00000000-0000-0000-0000-000000000002")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_content_tags_detach_invalid_content_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/contents/not-a-uuid/tags/00000000-0000-0000-0000-000000000002")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_content_tags_detach_invalid_tag_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/tags/not-a-uuid")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // =========================================================================
    // Groups API Tests
    // =========================================================================

    #[tokio::test]
    async fn test_v1_groups_list_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_list_with_valid_auth() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_v1_groups_create_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "My Collection"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_create_empty_name() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "   "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_groups_get_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_get_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_groups_get_invalid_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups/not-a-uuid")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_groups_update_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "Updated Name"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_update_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name": "Updated Name"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_groups_update_invalid_visibility() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("PATCH")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"visibility": "invalid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_groups_trash_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/trash")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_trash_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/trash")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_groups_restore_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/restore")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_archive_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/archive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_groups_unarchive_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/unarchive")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_group_contents_list_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_group_contents_list_group_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_group_contents_add_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"content_id": "00000000-0000-0000-0000-000000000002"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_group_contents_add_group_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"content_id": "00000000-0000-0000-0000-000000000002"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_group_contents_add_invalid_content_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"content_id": "not-a-uuid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_group_contents_remove_without_auth_returns_401() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents/00000000-0000-0000-0000-000000000002")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_group_contents_remove_group_not_found() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents/00000000-0000-0000-0000-000000000002")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_v1_group_contents_remove_invalid_group_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/groups/not-a-uuid/contents/00000000-0000-0000-0000-000000000002")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_v1_group_contents_remove_invalid_content_id() {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/groups/00000000-0000-0000-0000-000000000001/contents/not-a-uuid")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
