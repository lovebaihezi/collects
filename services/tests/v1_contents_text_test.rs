//! Integration tests for text content creation API (POST /v1/contents).

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use collects_services::users::storage::MockUserStorage;
use common::{MockSqlStorage, create_test_app, generate_test_token};
use tower::ServiceExt;

#[tokio::test]
async fn test_create_text_without_auth_returns_401() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::new();
    let app = create_test_app(sql_storage, user_storage).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title":"My Note","body":"Hello, world!"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_create_text_success() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(
                    r#"{"title":"My Note","body":"Hello, world!","content_type":"text/plain"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(json["content"]["id"].is_string());
    assert_eq!(json["content"]["title"], "My Note");
    assert_eq!(json["content"]["kind"], "text");
    assert_eq!(json["content"]["body"], "Hello, world!");
    assert_eq!(json["content"]["content_type"], "text/plain");
    assert_eq!(json["content"]["storage_backend"], "inline");
    assert_eq!(json["content"]["file_size"], 13); // "Hello, world!".len()
}

#[tokio::test]
async fn test_create_text_with_markdown() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    let payload = serde_json::json!({
        "title": "README",
        "body": "# Hello World",
        "content_type": "text/markdown"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["content"]["content_type"], "text/markdown");
    assert_eq!(json["content"]["kind"], "text");
}

#[tokio::test]
async fn test_create_text_invalid_content_type() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(
                    r#"{"title":"Binary","body":"data","content_type":"application/octet-stream"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "bad_request");
    assert!(json["message"].as_str().unwrap().contains("text/*"));
}

#[tokio::test]
async fn test_create_text_invalid_visibility() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(
                    r#"{"title":"Note","body":"text","visibility":"invalid"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["error"], "bad_request");
    assert!(
        json["message"]
            .as_str()
            .unwrap()
            .contains("Invalid visibility")
    );
}

#[tokio::test]
async fn test_create_text_default_content_type() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    // Request without content_type - should default to text/plain
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(r#"{"title":"Note","body":"Some text"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["content"]["content_type"], "text/plain");
}

#[tokio::test]
async fn test_create_text_with_description() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    let payload = serde_json::json!({
        "title": "My Note",
        "body": "Note content here",
        "description": "A test note with description"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        json["content"]["description"],
        "A test note with description"
    );
}

#[tokio::test]
async fn test_create_text_with_visibility() {
    let sql_storage = MockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let app = create_test_app(sql_storage, user_storage).await;

    let token = generate_test_token();

    let payload = serde_json::json!({
        "title": "Public Note",
        "body": "This is public",
        "visibility": "public"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents")
                .header("content-type", "application/json")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["content"]["visibility"], "public");
}
