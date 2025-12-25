//! Authentication middleware for protecting internal API routes.
//! 
//! This module provides middleware to validate authentication headers
//! passed from the Cloudflare Worker after validating CF Access JWT tokens.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

/// User information extracted from authentication headers
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub email: String,
    pub name: Option<String>,
}

/// Extract authenticated user information from request headers
pub fn extract_auth_user(request: &Request) -> Option<AuthUser> {
    let headers = request.headers();
    
    let user_id = headers.get("X-Auth-User-Id")?.to_str().ok()?.to_string();
    let email = headers.get("X-Auth-User-Email")?.to_str().ok()?.to_string();
    let name = headers
        .get("X-Auth-User-Name")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Validate that required fields are non-empty
    if user_id.is_empty() || email.is_empty() {
        return None;
    }

    // Basic email format validation
    if !email.contains('@') {
        return None;
    }

    Some(AuthUser {
        user_id,
        email,
        name,
    })
}

/// Middleware to require authentication for internal API routes
pub async fn require_auth(request: Request, next: Next) -> Response {
    if extract_auth_user(&request).is_some() {
        next.run(request).await
    } else {
        (StatusCode::UNAUTHORIZED, "Unauthorized").into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_extract_auth_user_with_valid_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-User-Id", HeaderValue::from_static("user123"));
        headers.insert(
            "X-Auth-User-Email",
            HeaderValue::from_static("user@example.com"),
        );
        headers.insert("X-Auth-User-Name", HeaderValue::from_static("John Doe"));

        let request = Request::builder()
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let user = extract_auth_user(&request);
        assert!(user.is_some());

        let user = user.unwrap();
        assert_eq!(user.user_id, "user123");
        assert_eq!(user.email, "user@example.com");
        assert_eq!(user.name, Some("John Doe".to_string()));
    }

    #[test]
    fn test_extract_auth_user_without_name() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-User-Id", HeaderValue::from_static("user123"));
        headers.insert(
            "X-Auth-User-Email",
            HeaderValue::from_static("user@example.com"),
        );

        let request = Request::builder()
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let user = extract_auth_user(&request);
        assert!(user.is_some());

        let user = user.unwrap();
        assert_eq!(user.user_id, "user123");
        assert_eq!(user.email, "user@example.com");
        assert_eq!(user.name, None);
    }

    #[test]
    fn test_extract_auth_user_missing_headers() {
        let headers = HeaderMap::new();

        let request = Request::builder()
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let user = extract_auth_user(&request);
        assert!(user.is_none());
    }

    #[test]
    fn test_extract_auth_user_partial_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-User-Id", HeaderValue::from_static("user123"));
        // Missing email header

        let request = Request::builder()
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let user = extract_auth_user(&request);
        assert!(user.is_none());
    }

    #[test]
    fn test_extract_auth_user_empty_values() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-User-Id", HeaderValue::from_static(""));
        headers.insert("X-Auth-User-Email", HeaderValue::from_static("user@example.com"));

        let request = Request::builder()
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let user = extract_auth_user(&request);
        assert!(user.is_none());
    }

    #[test]
    fn test_extract_auth_user_invalid_email() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Auth-User-Id", HeaderValue::from_static("user123"));
        headers.insert("X-Auth-User-Email", HeaderValue::from_static("invalid-email"));

        let request = Request::builder()
            .uri("/test")
            .body(axum::body::Body::empty())
            .unwrap();

        let mut request = request;
        *request.headers_mut() = headers;

        let user = extract_auth_user(&request);
        assert!(user.is_none());
    }
}
