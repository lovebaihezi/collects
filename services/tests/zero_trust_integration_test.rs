//! Integration tests for Cloudflare Zero Trust authentication

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use collects_services::{auth::ZeroTrustConfig, config::Config, database::SqlStorage, routes};
use tower::ServiceExt;

#[derive(Clone)]
struct MockStorage {
    is_connected: bool,
}

impl SqlStorage for MockStorage {
    async fn is_connected(&self) -> bool {
        self.is_connected
    }
}

#[tokio::test]
async fn test_internal_route_without_zerotrust_config() {
    // When Zero Trust is not configured, routes should be accessible
    let storage = MockStorage { is_connected: true };
    let config = Config::new_for_test();
    let app = routes(storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "testuser"}"#))
                .expect("Failed to create request"),
        )
        .await
        .expect("Failed to get response");

    // Should succeed (CREATED) since no auth is required in test mode
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_auth_route_always_accessible() {
    // Auth routes should always be accessible without Zero Trust
    let storage = MockStorage { is_connected: true };
    let config = Config::new_for_test();
    let app = routes(storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/verify-otp")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username": "testuser", "code": "123456"}"#))
                .expect("Failed to create request"),
        )
        .await
        .expect("Failed to get response");

    // Should return NOT_IMPLEMENTED (as per current implementation)
    // but not UNAUTHORIZED, proving auth routes don't require Zero Trust
    assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn test_health_check_always_accessible() {
    // Health check should always be accessible
    let storage = MockStorage { is_connected: true };
    let config = Config::new_for_test();
    let app = routes(storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/is-health")
                .body(Body::empty())
                .expect("Failed to create request"),
        )
        .await
        .expect("Failed to get response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_zero_trust_config_creation() {
    let config = ZeroTrustConfig::new(
        "myteam.cloudflareaccess.com".to_string(),
        "test-aud-123".to_string(),
    );

    assert_eq!(config.team_domain, "myteam.cloudflareaccess.com");
    assert_eq!(config.audience, "test-aud-123");
    assert_eq!(
        config.jwks_url(),
        "https://myteam.cloudflareaccess.com/cdn-cgi/access/certs"
    );
}
