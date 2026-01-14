//! Cloudflare Zero Trust authentication middleware.
//!
//! This module provides authentication middleware for protecting routes with
//! Cloudflare Access (Zero Trust) JWT tokens. It follows Axum best practices
//! for middleware and extractors.

use axum::{
    Json,
    extract::{FromRequestParts, Request},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Trait for resolving a JWT decoding key from the configured JWKS endpoint.
///
/// This is injectable so tests can provide a deterministic in-memory resolver
/// (no external network calls) while production uses HTTP fetching from Cloudflare.
pub trait JwksKeyResolver: Send + Sync + 'static {
    /// Resolve a decoding key for the given `kid` from the JWKS endpoint.
    ///
    /// This returns a `'static` future by taking owned inputs, which makes the trait
    /// object-safe and avoids lifetime issues when boxing the future.
    fn resolve_decoding_key(
        &self,
        jwks_url: String,
        kid: String,
    ) -> Pin<Box<dyn Future<Output = Result<DecodingKey, String>> + Send + 'static>>;
}

/// Default resolver that fetches JWKS over HTTP using `reqwest`.
#[derive(Debug, Default, Clone)]
pub struct ReqwestJwksKeyResolver;

impl JwksKeyResolver for ReqwestJwksKeyResolver {
    fn resolve_decoding_key(
        &self,
        jwks_url: String,
        kid: String,
    ) -> Pin<Box<dyn Future<Output = Result<DecodingKey, String>> + Send + 'static>> {
        Box::pin(async move { fetch_public_key(&jwks_url, &kid).await })
    }
}

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
    request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    zero_trust_middleware_with_resolver(Arc::new(ReqwestJwksKeyResolver), config, request, next)
        .await
}

/// Middleware function to validate Cloudflare Access JWT tokens using an injectable resolver.
///
/// Tests should call this variant with an in-memory resolver to avoid external network calls.
pub async fn zero_trust_middleware_with_resolver(
    resolver: Arc<dyn JwksKeyResolver>,
    config: Arc<ZeroTrustConfig>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract token from headers
    let token = extract_token_from_headers(request.headers()).ok_or_else(|| AuthError {
        error: "missing_token".to_owned(),
        message: "No authentication token provided".to_owned(),
    })?;

    // Validate token
    let claims = validate_token_with_resolver(token, &config, resolver.as_ref())
        .await
        .map_err(|e| AuthError {
            error: "invalid_token".to_owned(),
            message: format!("Token validation failed: {e}"),
        })?;

    // Insert claims into request extensions for later use
    request.extensions_mut().insert(claims);

    Ok(next.run(request).await)
}

/// Extract token from CF-Authorization header, Authorization header, or `CF_Authorization` cookie.
///
/// This supports multiple authentication flows:
/// 1. `cf-authorization` header - Cloudflare Access standard header
/// 2. `CF-Access-Jwt-Assertion` header - Standard CF Access JWT header (used by Workers)
/// 3. `Authorization: Bearer <token>` header - Standard OAuth-style header
/// 4. `CF_Authorization` cookie - Set automatically by Cloudflare Zero Trust in browsers
fn extract_token_from_headers(headers: &axum::http::HeaderMap) -> Option<&str> {
    // Try CF-Authorization header first (Cloudflare specific)
    if let Some(header_value) = headers.get("cf-authorization") {
        return header_value.to_str().ok();
    }

    // Try CF-Access-Jwt-Assertion header (standard CF Access header, often used by Workers)
    if let Some(header_value) = headers.get("cf-access-jwt-assertion") {
        return header_value.to_str().ok();
    }

    // Fall back to standard Authorization header
    if let Some(header_value) = headers.get(AUTHORIZATION) {
        let header_str = header_value.to_str().ok()?;
        // Remove "Bearer " prefix if present
        if let Some(stripped) = header_str.strip_prefix("Bearer ") {
            return Some(stripped);
        }
        return Some(header_str);
    }

    // Finally, try CF_Authorization cookie (set by Cloudflare Zero Trust in browsers)
    if let Some(cookie_header) = headers.get("cookie")
        && let Ok(cookie_str) = cookie_header.to_str()
    {
        for cookie in cookie_str.split(';') {
            let cookie = cookie.trim();
            if let Some(token) = cookie.strip_prefix("CF_Authorization=") {
                return Some(token);
            }
        }
    }

    None
}

