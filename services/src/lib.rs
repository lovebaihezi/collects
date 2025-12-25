use crate::config::Config;
use crate::database::SqlStorage;
use axum::{
    Router,
    extract::{Extension, Request, State},
    http::{HeaderName, HeaderValue, StatusCode},
    middleware,
    response::IntoResponse,
    routing::{any, get},
};
use opentelemetry::{global, propagation::Extractor};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod auth;
pub mod config;
pub mod database;
pub mod storage;
pub mod telemetry;
pub mod users;

const SERVICE_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_COMMIT: &str = env!("BUILD_COMMIT");

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

pub async fn routes<S>(storage: S, config: Config) -> Router
where
    S: SqlStorage + Clone + Send + Sync + 'static,
{
    // Build the protected internal routes with Zero Trust middleware if configured
    let internal_routes = create_internal_routes::<S>(&config);

    Router::new()
        .route("/is-health", get(health_check::<S>))
        .nest("/internal", internal_routes)
        .nest("/auth", users::auth_routes::<S>())
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
        .with_state(storage)
}

/// Create internal routes with optional Zero Trust middleware
fn create_internal_routes<S>(config: &Config) -> Router<S>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
{
    if let (Some(team_domain), Some(audience)) =
        (config.cf_access_team_domain(), config.cf_access_aud())
    {
        let zero_trust_config = Arc::new(auth::ZeroTrustConfig::new(
            team_domain.to_string(),
            audience.to_string(),
        ));

        users::internal_routes::<S>().layer(middleware::from_fn(move |req, next| {
            let config = Arc::clone(&zero_trust_config);
            auth::zero_trust_middleware(config, req, next)
        }))
    } else {
        // If Zero Trust is not configured, use routes without authentication
        // This is useful for local development
        users::internal_routes::<S>()
    }
}

async fn health_check<S>(
    State(storage): State<S>,
    Extension(config): Extension<Config>,
) -> impl IntoResponse
where
    S: SqlStorage,
{
    let mut response = if storage.is_connected().await {
        (StatusCode::OK, "OK").into_response()
    } else {
        (StatusCode::BAD_GATEWAY, "502").into_response()
    };

    let env_value = config.environment().to_string();
    response.headers_mut().insert(
        HeaderName::from_static("x-service-env"),
        HeaderValue::from_str(&env_value).expect("environment header is valid ASCII"),
    );

    let version_value = format!("{SERVICE_VERSION}+{BUILD_COMMIT}");
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
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[derive(Clone)]
    struct MockStorage {
        is_connected: bool,
    }

    impl SqlStorage for MockStorage {
        async fn is_connected(&self) -> bool {
            self.is_connected
        }
    }

    #[tokio::test]
    async fn test_health_check_connected() {
        let storage = MockStorage { is_connected: true };
        let config = Config::new_for_test();
        let app = routes(storage, config).await;

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

    #[tokio::test]
    async fn test_health_check_includes_headers() {
        let storage = MockStorage { is_connected: true };
        let config = Config::new_for_test();
        let app = routes(storage, config).await;

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
        let expected_version = format!("{SERVICE_VERSION}+{BUILD_COMMIT}");
        assert_eq!(version_header, Some(expected_version.as_str()));
    }

    #[tokio::test]
    async fn test_health_check_disconnected() {
        let storage = MockStorage {
            is_connected: false,
        };
        let config = Config::new_for_test();
        let app = routes(storage, config).await;

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
}
