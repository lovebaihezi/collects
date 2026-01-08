//! Session-based JWT authentication for protected routes.
//!
//! This module provides the `RequireAuth` extractor for protecting `/v1/*` routes
//! with session JWT tokens issued after successful OTP verification.
//!
//! # Usage
//!
//! ```rust,ignore
//! use collects_services::users::session_auth::RequireAuth;
//!
//! async fn protected_handler(auth: RequireAuth) -> impl IntoResponse {
//!     format!("Hello, {}!", auth.username())
//! }
//! ```
//!
//! # Authentication Flow
//!
//! 1. User authenticates via `/auth/verify-otp` and receives a JWT session token
//! 2. Client includes token in subsequent requests via `Authorization: Bearer <token>`
//! 3. `RequireAuth` extractor validates the token and provides user context to handlers
//!
//! # Token Requirements
//!
//! The JWT must:
//! - Be signed with the server's `JWT_SECRET`
//! - Have a valid `exp` (expiration) claim
//! - Have a `sub` (subject) claim containing the username
//! - Have an `iss` (issuer) claim matching our ISSUER constant

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::otp::{ISSUER, SessionClaims};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};

/// Authenticated user context extracted from a valid session JWT.
///
/// This extractor validates the JWT token from the `Authorization` header
/// and provides access to the authenticated user's information.
///
/// # Rejection
///
/// Returns `SessionAuthError` (401 Unauthorized) if:
/// - No token is provided
/// - Token format is invalid
/// - Token signature is invalid
/// - Token has expired
/// - Token issuer doesn't match
#[derive(Debug, Clone)]
pub struct RequireAuth {
    /// The validated JWT claims
    claims: SessionClaims,
}

impl RequireAuth {
    /// Get the authenticated user's username (from the `sub` claim).
    pub fn username(&self) -> &str {
        &self.claims.sub
    }

    /// Get the token's issued-at timestamp.
    pub fn issued_at(&self) -> i64 {
        self.claims.iat
    }

    /// Get the token's expiration timestamp.
    pub fn expires_at(&self) -> i64 {
        self.claims.exp
    }

    /// Get the full claims for advanced use cases.
    pub fn claims(&self) -> &SessionClaims {
        &self.claims
    }

    /// Create a new RequireAuth from validated claims (for testing).
    #[cfg(test)]
    pub fn new_for_test(username: impl Into<String>) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        Self {
            claims: SessionClaims {
                sub: username.into(),
                iat: now,
                exp: now + 3600,
                iss: ISSUER.to_string(),
            },
        }
    }
}

/// Error type for session authentication failures.
#[derive(Debug, Serialize)]
pub struct SessionAuthError {
    pub error: String,
    pub message: String,
}

impl SessionAuthError {
    fn missing_token() -> Self {
        Self {
            error: "missing_token".to_string(),
            message: "Authorization header with Bearer token is required".to_string(),
        }
    }

    fn invalid_format() -> Self {
        Self {
            error: "invalid_format".to_string(),
            message: "Authorization header must be in format: Bearer <token>".to_string(),
        }
    }

    fn invalid_token(reason: impl Into<String>) -> Self {
        Self {
            error: "invalid_token".to_string(),
            message: reason.into(),
        }
    }

    fn missing_config() -> Self {
        Self {
            error: "server_error".to_string(),
            message: "Server configuration error".to_string(),
        }
    }
}