/// Validate a Cloudflare Access JWT token using an injectable JWKS resolver.
async fn validate_token_with_resolver(
    token: &str,
    config: &ZeroTrustConfig,
    resolver: &dyn JwksKeyResolver,
) -> Result<AccessClaims, String> {
    // Decode header to get key ID
    let header = decode_header(token).map_err(|e| format!("Failed to decode JWT header: {e}"))?;

    let kid = header.kid.ok_or("JWT header missing kid field")?;

    // Resolve decoding key (injectable for tests)
    let public_key = resolver
        .resolve_decoding_key(config.jwks_url(), kid)
        .await
        .map_err(|e| format!("Failed to fetch public key: {e}"))?;

    // Set up validation parameters
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&config.audience]);
    validation.set_issuer(&[format!("https://{}", config.team_domain)]);

    // Validate and decode token
    let token_data = decode::<AccessClaims>(token, &public_key, &validation)
        .map_err(|e| format!("Failed to validate token: {e}"))?;

    Ok(token_data.claims)
}

/// JWKS key structure
#[derive(Debug, Deserialize)]
struct JwksKey {
    kid: String,
    n: String,
    e: String,
    /// Algorithm field - unused but part of JWKS spec
    #[serde(default)]
    #[expect(dead_code)]
    alg: Option<String>,
    /// Key type field - unused but part of JWKS spec
    #[serde(default)]
    #[expect(dead_code)]
    kty: Option<String>,
}

/// JWKS response structure
#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwksKey>,
}

/// Fetch public key from Cloudflare's JWKS endpoint
///
/// TODO: Implement caching for JWKS keys to reduce external calls and improve performance.
/// Consider using a cache with TTL (e.g., 1 hour) to avoid fetching keys on every request.
/// This would improve response times and reduce dependency on Cloudflare's JWKS endpoint.
async fn fetch_public_key(jwks_url: &str, kid: &str) -> Result<DecodingKey, String> {
    // Fetch JWKS from Cloudflare
    let response = reqwest::get(jwks_url)
        .await
        .map_err(|e| format!("Failed to fetch JWKS: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "JWKS endpoint returned status: {}",
            response.status()
        ));
    }

    let jwks: JwksResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse JWKS: {e}"))?;

    // Find the key with matching kid
    let key = jwks
        .keys
        .iter()
        .find(|k| k.kid == kid)
        .ok_or(format!("Key with kid '{kid}' not found in JWKS"))?;

    // Create decoding key from RSA components
    DecodingKey::from_rsa_components(&key.n, &key.e)
        .map_err(|e| format!("Failed to create decoding key: {e}"))
}

/// Extractor implementation for `ZeroTrustAuth`
///
/// This allows using `ZeroTrustAuth` directly as a parameter in route handlers
/// after the middleware has validated the token.
impl<S> FromRequestParts<S> for ZeroTrustAuth
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract claims from request extensions (set by middleware)
        let claims = parts
            .extensions
            .get::<AccessClaims>()
            .ok_or_else(|| AuthError {
                error: "missing_auth".to_owned(),
                message: "Authentication required but no valid token found".to_owned(),
            })?
            .clone();

        Ok(Self { claims })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_trust_config() {
        let config = ZeroTrustConfig::new(
            "myteam.cloudflareaccess.com".to_owned(),
            "test-audience".to_owned(),
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

        // Test CF-Access-Jwt-Assertion header (used by Workers)
        let mut headers = HeaderMap::new();
        headers.insert("cf-access-jwt-assertion", "worker-token".parse().unwrap());

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("worker-token"));

        // Test CF_Authorization cookie
        let mut headers = HeaderMap::new();
        headers.insert("cookie", "CF_Authorization=cookie-token".parse().unwrap());

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("cookie-token"));

        // Test CF_Authorization cookie with multiple cookies
        let mut headers = HeaderMap::new();
        headers.insert(
            "cookie",
            "other=value; CF_Authorization=multi-cookie-token; another=thing"
                .parse()
                .unwrap(),
        );

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("multi-cookie-token"));

        // Test header takes precedence over cookie
        let mut headers = HeaderMap::new();
        headers.insert("cf-authorization", "header-token".parse().unwrap());
        headers.insert("cookie", "CF_Authorization=cookie-token".parse().unwrap());

        let token = extract_token_from_headers(&headers);
        assert_eq!(token, Some("header-token"));
    }

    #[test]
    fn test_auth_error_response() {
        let error = AuthError {
            error: "test_error".to_owned(),
            message: "Test error message".to_owned(),
        };

        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
