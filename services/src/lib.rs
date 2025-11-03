use axum::{
    Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get},
    middleware,
};
use sqlx::PgPool;
use crate::auth::{auth_middleware, JwksClient};
use crate::config::Config;
use axum::Extension;

pub mod config;
pub mod database;
pub mod auth;

pub fn routes(pool: PgPool, config: Config) -> Router {
    let jwks_client = JwksClient::new(config.clerk_frontend_api().to_string());
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

async fn protected_route() -> impl IntoResponse {
    (StatusCode::OK, "This is a protected route")
}

async fn catch_all() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "nothing to see here")
}
