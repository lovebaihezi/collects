//! OpenAPI documentation module.
//!
//! This module provides OpenAPI/Swagger documentation for the Collects API.
//! It is only compiled when the `openapi` feature is enabled, and routes are
//! only accessible in internal environments (internal, test-internal).

use axum::{Router, routing::get};
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::config::Env;
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

/// Check if OpenAPI documentation should be enabled for the given environment.
///
/// Returns true only for internal environments:
/// - `internal` (collects-internal.lqxclqxc.com)
/// - `test-internal` (collects-test-internal.lqxclqxc.com)
pub fn is_openapi_enabled(env: &Env) -> bool {
    matches!(env, Env::Internal | Env::TestInternal)
}

/// Create OpenAPI documentation routes if enabled for the environment.
///
/// Returns `Some(Router)` with `/docs` and `/openapi.json` routes if OpenAPI is enabled,
/// otherwise returns `None`.
pub fn create_openapi_routes<S, U>(env: &Env) -> Option<Router<AppState<S, U>>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    if !is_openapi_enabled(env) {
        return None;
    }

    let api = ApiDoc::openapi();
    let api_clone = api.clone();

    Some(
        Router::new()
            .route(
                "/openapi.json",
                get(move || async move { axum::Json(api_clone) }),
            )
            .merge(Scalar::with_url("/docs", api)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_enabled_internal() {
        assert!(is_openapi_enabled(&Env::Internal));
    }

    #[test]
    fn test_openapi_enabled_test_internal() {
        assert!(is_openapi_enabled(&Env::TestInternal));
    }

    #[test]
    fn test_openapi_disabled_prod() {
        assert!(!is_openapi_enabled(&Env::Prod));
    }

    #[test]
    fn test_openapi_disabled_test() {
        assert!(!is_openapi_enabled(&Env::Test));
    }

    #[test]
    fn test_openapi_disabled_local() {
        assert!(!is_openapi_enabled(&Env::Local));
    }

    #[test]
    fn test_openapi_disabled_pr() {
        assert!(!is_openapi_enabled(&Env::Pr));
    }

    #[test]
    fn test_openapi_disabled_nightly() {
        assert!(!is_openapi_enabled(&Env::Nightly));
    }

    #[test]
    fn test_openapi_json_generates() {
        let doc = ApiDoc::openapi();
        let json = serde_json::to_string_pretty(&doc).expect("Should serialize to JSON");
        assert!(json.contains("Collects API"));
        assert!(json.contains("bearer_auth"));
    }
}
