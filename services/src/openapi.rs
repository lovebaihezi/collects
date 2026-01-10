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
use crate::v1::{content_tags, contents, groups, me, public, share_links, tags, types, uploads};

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
        (url = "https://collects-internal.lqxclqxc.com", description = "Internal Production"),
        (url = "https://collects-test-internal.lqxclqxc.com", description = "Internal Test"),
    ),
    tags(
        (name = "me", description = "Current user information"),
        (name = "contents", description = "Content management endpoints"),
        (name = "content-tags", description = "Content-tag relationship endpoints"),
        (name = "tags", description = "Tag management endpoints"),
        (name = "groups", description = "Group management endpoints"),
        (name = "uploads", description = "File upload endpoints"),
        (name = "share-links", description = "Share link management endpoints"),
        (name = "public", description = "Public share access endpoints (unauthenticated)"),
    ),
    paths(
        // Me
        me::v1_me,
        // Contents
        contents::v1_contents_list,
        contents::v1_contents_create,
        contents::v1_contents_get,
        contents::v1_contents_update,
        contents::v1_contents_trash,
        contents::v1_contents_restore,
        contents::v1_contents_archive,
        contents::v1_contents_unarchive,
        contents::v1_contents_view_url,
        // Content Tags
        content_tags::v1_content_tags_list,
        content_tags::v1_content_tags_attach,
        content_tags::v1_content_tags_detach,
        // Tags
        tags::v1_tags_list,
        tags::v1_tags_create,
        tags::v1_tags_update,
        tags::v1_tags_delete,
        // Groups
        groups::v1_groups_list,
        groups::v1_groups_create,
        groups::v1_groups_get,
        groups::v1_groups_update,
        groups::v1_groups_trash,
        groups::v1_groups_restore,
        groups::v1_groups_archive,
        groups::v1_groups_unarchive,
        groups::v1_groups_contents_list,
        groups::v1_groups_contents_add,
        groups::v1_groups_contents_remove,
        groups::v1_groups_contents_reorder,
        // Uploads
        uploads::v1_uploads_init,
        uploads::v1_uploads_complete,
        // Share Links
        share_links::v1_share_links_list,
        share_links::v1_share_links_create,
        share_links::v1_share_links_get,
        share_links::v1_share_links_update,
        share_links::v1_share_links_delete,
        share_links::v1_contents_share_link_create,
        share_links::v1_groups_share_link_create,
        // Public
        public::v1_public_share_get,
        public::v1_public_share_view_url,
    ),
    components(
        schemas(
            types::V1ErrorResponse,
            types::V1MeResponse,
            types::V1ContentItem,
            types::V1ContentsListQuery,
            types::V1ContentsListResponse,
            types::V1ContentsUpdateRequest,
            types::V1ContentCreateRequest,
            types::V1ContentCreateResponse,
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
            types::V1UploadsCompleteRequest,
            types::V1UploadsCompleteResponse,
            types::V1ShareLinkCreateRequest,
            types::V1ShareLinkUpdateRequest,
            types::V1ShareLinkResponse,
            types::V1ShareLinksListResponse,
            types::V1ContentShareLinkCreateRequest,
            types::V1GroupShareLinkCreateRequest,
            types::V1PublicShareResponse,
            types::V1PublicViewUrlRequest,
            types::V1PublicViewUrlResponse,
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
        // Verify paths are included
        assert!(json.contains("/v1/me"), "Should include /v1/me path");
        assert!(
            json.contains("/v1/contents"),
            "Should include /v1/contents path"
        );
        assert!(json.contains("/v1/tags"), "Should include /v1/tags path");
        assert!(
            json.contains("/v1/groups"),
            "Should include /v1/groups path"
        );
        assert!(
            json.contains("/v1/uploads/init"),
            "Should include /v1/uploads/init path"
        );
        assert!(
            json.contains("/v1/share-links"),
            "Should include /v1/share-links path"
        );
        assert!(
            json.contains("/v1/public/share/{token}"),
            "Should include /v1/public/share/{{token}} path"
        );
    }
}
