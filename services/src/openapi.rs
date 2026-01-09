//! OpenAPI documentation module.
//!
//! This module provides OpenAPI/Swagger documentation for the Collects API.
//! Routes are only accessible in internal environments (internal, test-internal)
//! and protected by Cloudflare Zero Trust authentication.

use std::sync::Arc;

use axum::{Router, middleware, routing::get};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::auth::{self, JwksKeyResolver};
use crate::config::{Config, Env};
use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::storage::UserStorage;
use crate::v1::types;

/// OpenAPI documentation structure.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Collects API",
        version = "1.0.0",
        description = "API for managing personal content collections",
        contact(
            name = "Collects Team",
            email = "support@lqxclqxc.com"
        ),
        license(
            name = "MIT",
            identifier = "MIT"
        )
    ),
    servers(
        (url = "https://collects-services-internal-145756646168.us-east1.run.app", description = "Internal Production"),
        (url = "https://collects-services-test-internal-145756646168.us-east1.run.app", description = "Internal Test"),
    ),
    tags(
        (name = "me", description = "Current user information"),
        (name = "contents", description = "Content management endpoints"),
        (name = "content-tags", description = "Content-tag relationship endpoints"),
        (name = "tags", description = "Tag management endpoints"),
        (name = "groups", description = "Group management endpoints"),
        (name = "uploads", description = "File upload endpoints"),
    ),
    components(
        schemas(
            types::V1ErrorResponse,
            types::V1MeResponse,
            types::V1ContentItem,
            types::V1ContentsListQuery,
            types::V1ContentsListResponse,
            types::V1ContentsUpdateRequest,
            types::V1ViewUrlRequest,
            types::V1ViewUrlResponse,
            types::V1TagItem,
            types::V1TagsListResponse,
            types::V1TagCreateRequest,
            types::V1TagUpdateRequest,
            types::V1ContentTagsAttachRequest,
            types::V1GroupItem,
            types::V1GroupsListQuery,
            types::V1GroupsListResponse,
            types::V1GroupCreateRequest,
            types::V1GroupUpdateRequest,
            types::V1GroupContentItem,
            types::V1GroupContentsListResponse,
            types::V1GroupAddContentRequest,
            types::V1GroupReorderRequest,
            types::V1GroupReorderItem,
            types::V1UploadsInitRequest,
            types::V1UploadsInitResponse,
        ),
    ),
    security(
        ("bearer_auth" = [])
    ),
    modifiers(&SecurityAddon),
)]
pub struct ApiDoc;

/// Security scheme addon for Bearer authentication.
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = &mut openapi.components {
            components.add_security_scheme(
                "bearer_auth",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .description(Some(
                            "JWT session token obtained from /auth/verify-otp endpoint",
                        ))
                        .build(),
                ),
            );
        }
    }
}

/// Check if OpenAPI documentation should be enabled for the given config.
///
/// Returns true only for internal environments:
/// - `internal` (collects-internal.lqxclqxc.com)
/// - `test-internal` (collects-test-internal.lqxclqxc.com)
pub fn is_openapi_enabled(config: &Config) -> bool {
    matches!(config.environment(), Env::Internal | Env::TestInternal)
}

/// Create OpenAPI documentation routes if enabled for the environment.
///
/// Routes are protected by Cloudflare Zero Trust authentication when configured.
/// Returns `Some(Router)` with `/docs` and `/openapi.json` routes if OpenAPI is enabled,
/// otherwise returns `None`.
pub fn create_openapi_routes<S, U>(config: &Config) -> Option<Router<AppState<S, U>>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    create_openapi_routes_with_resolver(config, Arc::new(auth::ReqwestJwksKeyResolver))
}

/// Create OpenAPI documentation routes with a custom JWKS resolver.
///
/// This exists primarily for deterministic tests without making external network calls.
pub fn create_openapi_routes_with_resolver<S, U>(
    config: &Config,
    resolver: Arc<dyn JwksKeyResolver>,
) -> Option<Router<AppState<S, U>>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    if !is_openapi_enabled(config) {
        return None;
    }

    let api = ApiDoc::openapi();
    let api_clone = api.clone();

    let routes = Router::new()
        .route(
            "/openapi.json",
            get(move || async move { axum::Json(api_clone) }),
        )
        .merge(Scalar::with_url("/docs", api));

    // Apply Zero Trust middleware if configured
    let protected_routes = match (config.cf_access_team_domain(), config.cf_access_aud()) {
        (Some(team_domain), Some(audience)) => {
            let zero_trust_config = Arc::new(auth::ZeroTrustConfig::new(
                team_domain.to_string(),
                audience.to_string(),
            ));

            routes.layer(middleware::from_fn(move |req, next| {
                let zt_config = Arc::clone(&zero_trust_config);
                let zt_resolver = Arc::clone(&resolver);
                auth::zero_trust_middleware_with_resolver(zt_resolver, zt_config, req, next)
            }))
        }
        _ => routes,
    };

    Some(protected_routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_enabled_internal() {
        let config = Config::new_for_test_with_env(Env::Internal);
        assert!(is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_enabled_test_internal() {
        let config = Config::new_for_test_with_env(Env::TestInternal);
        assert!(is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_disabled_prod() {
        let config = Config::new_for_test_with_env(Env::Prod);
        assert!(!is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_disabled_test() {
        let config = Config::new_for_test_with_env(Env::Test);
        assert!(!is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_disabled_local() {
        let config = Config::new_for_test_with_env(Env::Local);
        assert!(!is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_disabled_pr() {
        let config = Config::new_for_test_with_env(Env::Pr);
        assert!(!is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_disabled_nightly() {
        let config = Config::new_for_test_with_env(Env::Nightly);
        assert!(!is_openapi_enabled(&config));
    }

    #[test]
    fn test_openapi_json_generates() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_string_pretty(&doc).expect("Should serialize to JSON");
        assert!(json.contains("Collects API"));
        assert!(json.contains("bearer_auth"));
    }
}
