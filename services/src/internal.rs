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
pub fn create_internal_routes<S, U>(config: &Config) -> Router<AppState<S, U>>
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
                auth::zero_trust_middleware(config, req, next)
            }))
        }
        _ => routes,
    }
}


