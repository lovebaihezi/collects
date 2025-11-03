use axum::{
    Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get},
};
use sqlx::PgPool;
use crate::auth::JwksClient;
use crate::config::Config;

pub mod config;
pub mod database;
pub mod auth;

pub fn routes(pool: PgPool, config: Config) -> Router {
    let jwks_client = JwksClient::new(config.clerk_frontend_api().to_string());
    Router::new()
        .route("/is-health", get(async || "OK"))
        .fallback(any(catch_all))
        .with_state((pool, jwks_client))
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
