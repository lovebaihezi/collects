use crate::auth::{Claims, JwksClient, auth_middleware};
use crate::config::Config;
use axum::{
    Extension, Router,
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{any, get},
};
use sqlx::PgPool;
use std::sync::Arc;

pub mod auth;
pub mod config;
pub mod database;

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
        .with_state(pool)
}

async fn protected_route(Extension(claims): Extension<Claims>) -> impl IntoResponse {
    (StatusCode::OK, format!("Welcome, {}!", claims.sub))
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
