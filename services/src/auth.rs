//! Cloudflare Zero Trust authentication middleware.
//!
//! This module provides authentication middleware for protecting routes with
//! Cloudflare Access (Zero Trust) JWT tokens. It follows Axum best practices
//! for middleware and extractors.

use axum::{
    extract::{FromRequestParts, Request},
    http::{header::AUTHORIZATION, request::Parts, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Configuration for Cloudflare Zero Trust authentication.
#[derive(Clone, Debug)]
pub struct ZeroTrustConfig {
    /// Cloudflare Access team domain (e.g., "myteam.cloudflareaccess.com")
    pub team_domain: String,
    /// Cloudflare Access application audience tag
    pub audience: String,
}

impl ZeroTrustConfig {
    /// Create a new Zero Trust configuration
    pub fn new(team_domain: String, audience: String) -> Self {
        Self {
            team_domain,
            audience,
        }
    }

    /// Get the JWKS URL for the team domain
    pub fn jwks_url(&self) -> String {
        format!("https://{}/cdn-cgi/access/certs", self.team_domain)
    }
}

/// Claims from a Cloudflare Access JWT token
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessClaims {
    /// JWT issuer (should be the team domain)
    pub iss: String,
    /// JWT audience (should match the configured audience)
    pub aud: Vec<String>,
    /// JWT expiration time
    pub exp: i64,
    /// JWT issued at time
    pub iat: i64,
    /// Subject (user email or ID)
    pub sub: String,
    /// User email
    #[serde(default)]
    pub email: Option<String>,
    /// Custom claims
    #[serde(flatten)]
    pub custom: serde_json::Value,
}

/// Extractor for validated Cloudflare Access tokens.
///
/// Use this in route handlers that require authentication:
///
/// ```rust,ignore
/// async fn protected_handler(auth: ZeroTrustAuth) -> impl IntoResponse {
///     format!("Hello, {}!", auth.claims.email.unwrap_or_default())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ZeroTrustAuth {
    /// Validated JWT claims
    pub claims: AccessClaims,
}

impl ZeroTrustAuth {
    /// Get the user's email from the token
    pub fn email(&self) -> Option<&str> {
        self.claims.email.as_deref()
    }

    /// Get the user's subject (ID)
    pub fn subject(&self) -> &str {
        &self.claims.sub
    }
}

/// Error type for authentication failures
#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

/// Middleware function to validate Cloudflare Access JWT tokens.
///
/// This middleware extracts and validates the JWT token from the
/// `CF-Authorization` or `Authorization` header.
pub async fn zero_trust_middleware(
    config: Arc<ZeroTrustConfig>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract token from headers
    let token = extract_token_from_headers(request.headers())
        .ok_or_else(|| AuthError {
            error: "missing_token".to_string(),
            message: "No authentication token provided".to_string(),
        })?;

    // Validate token
    let claims = validate_token(token, &config).await.map_err(|e| AuthError {
        error: "invalid_token".to_string(),
        message: format!("Token validation failed: {}", e),
    })?;

    // Insert claims into request extensions for later use
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

/// Extract token from CF-Authorization or Authorization header
fn extract_token_from_headers(headers: &axum::http::HeaderMap) -> Option<&str> {
    // Try CF-Authorization header first (Cloudflare specific)
    if let Some(header_value) = headers.get("cf-authorization") {
        return header_value.to_str().ok();
    }

    // Fall back to standard Authorization header
    if let Some(header_value) = headers.get(AUTHORIZATION) {
        let header_str = header_value.to_str().ok()?;
        // Remove "Bearer " prefix if present
        if header_str.starts_with("Bearer ") {
            return Some(&header_str[7..]);
        }
        return Some(header_str);
    }

    None
}

/// Validate a Cloudflare Access JWT token
async fn validate_token(token: &str, config: &ZeroTrustConfig) -> Result<AccessClaims, String> {
    // Decode header to get key ID
    let header = decode_header(token).map_err(|e| format!("Failed to decode JWT header: {}", e))?;

    let kid = header.kid.ok_or("JWT header missing kid field")?;

    // Fetch public keys from Cloudflare
    let public_key = fetch_public_key(&config.jwks_url(), &kid)
        .await
        .map_err(|e| format!("Failed to fetch public key: {}", e))?;

    // Set up validation parameters
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&config.audience]);
    validation.set_issuer(&[format!("https://{}", config.team_domain)]);

    // Validate and decode token
    let token_data = decode::<AccessClaims>(token, &public_key, &validation)
        .map_err(|e| format!("Failed to validate token: {}", e))?;

    Ok(token_data.claims)
}

/// JWKS key structure
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct JwksKey {
    kid: String,
    n: String,
    e: String,
    #[serde(default)]
    alg: Option<String>,
    #[serde(default)]
    kty: Option<String>,
}

/// JWKS response structure
#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwksKey>,
}

/// Fetch public key from Cloudflare's JWKS endpoint
async fn fetch_public_key(jwks_url: &str, kid: &str) -> Result<DecodingKey, String> {
    // Fetch JWKS from Cloudflare
    let response = reqwest::get(jwks_url)
        .await
        .map_err(|e| format!("Failed to fetch JWKS: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("JWKS endpoint returned status: {}", response.status()));
    }

    let jwks: JwksResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JWKS: {}", e))?;

    // Find the key with matching kid
    let key = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or(format!("Key with kid '{}' not found in JWKS", kid))?;

    // Create decoding key from RSA components
    DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|e| format!("Failed to create decoding key: {}", e))
}

/// Extractor implementation for ZeroTrustAuth
///
/// This allows using ZeroTrustAuth directly as a parameter in route handlers
/// after the middleware has validated the token.
impl<S> FromRequestParts<S> for ZeroTrustAuth
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Extract claims from request extensions (set by middleware)
        let claims = parts
            .extensions
            .get::<AccessClaims>()
            .ok_or_else(|| AuthError {
                error: "missing_auth".to_string(),
                message: "Authentication required but no valid token found".to_string(),
            })?
            .clone();

        Ok(ZeroTrustAuth { claims })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_trust_config() {
        let config = ZeroTrustConfig::new(
            "myteam.cloudflareaccess.com".to_string(),
            "test-audience".to_string(),
        );
        assert_eq!(
            config.jwks_url(),
            "https://myteam.cloudflareaccess.com/cdn-cgi/access/certs"
        );
    }

    #[test]
    fn test_extract_token_from_headers() {
        use axum::http::HeaderMap;

        let mut headers = HeaderMap::new();
        headers.insert("cf-authorization", "test-token".parse().unwrap());

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("test-token"));

        // Test Authorization header with Bearer prefix
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "Bearer test-token-2".parse().unwrap());

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("test-token-2"));

        // Test Authorization header without Bearer prefix
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, "test-token-3".parse().unwrap());

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("test-token-3"));

        // Test missing header
        let headers = HeaderMap::new();
        let token = extract_token_from_headers(&headers);
        assert_eq!(token, None);
    }

    #[test]
    fn test_auth_error_response() {
        let error = AuthError {
            error: "test_error".to_string(),
            message: "Test error message".to_string(),
        };

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
