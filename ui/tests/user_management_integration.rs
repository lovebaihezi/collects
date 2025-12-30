//! Integration tests for user management features.
//!
//! These tests verify the complete flow for user management operations:
//! - Get user details (including QR code)
//! - Update username
//! - Delete user
//! - Revoke OTP
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use collects_business::{
    DeleteUserResponse, GetUserResponse, RevokeOtpResponse, UpdateUsernameResponse,
};
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for user management integration tests.
struct UserManagementTestCtx {
    mock_server: MockServer,
}

impl UserManagementTestCtx {
    /// Get reference to the mock server.
    fn mock_server(&self) -> &MockServer {
        &self.mock_server
    }
}

/// Setup test state with mock server configured for user management endpoints.
async fn setup_user_management_test() -> UserManagementTestCtx {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock the health check endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    UserManagementTestCtx { mock_server }
}

// ===========================================
// Tests for GET /api/internal/users/{username}
// ===========================================

/// Test that get user returns user details with QR code info.
#[tokio::test]
async fn test_get_user_returns_qr_code_info() {
    let ctx = setup_user_management_test().await;

    // Mock successful get user response
    Mock::given(method("GET"))
        .and(path("/api/internal/users/testuser"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "testuser",
            "current_otp": "123456",
            "otpauth_url": "otpauth://totp/Collects:testuser?secret=JBSWY3DPEHPK3PXP&issuer=Collects"
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server is ready
    let client = reqwest::Client::new();
    let response = client
        .get(&format!(
            "{}/api/internal/users/testuser",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let get_response: GetUserResponse = response.json().await.expect("Failed to parse response");
    assert_eq!(get_response.username, "testuser");
    assert_eq!(get_response.current_otp, "123456");
    assert!(get_response.otpauth_url.contains("testuser"));
}

/// Test that get user returns 404 for non-existent user.
#[tokio::test]
async fn test_get_user_not_found() {
    let ctx = setup_user_management_test().await;

    // Mock 404 response
    Mock::given(method("GET"))
        .and(path("/api/internal/users/nonexistent"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": "user_not_found",
            "message": "User not found"
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server returns 404
    let client = reqwest::Client::new();
    let response = client
        .get(&format!(
            "{}/api/internal/users/nonexistent",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}

// ===========================================
// Tests for PUT /api/internal/users/{username}
// ===========================================

/// Test that update username succeeds.
#[tokio::test]
async fn test_update_username_success() {
    let ctx = setup_user_management_test().await;

    // Mock successful update username response
    Mock::given(method("PUT"))
        .and(path("/api/internal/users/oldname"))
        .and(body_json(serde_json::json!({
            "new_username": "newname"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "old_username": "oldname",
            "new_username": "newname"
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server is ready
    let client = reqwest::Client::new();
    let response = client
        .put(&format!(
            "{}/api/internal/users/oldname",
            ctx.mock_server.uri()
        ))
        .header("Content-Type", "application/json")
        .body(r#"{"new_username": "newname"}"#)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let update_response: UpdateUsernameResponse =
        response.json().await.expect("Failed to parse response");
    assert_eq!(update_response.old_username, "oldname");
    assert_eq!(update_response.new_username, "newname");
}

/// Test that update username returns conflict for duplicate username.
#[tokio::test]
async fn test_update_username_conflict() {
    let ctx = setup_user_management_test().await;

    // Mock conflict response
    Mock::given(method("PUT"))
        .and(path("/api/internal/users/alice"))
        .and(body_json(serde_json::json!({
            "new_username": "bob"
        })))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "error": "user_already_exists",
            "message": "User 'bob' already exists"
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server returns 409
    let client = reqwest::Client::new();
    let response = client
        .put(&format!(
            "{}/api/internal/users/alice",
            ctx.mock_server.uri()
        ))
        .header("Content-Type", "application/json")
        .body(r#"{"new_username": "bob"}"#)
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 409);
}

// ===========================================
// Tests for DELETE /api/internal/users/{username}
// ===========================================

/// Test that delete user succeeds.
#[tokio::test]
async fn test_delete_user_success() {
    let ctx = setup_user_management_test().await;

    // Mock successful delete response
    Mock::given(method("DELETE"))
        .and(path("/api/internal/users/todelete"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "todelete",
            "deleted": true
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server is ready
    let client = reqwest::Client::new();
    let response = client
        .delete(&format!(
            "{}/api/internal/users/todelete",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let delete_response: DeleteUserResponse =
        response.json().await.expect("Failed to parse response");
    assert_eq!(delete_response.username, "todelete");
    assert!(delete_response.deleted);
}

/// Test that delete user returns false for non-existent user.
#[tokio::test]
async fn test_delete_user_not_found() {
    let ctx = setup_user_management_test().await;

    // Mock response for non-existent user (still 200 but deleted=false)
    Mock::given(method("DELETE"))
        .and(path("/api/internal/users/nonexistent"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "nonexistent",
            "deleted": false
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server is ready
    let client = reqwest::Client::new();
    let response = client
        .delete(&format!(
            "{}/api/internal/users/nonexistent",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let delete_response: DeleteUserResponse =
        response.json().await.expect("Failed to parse response");
    assert_eq!(delete_response.username, "nonexistent");
    assert!(!delete_response.deleted);
}

// ===========================================
// Tests for POST /api/internal/users/{username}/revoke
// ===========================================

/// Test that revoke OTP succeeds and returns new QR code.
#[tokio::test]
async fn test_revoke_otp_success() {
    let ctx = setup_user_management_test().await;

    // Mock successful revoke response
    Mock::given(method("POST"))
        .and(path("/api/internal/users/alice/revoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "alice",
            "secret": "NEWJBSWY3DPEHPK3PXP",
            "otpauth_url": "otpauth://totp/Collects:alice?secret=NEWJBSWY3DPEHPK3PXP&issuer=Collects"
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server is ready
    let client = reqwest::Client::new();
    let response = client
        .post(&format!(
            "{}/api/internal/users/alice/revoke",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 200);

    let revoke_response: RevokeOtpResponse =
        response.json().await.expect("Failed to parse response");
    assert_eq!(revoke_response.username, "alice");
    assert_eq!(revoke_response.secret, "NEWJBSWY3DPEHPK3PXP");
    assert!(revoke_response.otpauth_url.contains("alice"));
}

/// Test that revoke OTP returns 404 for non-existent user.
#[tokio::test]
async fn test_revoke_otp_user_not_found() {
    let ctx = setup_user_management_test().await;

    // Mock 404 response
    Mock::given(method("POST"))
        .and(path("/api/internal/users/nonexistent/revoke"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": "user_not_found",
            "message": "User not found"
        })))
        .mount(ctx.mock_server())
        .await;

    // Verify mock server returns 404
    let client = reqwest::Client::new();
    let response = client
        .post(&format!(
            "{}/api/internal/users/nonexistent/revoke",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(response.status(), 404);
}

// ===========================================
// Full integration test for user management flow
// ===========================================

/// Test the complete user management flow:
/// 1. Create user
/// 2. Get user details
/// 3. Update username
/// 4. Revoke OTP
/// 5. Delete user
#[tokio::test]
async fn test_complete_user_management_flow() {
    let ctx = setup_user_management_test().await;

    let client = reqwest::Client::new();

    // Step 1: Create user
    Mock::given(method("POST"))
        .and(path("/api/internal/users"))
        .and(body_json(serde_json::json!({
            "username": "flowuser"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({
            "username": "flowuser",
            "secret": "JBSWY3DPEHPK3PXP",
            "otpauth_url": "otpauth://totp/Collects:flowuser?secret=JBSWY3DPEHPK3PXP&issuer=Collects"
        })))
        .mount(ctx.mock_server())
        .await;

    let response = client
        .post(&format!("{}/api/internal/users", ctx.mock_server.uri()))
        .header("Content-Type", "application/json")
        .body(r#"{"username": "flowuser"}"#)
        .send()
        .await
        .expect("Failed to create user");
    assert_eq!(response.status(), 201);

    // Step 2: Get user details
    Mock::given(method("GET"))
        .and(path("/api/internal/users/flowuser"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "flowuser",
            "current_otp": "123456",
            "otpauth_url": "otpauth://totp/Collects:flowuser?secret=JBSWY3DPEHPK3PXP&issuer=Collects"
        })))
        .mount(ctx.mock_server())
        .await;

    let response = client
        .get(&format!(
            "{}/api/internal/users/flowuser",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to get user");
    assert_eq!(response.status(), 200);

    // Step 3: Update username
    Mock::given(method("PUT"))
        .and(path("/api/internal/users/flowuser"))
        .and(body_json(serde_json::json!({
            "new_username": "renameduser"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "old_username": "flowuser",
            "new_username": "renameduser"
        })))
        .mount(ctx.mock_server())
        .await;

    let response = client
        .put(&format!(
            "{}/api/internal/users/flowuser",
            ctx.mock_server.uri()
        ))
        .header("Content-Type", "application/json")
        .body(r#"{"new_username": "renameduser"}"#)
        .send()
        .await
        .expect("Failed to update username");
    assert_eq!(response.status(), 200);

    // Step 4: Revoke OTP
    Mock::given(method("POST"))
        .and(path("/api/internal/users/renameduser/revoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "renameduser",
            "secret": "NEWJBSWY3DPEHPK3PXP",
            "otpauth_url": "otpauth://totp/Collects:renameduser?secret=NEWJBSWY3DPEHPK3PXP&issuer=Collects"
        })))
        .mount(ctx.mock_server())
        .await;

    let response = client
        .post(&format!(
            "{}/api/internal/users/renameduser/revoke",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to revoke OTP");
    assert_eq!(response.status(), 200);

    // Step 5: Delete user
    Mock::given(method("DELETE"))
        .and(path("/api/internal/users/renameduser"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "renameduser",
            "deleted": true
        })))
        .mount(ctx.mock_server())
        .await;

    let response = client
        .delete(&format!(
            "{}/api/internal/users/renameduser",
            ctx.mock_server.uri()
        ))
        .send()
        .await
        .expect("Failed to delete user");
    assert_eq!(response.status(), 200);
}
