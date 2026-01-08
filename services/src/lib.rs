//! Collects Services - API server for the Collects application.

use crate::config::Config;
use crate::database::SqlStorage;
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

    // V1 API routes
    let v1_routes = v1::routes::<S, U>();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{
        ContentGroupItemRow, ContentGroupRow, ContentRow, ContentsInsert, ContentsListParams,
        ContentsUpdate, GroupCreate, GroupUpdate, GroupsListParams, OtpAttemptRecord,
        OtpRateLimitConfig, ShareLinkCreate, ShareLinkRow, SqlStorageError, TagCreate, TagRow,
        TagUpdate,
    };
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
            _input: ContentsInsert,
        ) -> Result<ContentRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.contents_insert: unimplemented".to_string(),
            ))
        }

        async fn contents_get(
            &self,
            _id: uuid::Uuid,
        ) -> Result<Option<ContentRow>, SqlStorageError> {
            Ok(None)
        }

        async fn contents_list_for_user(
            &self,
            _user_id: uuid::Uuid,
            _params: ContentsListParams,
        ) -> Result<Vec<ContentRow>, SqlStorageError> {
            Ok(vec![])
        }

        async fn contents_update_metadata(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _changes: ContentsUpdate,
        ) -> Result<Option<ContentRow>, SqlStorageError> {
            Ok(None)
        }

        async fn contents_set_status(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _new_status: crate::database::ContentStatus,
            _now: chrono::DateTime<chrono::Utc>,
        ) -> Result<Option<ContentRow>, SqlStorageError> {
            Ok(None)
        }

        async fn groups_create(
            &self,
            _input: GroupCreate,
        ) -> Result<ContentGroupRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.groups_create: unimplemented".to_string(),
            ))
        }

        async fn groups_get(
            &self,
            _id: uuid::Uuid,
        ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
            Ok(None)
        }

        async fn groups_list_for_user(
            &self,
            _user_id: uuid::Uuid,
            _params: GroupsListParams,
        ) -> Result<Vec<ContentGroupRow>, SqlStorageError> {
            Ok(vec![])
        }

        async fn groups_update_metadata(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _changes: GroupUpdate,
        ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
            Ok(None)
        }

        async fn groups_set_status(
            &self,
            _id: uuid::Uuid,
            _user_id: uuid::Uuid,
            _new_status: crate::database::GroupStatus,
            _now: chrono::DateTime<chrono::Utc>,
        ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
            Ok(None)
        }

        async fn group_items_add(
            &self,
            _group_id: uuid::Uuid,
            _content_id: uuid::Uuid,
            _sort_order: i32,
        ) -> Result<(), SqlStorageError> {
            Ok(())
        }

        async fn group_items_remove(
            &self,
            _group_id: uuid::Uuid,
            _content_id: uuid::Uuid,
        ) -> Result<bool, SqlStorageError> {
            Ok(false)
        }

        async fn group_items_list(
            &self,
            _group_id: uuid::Uuid,
        ) -> Result<Vec<ContentGroupItemRow>, SqlStorageError> {
            Ok(vec![])
        }

        async fn tags_create(&self, _input: TagCreate) -> Result<TagRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.tags_create: unimplemented".to_string(),
            ))
        }

        async fn tags_list_for_user(
            &self,
            _user_id: uuid::Uuid,
        ) -> Result<Vec<TagRow>, SqlStorageError> {
            Ok(vec![])
        }

        async fn tags_delete(
            &self,
            _user_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
        ) -> Result<bool, SqlStorageError> {
            Ok(false)
        }

        async fn tags_update(
            &self,
            _user_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
            _changes: TagUpdate,
        ) -> Result<Option<TagRow>, SqlStorageError> {
            Ok(None)
        }

        async fn content_tags_attach(
            &self,
            _content_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
        ) -> Result<(), SqlStorageError> {
            Ok(())
        }

        async fn content_tags_detach(
            &self,
            _content_id: uuid::Uuid,
            _tag_id: uuid::Uuid,
        ) -> Result<bool, SqlStorageError> {
            Ok(false)
        }

        async fn content_tags_list_for_content(
            &self,
            _content_id: uuid::Uuid,
        ) -> Result<Vec<TagRow>, SqlStorageError> {
            Ok(vec![])
        }

        async fn share_links_create(
            &self,
            _input: ShareLinkCreate,
        ) -> Result<ShareLinkRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.share_links_create: unimplemented".to_string(),
            ))
        }

        async fn share_links_get_by_token(
            &self,
            _token: &str,
        ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
            Ok(None)
        }

        async fn share_links_list_for_owner(
            &self,
            _owner_id: uuid::Uuid,
        ) -> Result<Vec<ShareLinkRow>, SqlStorageError> {
            Ok(vec![])
        }

        async fn share_links_deactivate(
            &self,
            _owner_id: uuid::Uuid,
            _share_link_id: uuid::Uuid,
        ) -> Result<bool, SqlStorageError> {
            Ok(false)
        }

        async fn content_shares_create_for_user(
            &self,
            _input: crate::database::ContentShareCreateForUser,
        ) -> Result<crate::database::ContentShareRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.content_shares_create_for_user: unimplemented".to_string(),
            ))
        }

        async fn content_shares_create_for_link(
            &self,
            _input: crate::database::ContentShareCreateForLink,
        ) -> Result<crate::database::ContentShareRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.content_shares_create_for_link: unimplemented".to_string(),
            ))
        }

        async fn group_shares_create_for_user(
            &self,
            _input: crate::database::GroupShareCreateForUser,
        ) -> Result<crate::database::ContentGroupShareRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.group_shares_create_for_user: unimplemented".to_string(),
            ))
        }

        async fn group_shares_create_for_link(
            &self,
            _input: crate::database::GroupShareCreateForLink,
        ) -> Result<crate::database::ContentGroupShareRow, SqlStorageError> {
            Err(SqlStorageError::Db(
                "MockSqlStorage.group_shares_create_for_link: unimplemented".to_string(),
            ))
        }

        async fn otp_record_attempt(
            &self,
            _record: OtpAttemptRecord,
        ) -> Result<(), SqlStorageError> {
            Ok(())
        }

        async fn otp_is_rate_limited(
            &self,
            _username: &str,
            _ip_address: Option<std::net::IpAddr>,
            _config: &OtpRateLimitConfig,
        ) -> Result<bool, SqlStorageError> {
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

    fn generate_test_token() -> String {
        use crate::users::otp::generate_session_token;
        let secret = "test-jwt-secret-key-for-local-development";
        generate_session_token("testuser", secret).unwrap()
    }

    // =========================================================================
    // Me API Tests
    // =========================================================================

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
        let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
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

    // =========================================================================
    // Uploads API Tests
    // =========================================================================

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
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/view-url")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"disposition":"inline"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

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
                    .uri("/v1/contents/00000000-0000-0000-0000-000000000001/view-url")
                    .header("Authorization", format!("Bearer {}", token))
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"disposition":"inline"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
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
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"visibility": "invalid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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

    // =========================================================================
    // Misc Tests
    // =========================================================================

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

        assert!(response.headers().contains_key("x-service-env"));
        assert!(response.headers().contains_key("x-service-version"));
    }

    #[test]
    fn test_env_to_runtime_env_conversion() {
        use crate::config::Env;

        let local: RuntimeEnv = (&Env::Local).into();
        assert_eq!(local, RuntimeEnv::Local);

        let test: RuntimeEnv = (&Env::Test).into();
        assert_eq!(test, RuntimeEnv::Test);

        let prod: RuntimeEnv = (&Env::Prod).into();
        assert_eq!(prod, RuntimeEnv::Prod);

        let internal: RuntimeEnv = (&Env::Internal).into();
        assert_eq!(internal, RuntimeEnv::Internal);
    }
}
