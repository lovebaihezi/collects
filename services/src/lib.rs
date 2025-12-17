use crate::auth::{Claims, JwksClient, auth_middleware};
use crate::config::Config;
use axum::{
    Extension, Router,
    extract::Request,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{any, get},
};
use opentelemetry::{global, propagation::Extractor};
use sqlx::PgPool;
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing_opentelemetry::OpenTelemetrySpanExt;

pub mod auth;
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

pub async fn routes(pool: PgPool, config: Config) -> Router {
    let jwks_client = Arc::new(JwksClient::new(config.clerk_frontend_api().to_string()));

    Router::new()
        .route("/is-health", get(async || "OK"))
        .route(
            "/protected",
            get(protected_route).route_layer(middleware::from_fn(auth_middleware)),
        )
        .fallback(any(catch_all))
        .layer(Extension(jwks_client))
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
        .with_state(pool)
}

async fn protected_route(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, format!("Welcome, {}!", claims.sub))
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
