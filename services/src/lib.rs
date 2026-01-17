use crate::config::Config;
use crate::database::SqlStorage;
use crate::storage::{CFDiskConfig, R2Presigner};
use crate::users::routes::AppState;
use crate::users::storage::UserStorage;
use axum::{
    Router,
    extract::{Extension, Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{any, get},
};
use collects_utils::version_info::{RuntimeEnv, format_version_for_runtime_env};
use opentelemetry::{global, propagation::Extractor};
use tower_http::trace::TraceLayer;
use tracing_opentelemetry::OpenTelemetrySpanExt as _;

use axum::http::header;

pub mod auth;
pub mod collect_files;
pub mod collects;
pub mod config;
pub mod database;
pub mod internal;
pub mod openapi;
pub mod storage;
pub mod telemetry;
pub mod users;
pub mod v1;

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

    // v1 API routes from dedicated module
    let v1_routes = v1::create_routes::<S, U>();
    let v1_public_routes = v1::create_public_routes::<S, U>();

    let mut router = Router::new()
        .route("/is-health", get(health_check::<S, U>))
        .route("/favicon.png", get(favicon_png))
        .nest("/v1", v1_routes)
        .nest("/v1", v1_public_routes)
        .nest("/internal", internal_routes)
        .nest("/auth", users::auth_routes::<S, U>());

    // Add OpenAPI documentation routes for internal environments (protected by Zero Trust)
    if let Some(openapi_routes) = openapi::create_openapi_routes::<S, U>(&config) {
        router = router.merge(openapi_routes);
    }

    // Add R2 presigner extension if R2 is configured
    if let Some(r2_config) = config.r2() {
        let presigner = R2Presigner::new(CFDiskConfig {
            account_id: r2_config.account_id().to_owned(),
            access_key_id: r2_config.access_key_id().to_owned(),
            secret_access_key: r2_config.secret_access_key().to_owned(),
            bucket: r2_config.bucket().to_owned(),
        });
        router = router.layer(Extension(presigner));
    }

    router
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

async fn favicon_png() -> impl IntoResponse {
    let bytes: &'static [u8] = collects_assets::icon();
    (
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=3600"),
        ],
        bytes,
    )
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::storage::{MockUserStorage, StoredUser};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    /// A fixed UUID for test scenarios to coordinate between MockSqlStorage and MockUserStorage.
    const TEST_USER_ID: uuid::Uuid = uuid::Uuid::from_u128(0x00000000_0000_0000_0000_000000000001);

    /// A fixed UUID for test content.
    const TEST_CONTENT_ID: uuid::Uuid =
        uuid::Uuid::from_u128(0x00000000_0000_0000_0000_000000000000);

    #[derive(Clone)]
    struct MockSqlStorage {
        is_connected: bool,
        /// When set, mock methods will use this user ID for ownership checks.
        mock_user_id: Option<uuid::Uuid>,
    }

    impl MockSqlStorage {
        /// Creates a new MockSqlStorage with default settings (connected, no mock user ID).
        fn new() -> Self {
            Self {
                is_connected: true,
                mock_user_id: None,
            }
        }

        /// Creates a MockSqlStorage configured to work with a specific user ID.
        fn with_user_id(user_id: uuid::Uuid) -> Self {
            Self {
                is_connected: true,
                mock_user_id: Some(user_id),
            }
        }

        /// Creates a MockSqlStorage that simulates a disconnected database.
        fn disconnected() -> Self {
            Self {
                is_connected: false,
                mock_user_id: None,
            }
        }
    }

    /// Creates a MockUserStorage with a user that has the TEST_USER_ID.
    fn create_test_user_storage() -> MockUserStorage {
        let user = StoredUser::with_id(TEST_USER_ID, "testuser", "SECRET123");
        let storage = MockUserStorage::new();
        // We need to insert the user manually since with_users generates random IDs
        storage
            .users
            .write()
            .expect("lock poisoned")
            .insert("testuser".to_owned(), user);
        storage
    }

    impl SqlStorage for MockSqlStorage {
        async fn is_connected(&self) -> bool {
            self.is_connected
        }

        async fn contents_insert(
            &self,
            input: crate::database::ContentsInsert,
        ) -> Result<crate::database::ContentRow, crate::database::SqlStorageError> {
            // Return a mock content row based on the input
            Ok(crate::database::ContentRow {
                id: uuid::Uuid::new_v4(),
                user_id: input.user_id,
                title: input.title,
                description: input.description,
                storage_backend: input.storage_backend,
                storage_profile: input.storage_profile,
                storage_key: input.storage_key,
                content_type: input.content_type,
                file_size: input.file_size,
                status: "active".to_owned(),
                visibility: input.visibility.as_db_str().to_owned(),
                kind: input.kind.unwrap_or_else(|| "file".to_owned()),
                body: input.body,
                trashed_at: None,
                archived_at: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
        }

        async fn contents_get(
            &self,
            id: uuid::Uuid,
        ) -> Result<Option<crate::database::ContentRow>, crate::database::SqlStorageError> {
            // Return a mock content for the test content ID when mock_user_id is set
            if id == TEST_CONTENT_ID
                && let Some(user_id) = self.mock_user_id
            {
                return Ok(Some(crate::database::ContentRow {
                    id: TEST_CONTENT_ID,
                    user_id,
                    title: "Test Content".to_owned(),
                    description: None,
                    storage_backend: "r2".to_owned(),
                    storage_profile: "default".to_owned(),
                    storage_key: format!("{}/test-uuid/test-file.jpg", user_id),
                    content_type: "image/jpeg".to_owned(),
                    file_size: 1234,
                    status: "active".to_owned(),
                    visibility: "private".to_owned(),
                    kind: "file".to_owned(),
                    body: None,
                    trashed_at: None,
                    archived_at: None,
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                }));
            }
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
                "MockSqlStorage.groups_create: unimplemented".to_owned(),
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

        async fn group_items_reorder(
            &self,
            _group_id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _items: &[(uuid::Uuid, i32)],
        ) -> Result<(), crate::database::SqlStorageError> {
            Ok(())
        }

        async fn tags_create(
            &self,
            _input: crate::database::TagCreate,
        ) -> Result<crate::database::TagRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.tags_create: unimplemented".to_owned(),
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
                "MockSqlStorage.share_links_create: unimplemented".to_owned(),
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

        async fn share_links_get(
            &self,
            _id: uuid::Uuid,
            _owner_id: uuid::Uuid,
        ) -> Result<Option<crate::database::ShareLinkRow>, crate::database::SqlStorageError>
        {
            Ok(None)
        }

        async fn share_links_update(
            &self,
            _id: uuid::Uuid,
            _owner_id: uuid::Uuid,
            _input: crate::database::ShareLinkUpdate,
        ) -> Result<Option<crate::database::ShareLinkRow>, crate::database::SqlStorageError>
        {
            Ok(None)
        }

        async fn share_links_delete(
            &self,
            _id: uuid::Uuid,
            _owner_id: uuid::Uuid,
        ) -> Result<bool, crate::database::SqlStorageError> {
            Ok(false)
        }

        async fn share_links_increment_access(
            &self,
            _id: uuid::Uuid,
        ) -> Result<(), crate::database::SqlStorageError> {
            Ok(())
        }

        async fn content_shares_attach_link(
            &self,
            _content_id: uuid::Uuid,
            _share_link_id: uuid::Uuid,
            _created_by: uuid::Uuid,
        ) -> Result<(), crate::database::SqlStorageError> {
            Ok(())
        }

        async fn group_shares_attach_link(
            &self,
            _group_id: uuid::Uuid,
            _share_link_id: uuid::Uuid,
            _created_by: uuid::Uuid,
        ) -> Result<(), crate::database::SqlStorageError> {
            Ok(())
        }

        async fn contents_get_by_share_token(
            &self,
            _token: &str,
        ) -> Result<
            Option<(crate::database::ContentRow, crate::database::ShareLinkRow)>,
            crate::database::SqlStorageError,
        > {
            Ok(None)
        }

        async fn groups_get_by_share_token(
            &self,
            _token: &str,
        ) -> Result<
            Option<(
                crate::database::ContentGroupRow,
                crate::database::ShareLinkRow,
                i64,
            )>,
            crate::database::SqlStorageError,
        > {
            Ok(None)
        }

        async fn content_shares_create_for_user(
            &self,
            _input: crate::database::ContentShareCreateForUser,
        ) -> Result<crate::database::ContentShareRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.content_shares_create_for_user: unimplemented".to_owned(),
            ))
        }

        async fn content_shares_create_for_link(
            &self,
            _input: crate::database::ContentShareCreateForLink,
        ) -> Result<crate::database::ContentShareRow, crate::database::SqlStorageError> {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.content_shares_create_for_link: unimplemented".to_owned(),
            ))
        }

        async fn group_shares_create_for_user(
            &self,
            _input: crate::database::GroupShareCreateForUser,
        ) -> Result<crate::database::ContentGroupShareRow, crate::database::SqlStorageError>
        {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.group_shares_create_for_user: unimplemented".to_owned(),
            ))
        }

        async fn group_shares_create_for_link(
            &self,
            _input: crate::database::GroupShareCreateForLink,
        ) -> Result<crate::database::ContentGroupShareRow, crate::database::SqlStorageError>
        {
            Err(crate::database::SqlStorageError::Db(
                "MockSqlStorage.group_shares_create_for_link: unimplemented".to_owned(),
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

        async fn uploads_create(
            &self,
            input: crate::database::UploadInsert,
        ) -> Result<crate::database::UploadRow, crate::database::SqlStorageError> {
            // Return a mock upload row based on the input
            Ok(crate::database::UploadRow {
                id: uuid::Uuid::new_v4(),
                user_id: input.user_id,
                storage_backend: input.storage_backend,
                storage_profile: input.storage_profile,
                storage_key: input.storage_key,
                content_type: input.content_type,
                file_size: input.file_size,
                status: "initiated".to_owned(),
                expires_at: input.expires_at,
                created_at: chrono::Utc::now(),
                completed_at: None,
            })
        }

        async fn uploads_get(
            &self,
            _id: uuid::Uuid,
        ) -> Result<Option<crate::database::UploadRow>, crate::database::SqlStorageError> {
            Ok(None)
        }

        async fn uploads_complete(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
        ) -> Result<Option<crate::database::UploadRow>, crate::database::SqlStorageError> {
            Ok(None)
        }

        async fn revoked_tokens_add(
            &self,
            _token_hash: &str,
            _username: &str,
            _expires_at: chrono::DateTime<chrono::Utc>,
        ) -> Result<(), crate::database::SqlStorageError> {
            // Mock: silently succeed
            Ok(())
        }

        async fn revoked_tokens_is_revoked(
            &self,
            _token_hash: &str,
        ) -> Result<bool, crate::database::SqlStorageError> {
            // Mock: tokens are never revoked
            Ok(false)
        }
    }

    #[tokio::test]
    async fn test_health_check_connected() {
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::with_user_id(TEST_USER_ID);
        let user_storage = create_test_user_storage();
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

        // Verify we return a real R2 presigned URL (not a mock placeholder)
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let upload_url = json["upload_url"].as_str().unwrap_or_default();
        assert!(upload_url.contains("r2.cloudflarestorage.com"));
        assert!(!upload_url.contains("test.r2.example.com"));
        assert!(!upload_url.contains("mock=true"));
    }

    #[tokio::test]
    async fn test_v1_contents_view_url_without_auth_returns_401() {
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::with_user_id(TEST_USER_ID);
        let user_storage = create_test_user_storage();
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

        // Verify we return a real R2 presigned URL (not a mock placeholder)
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let url = json["url"].as_str().unwrap_or_default();
        assert!(url.contains("r2.cloudflarestorage.com"));
        assert!(!url.contains("test.r2.example.com"));
        assert!(!url.contains("mock=true"));
    }

    #[tokio::test]
    async fn test_v1_me_with_invalid_token_returns_401() {
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::disconnected();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
        let sql_storage = MockSqlStorage::new();
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
    // Text Content API Tests (POST /v1/contents)
    // =========================================================================

    #[tokio::test]
    async fn test_v1_contents_create_text_without_auth_returns_401() {
        let sql_storage = MockSqlStorage::new();
        let user_storage = MockUserStorage::new();
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"title":"My Note","body":"Hello, world!"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_v1_contents_create_text_success() {
        let sql_storage = MockSqlStorage::new();
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from(
                        r#"{"title":"My Note","body":"Hello, world!","content_type":"text/plain"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        // Verify the response structure
        assert!(json["content"]["id"].is_string());
        assert_eq!(json["content"]["title"], "My Note");
        assert_eq!(json["content"]["kind"], "text");
        assert_eq!(json["content"]["body"], "Hello, world!");
        assert_eq!(json["content"]["content_type"], "text/plain");
        assert_eq!(json["content"]["storage_backend"], "inline");
        assert_eq!(json["content"]["file_size"], 13); // "Hello, world!".len()
    }

    #[tokio::test]
    async fn test_v1_contents_create_text_with_markdown() {
        let sql_storage = MockSqlStorage::new();
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let payload = serde_json::json!({
            "title": "README",
            "body": "# Hello World",
            "content_type": "text/markdown"
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["content"]["content_type"], "text/markdown");
        assert_eq!(json["content"]["kind"], "text");
    }

    #[tokio::test]
    async fn test_v1_contents_create_text_invalid_content_type() {
        let sql_storage = MockSqlStorage::new();
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from(
                        r#"{"title":"Binary","body":"data","content_type":"application/octet-stream"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], "bad_request");
        assert!(json["message"].as_str().unwrap().contains("text/*"));
    }

    #[tokio::test]
    async fn test_v1_contents_create_text_invalid_visibility() {
        let sql_storage = MockSqlStorage::new();
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {}", token))
                    .body(Body::from(
                        r#"{"title":"Note","body":"text","visibility":"invalid"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .expect("Request failed");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("Invalid JSON");

        assert_eq!(json["error"], "bad_request");
        assert!(
            json["message"]
                .as_str()
                .expect("Message not str")
                .contains("Invalid visibility")
        );
    }

    #[tokio::test]
    async fn test_v1_contents_create_text_default_content_type() {
        let sql_storage = MockSqlStorage::new();
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
        let config = Config::new_for_test();
        let app = routes(sql_storage, user_storage, config).await;

        let token = generate_test_token();

        // Request without content_type - should default to text/plain
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/contents")
                    .header("content-type", "application/json")
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::from(r#"{"title":"Note","body":"Some text"}"#))
                    .expect("Invalid request"),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");
        let json: serde_json::Value = serde_json::from_slice(&body).expect("Invalid JSON");

        assert_eq!(json["content"]["content_type"], "text/plain");
    }
}
