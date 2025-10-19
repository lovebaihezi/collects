use axum::{
    Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get},
};
use sqlx::PgPool;

pub mod config;
pub mod database;

pub mod collect_files;
pub mod collects;
pub mod tags;

pub fn routes(pool: PgPool) -> Router {
    Router::new()
        .route("/is-health", get(async || "OK"))
        .fallback(any(catch_all))
        .with_state(pool)
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