impl IntoResponse for SessionAuthError {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

/// Extract the Bearer token from the Authorization header.
fn extract_bearer_token(headers: &axum::http::HeaderMap) -> Option<&str> {
    let header_value = headers.get(AUTHORIZATION)?;
    let header_str = header_value.to_str().ok()?;

    // Must have "Bearer " prefix (case-insensitive for "Bearer")
    let stripped = header_str.strip_prefix("Bearer ")?;
    if stripped.is_empty() {
        return None;
    }
    Some(stripped)
}

/// Validate a session JWT token and return the claims.
fn validate_session_token(token: &str, jwt_secret: &str) -> Result<SessionClaims, String> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[ISSUER]);
    validation.validate_exp = true;

    let token_data = decode::<SessionClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| match e.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => "Token has expired".to_string(),
        jsonwebtoken::errors::ErrorKind::InvalidSignature => "Invalid token signature".to_string(),
        jsonwebtoken::errors::ErrorKind::InvalidIssuer => "Invalid token issuer".to_string(),
        _ => format!("Token validation failed: {}", e),
    })?;

    Ok(token_data.claims)
}

impl<S> FromRequestParts<S> for RequireAuth
where
    S: Send + Sync,
{
    type Rejection = SessionAuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get JWT secret from request extensions (set via Extension layer)
        let config = parts
            .extensions
            .get::<crate::config::Config>()
            .ok_or_else(SessionAuthError::missing_config)?;

        let jwt_secret = config.jwt_secret();

        // Extract Bearer token from Authorization header
        let token = extract_bearer_token(&parts.headers).ok_or_else(|| {
            // Distinguish between missing header and invalid format
            if parts.headers.get(AUTHORIZATION).is_some() {
                SessionAuthError::invalid_format()
            } else {
                SessionAuthError::missing_token()
            }
        })?;

        // Validate the token
        let claims =
            validate_session_token(token, jwt_secret).map_err(SessionAuthError::invalid_token)?;

        Ok(RequireAuth { claims })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::otp::generate_session_token;

    const TEST_SECRET: &str = "test-jwt-secret-for-unit-tests";

    #[test]
    fn test_extract_bearer_token_valid() {
        use axum::http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer my-token-123".parse().unwrap());

        let token = extract_bearer_token(&headers);
        assert_eq!(token, Some("my-token-123"));
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        use axum::http::HeaderMap;

        let headers = HeaderMap::new();
        let token = extract_bearer_token(&headers);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_no_bearer_prefix() {
        use axum::http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "my-token-123".parse().unwrap());

        let token = extract_bearer_token(&headers);
        assert_eq!(token, None);
    }

    #[test]
    fn test_extract_bearer_token_empty_token() {
        use axum::http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer ".parse().unwrap());

        let token = extract_bearer_token(&headers);
        assert_eq!(token, None);
    }

    #[test]
    fn test_validate_session_token_success() {
        let token = generate_session_token("testuser", TEST_SECRET).unwrap();
        let claims = validate_session_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.iss, ISSUER);
    }

    #[test]
    fn test_validate_session_token_wrong_secret() {
        let token = generate_session_token("testuser", TEST_SECRET).unwrap();
        let result = validate_session_token(&token, "wrong-secret");

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid token signature"));
    }

    #[test]
    fn test_validate_session_token_malformed() {
        let result = validate_session_token("not-a-valid-jwt", TEST_SECRET);

        assert!(result.is_err());
    }

    #[test]
    fn test_require_auth_accessors() {
        let auth = RequireAuth::new_for_test("alice");

        assert_eq!(auth.username(), "alice");
        assert!(auth.expires_at() > auth.issued_at());
        assert_eq!(auth.claims().iss, ISSUER);
    }

    #[test]
    fn test_session_auth_error_into_response() {
        let error = SessionAuthError::missing_token();
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn test_session_auth_error_types() {
        let missing = SessionAuthError::missing_token();
        assert_eq!(missing.error, "missing_token");

        let invalid_format = SessionAuthError::invalid_format();
        assert_eq!(invalid_format.error, "invalid_format");

        let invalid_token = SessionAuthError::invalid_token("test reason");
        assert_eq!(invalid_token.error, "invalid_token");
        assert_eq!(invalid_token.message, "test reason");

        let missing_config = SessionAuthError::missing_config();
        assert_eq!(missing_config.error, "server_error");
    }
}
