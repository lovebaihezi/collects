//! Internal routes module.
//!
//! This module contains route configuration for internal endpoints
//! protected by Cloudflare Zero Trust authentication.

use std::sync::Arc;

use axum::{Router, middleware};

use crate::auth;
use crate::config::Config;
use crate::database::SqlStorage;
use crate::users::storage::UserStorage;
use crate::users::{self, AppState};

/// Create internal routes with optional Zero Trust middleware.
///
/// This uses the default JWKS resolver (HTTP fetch via reqwest).
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
        _ => routes,
    }
}
