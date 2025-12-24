use crate::config::Config;
use crate::database::PersistentStructureDataService;
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get},
};
use opentelemetry::{global, propagation::Extractor};
use tower_http::trace::TraceLayer;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod config;
pub mod database;
pub mod telemetry;

struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

pub async fn routes<S>(storage: S, _config: Config) -> Router
where
    S: PersistentStructureDataService + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/is-health", get(health_check::<S>))
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
        .with_state(storage)
}

async fn health_check<S>(State(storage): State<S>) -> impl IntoResponse
where
    S: PersistentStructureDataService,
{
    if storage.is_connected().await {
        (StatusCode::OK, "OK")
    } else {
        (StatusCode::BAD_GATEWAY, "502")
    }
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
    struct NeonTestService {
        is_connected: bool,
    }

    impl PersistentStructureDataService for NeonTestService {
        async fn is_connected(&self) -> bool {
            self.is_connected
        }
    }

    #[tokio::test]
    async fn test_health_check_connected() {
        let storage = NeonTestService { is_connected: true };
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
    async fn test_health_check_disconnected() {
        let storage = NeonTestService {
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
