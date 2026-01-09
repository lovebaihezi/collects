//! Internal routes module.
//!
//! This module contains route configuration for internal endpoints
//! protected by Cloudflare Zero Trust authentication.
//!
//! # Security
//!
//! In deployed environments (Internal, TestInternal, Prod, Nightly, Pr),
//! Zero Trust configuration is **required**. If `CF_ACCESS_TEAM_DOMAIN` and
//! `CF_ACCESS_AUD` are not set, all requests to internal routes will be
//! rejected with 401 Unauthorized.
//!
//! Only Local and Test environments allow internal routes without Zero Trust
//! to facilitate local development and basic testing.

use std::sync::Arc;

use axum::{
    Json, Router,
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::any,
};

use crate::auth;
use crate::config::Config;
use crate::database::SqlStorage;
use crate::users::storage::UserStorage;
use crate::users::{self, AppState};

/// Create internal routes with optional Zero Trust middleware.
///
/// This uses the default JWKS resolver (HTTP fetch via reqwest).
///
/// # Security
///
/// In deployed environments, Zero Trust configuration is required.
/// If not configured, all requests will be rejected with 401 Unauthorized.
pub fn create_internal_routes<S, U>(config: &Config) -> Router<AppState<S, U>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    create_internal_routes_with_resolver::<S, U>(config, Arc::new(auth::ReqwestJwksKeyResolver))
}

/// Create internal routes with optional Zero Trust middleware, using a custom JWKS resolver.
///
/// This exists primarily for deterministic tests in "internal env" (Zero Trust enabled)
/// without making external network calls to Cloudflare's JWKS endpoint.
///
/// # Security
///
/// When `requires_zero_trust_for_internal()` returns true (deployed environments)
/// but Zero Trust config is missing, a fallback route is returned that rejects
/// all requests with 401 Unauthorized. This ensures fail-secure behavior.
pub fn create_internal_routes_with_resolver<S, U>(
    config: &Config,
    resolver: Arc<dyn auth::JwksKeyResolver>,
) -> Router<AppState<S, U>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    let routes = users::internal_routes::<S, U>();

    match (config.cf_access_team_domain(), config.cf_access_aud()) {
        (Some(team_domain), Some(audience)) => {
            // Zero Trust is configured - apply middleware
            let zero_trust_config = Arc::new(auth::ZeroTrustConfig::new(
                team_domain.to_string(),
                audience.to_string(),
            ));

            routes.layer(middleware::from_fn(move |req, next| {
                let config = Arc::clone(&zero_trust_config);
                let resolver = Arc::clone(&resolver);
                auth::zero_trust_middleware_with_resolver(resolver, config, req, next)
            }))
        }
        _ if config.requires_zero_trust_for_internal() => {
            // Deployed environment but Zero Trust is NOT configured - fail secure
            // Return a router that rejects all requests with 401
            tracing::warn!(
                env = %config.environment(),
                "Zero Trust not configured for internal routes in deployed environment - all requests will be rejected"
            );
            Router::new().fallback(any(zero_trust_not_configured))
        }
        _ => {
            // Local/Test environment without Zero Trust - allow unprotected access
            routes
        }
    }
}

/// Handler that rejects requests when Zero Trust is not configured in deployed environments.
async fn zero_trust_not_configured() -> Response {
    let error = serde_json::json!({
        "error": "zero_trust_not_configured",
        "message": "Internal routes require Zero Trust authentication which is not configured"
    });
    (StatusCode::UNAUTHORIZED, Json(error)).into_response()
}
