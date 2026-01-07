use crate::config::Config;
use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::storage::UserStorage;
use axum::{
    Json, Router,
    extract::{Extension, Path, Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{any, get, post},
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
        .route(
            "/contents/{id}/view-url",
            post(v1_contents_view_url::<S, U>),
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

#[derive(Debug, Serialize)]
struct V1MeResponse {
    // Placeholder. Real implementation should return the authenticated user/session identity.
    ok: bool,
}

async fn v1_me<S, U>(State(_state): State<AppState<S, U>>) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    (StatusCode::OK, Json(V1MeResponse { ok: true }))
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
    Json(payload): Json<V1UploadsInitRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Use request fields to avoid dead-code warnings while this is still a stub.
    let _ = (&payload.content_type, payload.file_size);

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
    Path(_id): Path<String>,
    Json(payload): Json<V1ViewUrlRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Use request fields to avoid dead-code warnings while this is still a stub.
    let _ = payload.disposition;

    (
        StatusCode::OK,
        Json(V1ViewUrlResponse {
            url: "https://example.invalid/view".to_string(),
            expires_at: "1970-01-01T00:00:00Z".to_string(),
        }),
    )
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

    // MVP v1 API TODO: first undone endpoints should not exist yet.
    // These tests intentionally assert the *desired* behavior (200/201),
    // so they will fail until the endpoints are implemented and wired into the router.

    #[tokio::test]
    async fn test_v1_me_should_exist_but_currently_missing() {
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

        // Expected (once implemented): 200 OK with current user/session info.
        // Current behavior: 404 from fallback.
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_v1_uploads_init_should_exist_but_currently_missing() {
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

        // Expected (once implemented): 201 Created with upload session info.
        // Current behavior: 404 from fallback.
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_v1_contents_view_url_should_exist_but_currently_missing() {
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

        // Expected (once implemented): 200 OK with { url, expires_at }.
        // Current behavior: 404 from fallback.
        assert_eq!(response.status(), StatusCode::OK);
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
}
