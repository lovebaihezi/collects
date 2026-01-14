//! Integration tests for Share Links API endpoints.
//!
//! Tests cover:
//! - Share links CRUD operations (list, create, get, update, delete)
//! - Content/group share link attachment
//! - Public share access endpoints
//! - Authentication requirements
//! - Error handling

mod common;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use collects_services::{
    config::Config,
    database::{
        ContentGroupRow, ContentRow, ShareLinkCreate, ShareLinkRow, ShareLinkUpdate, SqlStorage,
        SqlStorageError,
    },
    routes,
    users::storage::MockUserStorage,
};
use common::{MockSqlStorage, TEST_USER_ID, generate_test_token};
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};
use tower::ServiceExt;

// =============================================================================
// Test Helpers
// =============================================================================

/// A MockSqlStorage that can return share links for testing.
#[derive(Clone)]
struct ShareLinksMockSqlStorage {
    inner: MockSqlStorage,
    share_links: Arc<RwLock<Vec<ShareLinkRow>>>,
    contents: Arc<RwLock<Vec<ContentRow>>>,
    groups: Arc<RwLock<Vec<ContentGroupRow>>>,
    content_shares: Arc<RwLock<Vec<(uuid::Uuid, uuid::Uuid)>>>, // (content_id, share_link_id)
    group_shares: Arc<RwLock<Vec<(uuid::Uuid, uuid::Uuid)>>>,   // (group_id, share_link_id)
}

impl ShareLinksMockSqlStorage {
    fn new() -> Self {
        Self {
            inner: MockSqlStorage::with_user_id(TEST_USER_ID),
            share_links: Arc::new(RwLock::new(vec![])),
            contents: Arc::new(RwLock::new(vec![])),
            groups: Arc::new(RwLock::new(vec![])),
            content_shares: Arc::new(RwLock::new(vec![])),
            group_shares: Arc::new(RwLock::new(vec![])),
        }
    }

    fn with_share_link(self, share_link: ShareLinkRow) -> Self {
        self.share_links.write().unwrap().push(share_link);
        self
    }

    fn with_content(self, content: ContentRow) -> Self {
        self.contents.write().unwrap().push(content);
        self
    }

    fn with_group(self, group: ContentGroupRow) -> Self {
        self.groups.write().unwrap().push(group);
        self
    }

    fn create_test_share_link(owner_id: uuid::Uuid, token: &str, permission: &str) -> ShareLinkRow {
        ShareLinkRow {
            id: uuid::Uuid::new_v4(),
            owner_id,
            token: token.to_owned(),
            name: Some("Test Share".to_owned()),
            permission: permission.to_owned(),
            password_hash: None,
            max_access_count: None,
            access_count: 0,
            expires_at: None,
            is_active: true,
            created_at: chrono::Utc::now(),
        }
    }

    fn create_test_content(user_id: uuid::Uuid) -> ContentRow {
        ContentRow {
            id: uuid::Uuid::new_v4(),
            user_id,
            title: "Test Content".to_owned(),
            description: Some("Test description".to_owned()),
            storage_backend: "r2".to_owned(),
            storage_profile: "default".to_owned(),
            storage_key: "test/file.txt".to_owned(),
            content_type: "text/plain".to_owned(),
            file_size: 1024,
            status: "active".to_owned(),
            visibility: "private".to_owned(),
            kind: "file".to_owned(),
            body: None,
            trashed_at: None,
            archived_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn create_test_group(user_id: uuid::Uuid) -> ContentGroupRow {
        ContentGroupRow {
            id: uuid::Uuid::new_v4(),
            user_id,
            name: "Test Group".to_owned(),
            description: Some("Test group description".to_owned()),
            visibility: "private".to_owned(),
            status: "active".to_owned(),
            trashed_at: None,
            archived_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

// Implement SqlStorage by delegating most methods to inner MockSqlStorage
// but override share link methods to use our test data
impl SqlStorage for ShareLinksMockSqlStorage {
    async fn is_connected(&self) -> bool {
        self.inner.is_connected().await
    }

    async fn contents_insert(
        &self,
        input: collects_services::database::ContentsInsert,
    ) -> Result<ContentRow, SqlStorageError> {
        self.inner.contents_insert(input).await
    }

    async fn contents_get(&self, id: uuid::Uuid) -> Result<Option<ContentRow>, SqlStorageError> {
        let contents = self.contents.read().unwrap();
        Ok(contents.iter().find(|c| c.id == id).cloned())
    }

    async fn contents_list_for_user(
        &self,
        user_id: uuid::Uuid,
        params: collects_services::database::ContentsListParams,
    ) -> Result<Vec<ContentRow>, SqlStorageError> {
        self.inner.contents_list_for_user(user_id, params).await
    }

    async fn contents_update_metadata(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        changes: collects_services::database::ContentsUpdate,
    ) -> Result<Option<ContentRow>, SqlStorageError> {
        self.inner
            .contents_update_metadata(id, user_id, changes)
            .await
    }

    async fn contents_set_status(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        new_status: collects_services::database::ContentStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ContentRow>, SqlStorageError> {
        self.inner
            .contents_set_status(id, user_id, new_status, now)
            .await
    }

    async fn groups_create(
        &self,
        input: collects_services::database::GroupCreate,
    ) -> Result<ContentGroupRow, SqlStorageError> {
        self.inner.groups_create(input).await
    }

    async fn groups_get(&self, id: uuid::Uuid) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        let groups = self.groups.read().unwrap();
        Ok(groups.iter().find(|g| g.id == id).cloned())
    }

    async fn groups_list_for_user(
        &self,
        user_id: uuid::Uuid,
        params: collects_services::database::GroupsListParams,
    ) -> Result<Vec<ContentGroupRow>, SqlStorageError> {
        self.inner.groups_list_for_user(user_id, params).await
    }

    async fn groups_update_metadata(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        changes: collects_services::database::GroupUpdate,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        self.inner
            .groups_update_metadata(id, user_id, changes)
            .await
    }

    async fn groups_set_status(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        new_status: collects_services::database::GroupStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        self.inner
            .groups_set_status(id, user_id, new_status, now)
            .await
    }

    async fn group_items_add(
        &self,
        group_id: uuid::Uuid,
        content_id: uuid::Uuid,
        sort_order: i32,
    ) -> Result<(), SqlStorageError> {
        self.inner
            .group_items_add(group_id, content_id, sort_order)
            .await
    }

    async fn group_items_remove(
        &self,
        group_id: uuid::Uuid,
        content_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        self.inner.group_items_remove(group_id, content_id).await
    }

    async fn group_items_list(
        &self,
        group_id: uuid::Uuid,
    ) -> Result<Vec<collects_services::database::ContentGroupItemRow>, SqlStorageError> {
        self.inner.group_items_list(group_id).await
    }

    async fn group_items_reorder(
        &self,
        group_id: uuid::Uuid,
        user_id: uuid::Uuid,
        items: &[(uuid::Uuid, i32)],
    ) -> Result<(), SqlStorageError> {
        self.inner
            .group_items_reorder(group_id, user_id, items)
            .await
    }

    async fn tags_create(
        &self,
        input: collects_services::database::TagCreate,
    ) -> Result<collects_services::database::TagRow, SqlStorageError> {
        self.inner.tags_create(input).await
    }

    async fn tags_list_for_user(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<Vec<collects_services::database::TagRow>, SqlStorageError> {
        self.inner.tags_list_for_user(user_id).await
    }

    async fn tags_delete(
        &self,
        user_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        self.inner.tags_delete(user_id, tag_id).await
    }

    async fn tags_update(
        &self,
        user_id: uuid::Uuid,
        tag_id: uuid::Uuid,
        input: collects_services::database::TagUpdate,
    ) -> Result<Option<collects_services::database::TagRow>, SqlStorageError> {
        self.inner.tags_update(user_id, tag_id, input).await
    }

    async fn content_tags_attach(
        &self,
        content_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        self.inner.content_tags_attach(content_id, tag_id).await
    }

    async fn content_tags_detach(
        &self,
        content_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        self.inner.content_tags_detach(content_id, tag_id).await
    }

    async fn content_tags_list_for_content(
        &self,
        content_id: uuid::Uuid,
    ) -> Result<Vec<collects_services::database::TagRow>, SqlStorageError> {
        self.inner.content_tags_list_for_content(content_id).await
    }

    // Share links methods - these are the ones we're testing
    async fn share_links_create(
        &self,
        input: ShareLinkCreate,
    ) -> Result<ShareLinkRow, SqlStorageError> {
        let share_link = ShareLinkRow {
            id: uuid::Uuid::new_v4(),
            owner_id: input.owner_id,
            token: input.token,
            name: input.name,
            permission: input.permission.as_db_str().to_owned(),
            password_hash: input.password_hash,
            max_access_count: input.max_access_count,
            access_count: 0,
            expires_at: input.expires_at,
            is_active: true,
            created_at: chrono::Utc::now(),
        };
        self.share_links.write().unwrap().push(share_link.clone());
        Ok(share_link)
    }

    async fn share_links_get_by_token(
        &self,
        token: &str,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        let share_links = self.share_links.read().unwrap();
        Ok(share_links.iter().find(|s| s.token == token).cloned())
    }

    async fn share_links_list_for_owner(
        &self,
        owner_id: uuid::Uuid,
    ) -> Result<Vec<ShareLinkRow>, SqlStorageError> {
        let share_links = self.share_links.read().unwrap();
        Ok(share_links
            .iter()
            .filter(|s| s.owner_id == owner_id)
            .cloned()
            .collect())
    }

    async fn share_links_deactivate(
        &self,
        owner_id: uuid::Uuid,
        share_link_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        let mut share_links = self.share_links.write().unwrap();
        if let Some(link) = share_links
            .iter_mut()
            .find(|s| s.id == share_link_id && s.owner_id == owner_id)
        {
            link.is_active = false;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn share_links_get(
        &self,
        id: uuid::Uuid,
        owner_id: uuid::Uuid,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        let share_links = self.share_links.read().unwrap();
        Ok(share_links
            .iter()
            .find(|s| s.id == id && s.owner_id == owner_id)
            .cloned())
    }

    async fn share_links_update(
        &self,
        id: uuid::Uuid,
        owner_id: uuid::Uuid,
        input: ShareLinkUpdate,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        let mut share_links = self.share_links.write().unwrap();
        if let Some(link) = share_links
            .iter_mut()
            .find(|s| s.id == id && s.owner_id == owner_id)
        {
            if let Some(name) = input.name {
                link.name = name;
            }
            if let Some(permission) = input.permission {
                link.permission = permission.as_db_str().to_owned();
            }
            if let Some(password_hash) = input.password_hash {
                link.password_hash = password_hash;
            }
            if let Some(expires_at) = input.expires_at {
                link.expires_at = expires_at;
            }
            if let Some(max_access_count) = input.max_access_count {
                link.max_access_count = max_access_count;
            }
            if let Some(is_active) = input.is_active {
                link.is_active = is_active;
            }
            Ok(Some(link.clone()))
        } else {
            Ok(None)
        }
    }

    async fn share_links_delete(
        &self,
        id: uuid::Uuid,
        owner_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        let mut share_links = self.share_links.write().unwrap();
        let initial_len = share_links.len();
        share_links.retain(|s| !(s.id == id && s.owner_id == owner_id));
        Ok(share_links.len() < initial_len)
    }

    async fn share_links_increment_access(&self, id: uuid::Uuid) -> Result<(), SqlStorageError> {
        let mut share_links = self.share_links.write().unwrap();
        if let Some(link) = share_links.iter_mut().find(|s| s.id == id) {
            link.access_count += 1;
        }
        Ok(())
    }

    async fn content_shares_attach_link(
        &self,
        content_id: uuid::Uuid,
        share_link_id: uuid::Uuid,
        _created_by: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        self.content_shares
            .write()
            .unwrap()
            .push((content_id, share_link_id));
        Ok(())
    }

    async fn group_shares_attach_link(
        &self,
        group_id: uuid::Uuid,
        share_link_id: uuid::Uuid,
        _created_by: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        self.group_shares
            .write()
            .unwrap()
            .push((group_id, share_link_id));
        Ok(())
    }

    async fn contents_get_by_share_token(
        &self,
        token: &str,
    ) -> Result<Option<(ContentRow, ShareLinkRow)>, SqlStorageError> {
        let share_links = self.share_links.read().unwrap();
        let content_shares = self.content_shares.read().unwrap();
        let contents = self.contents.read().unwrap();

        if let Some(share_link) = share_links.iter().find(|s| s.token == token)
            && let Some((content_id, _)) = content_shares
                .iter()
                .find(|(_, sl_id)| *sl_id == share_link.id)
            && let Some(content) = contents.iter().find(|c| c.id == *content_id)
        {
            return Ok(Some((content.clone(), share_link.clone())));
        }
        Ok(None)
    }

    async fn groups_get_by_share_token(
        &self,
        token: &str,
    ) -> Result<Option<(ContentGroupRow, ShareLinkRow, i64)>, SqlStorageError> {
        let share_links = self.share_links.read().unwrap();
        let group_shares = self.group_shares.read().unwrap();
        let groups = self.groups.read().unwrap();

        if let Some(share_link) = share_links.iter().find(|s| s.token == token)
            && let Some((group_id, _)) = group_shares
                .iter()
                .find(|(_, sl_id)| *sl_id == share_link.id)
            && let Some(group) = groups.iter().find(|g| g.id == *group_id)
        {
            return Ok(Some((group.clone(), share_link.clone(), 5))); // Mock file count
        }
        Ok(None)
    }

    async fn content_shares_create_for_user(
        &self,
        input: collects_services::database::ContentShareCreateForUser,
    ) -> Result<collects_services::database::ContentShareRow, SqlStorageError> {
        self.inner.content_shares_create_for_user(input).await
    }

    async fn content_shares_create_for_link(
        &self,
        input: collects_services::database::ContentShareCreateForLink,
    ) -> Result<collects_services::database::ContentShareRow, SqlStorageError> {
        self.inner.content_shares_create_for_link(input).await
    }

    async fn group_shares_create_for_user(
        &self,
        input: collects_services::database::GroupShareCreateForUser,
    ) -> Result<collects_services::database::ContentGroupShareRow, SqlStorageError> {
        self.inner.group_shares_create_for_user(input).await
    }

    async fn group_shares_create_for_link(
        &self,
        input: collects_services::database::GroupShareCreateForLink,
    ) -> Result<collects_services::database::ContentGroupShareRow, SqlStorageError> {
        self.inner.group_shares_create_for_link(input).await
    }

    async fn otp_record_attempt(
        &self,
        input: collects_services::database::OtpAttemptRecord,
    ) -> Result<(), SqlStorageError> {
        self.inner.otp_record_attempt(input).await
    }

    async fn otp_is_rate_limited(
        &self,
        username: &str,
        ip_address: Option<std::net::IpAddr>,
        config: &collects_services::database::OtpRateLimitConfig,
    ) -> Result<bool, SqlStorageError> {
        self.inner
            .otp_is_rate_limited(username, ip_address, config)
            .await
    }

    async fn uploads_create(
        &self,
        input: collects_services::database::UploadInsert,
    ) -> Result<collects_services::database::UploadRow, SqlStorageError> {
        self.inner.uploads_create(input).await
    }

    async fn uploads_get(
        &self,
        id: uuid::Uuid,
    ) -> Result<Option<collects_services::database::UploadRow>, SqlStorageError> {
        self.inner.uploads_get(id).await
    }

    async fn uploads_complete(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
    ) -> Result<Option<collects_services::database::UploadRow>, SqlStorageError> {
        self.inner.uploads_complete(id, user_id).await
    }

    async fn revoked_tokens_add(
        &self,
        token_hash: &str,
        username: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), SqlStorageError> {
        self.inner
            .revoked_tokens_add(token_hash, username, expires_at)
            .await
    }

    async fn revoked_tokens_is_revoked(&self, token_hash: &str) -> Result<bool, SqlStorageError> {
        self.inner.revoked_tokens_is_revoked(token_hash).await
    }
}

async fn get_response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

// =============================================================================
// Share Links CRUD Tests
// =============================================================================

#[tokio::test]
async fn test_share_links_list_without_auth_returns_401() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/share-links")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_share_links_list_empty() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert!(json["share_links"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_share_links_list_with_links() {
    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "test-token-123", "view");

    let sql_storage = ShareLinksMockSqlStorage::new().with_share_link(share_link);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    let share_links = json["share_links"].as_array().unwrap();
    assert_eq!(share_links.len(), 1);
    assert_eq!(share_links[0]["token"], "test-token-123");
    assert_eq!(share_links[0]["permission"], "view");
}

#[tokio::test]
async fn test_share_links_create_success() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "My Share",
                        "permission": "download"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = get_response_json(response).await;
    assert_eq!(json["name"], "My Share");
    assert_eq!(json["permission"], "download");
    assert!(!json["token"].as_str().unwrap().is_empty());
    assert!(json["share_url"].as_str().unwrap().contains("/s/"));
}

#[tokio::test]
async fn test_share_links_create_with_password() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Protected Share",
                        "permission": "view",
                        "password": "secret123"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = get_response_json(response).await;
    assert_eq!(json["has_password"], true);
}

#[tokio::test]
async fn test_share_links_create_with_expiration() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();
    let expires_at = (chrono::Utc::now() + chrono::Duration::days(7)).to_rfc3339();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "permission": "view",
                        "expires_at": expires_at,
                        "max_access_count": 10
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = get_response_json(response).await;
    assert!(json["expires_at"].as_str().is_some());
    assert_eq!(json["max_access_count"], 10);
}

#[tokio::test]
async fn test_share_links_create_invalid_permission() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "permission": "invalid"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_share_links_create_invalid_expires_at() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "permission": "view",
                        "expires_at": "not-a-date"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_share_links_get_success() {
    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "test-token-123", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_share_link(share_link);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/share-links/{}", share_link_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert_eq!(json["token"], "test-token-123");
}

#[tokio::test]
async fn test_share_links_get_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/share-links/00000000-0000-0000-0000-000000000001")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_share_links_get_invalid_id() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/share-links/not-a-uuid")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_share_links_update_success() {
    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "test-token-123", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_share_link(share_link);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/share-links/{}", share_link_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Updated Name",
                        "permission": "download",
                        "is_active": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert_eq!(json["name"], "Updated Name");
    assert_eq!(json["permission"], "download");
    assert_eq!(json["is_active"], false);
}

#[tokio::test]
async fn test_share_links_update_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/v1/share-links/00000000-0000-0000-0000-000000000001")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"name": "Test"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_share_links_delete_success() {
    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "test-token-123", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_share_link(share_link);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/share-links/{}", share_link_id))
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_share_links_delete_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/v1/share-links/00000000-0000-0000-0000-000000000001")
                .header("Authorization", format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Content Share Link Attachment Tests
// =============================================================================

#[tokio::test]
async fn test_content_share_link_create_success() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_content(content);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contents/{}/share-link", content_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Content Share",
                        "permission": "download"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = get_response_json(response).await;
    assert_eq!(json["name"], "Content Share");
    assert_eq!(json["permission"], "download");
}

#[tokio::test]
async fn test_content_share_link_create_content_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents/00000000-0000-0000-0000-000000000001/share-link")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"permission": "view"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_content_share_link_create_not_owner() {
    // Create content owned by a different user
    let mut content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    content.user_id = uuid::Uuid::new_v4(); // Different user
    let content_id = content.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_content(content);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/contents/{}/share-link", content_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"permission": "view"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Group Share Link Attachment Tests
// =============================================================================

#[tokio::test]
async fn test_group_share_link_create_success() {
    let group = ShareLinksMockSqlStorage::create_test_group(TEST_USER_ID);
    let group_id = group.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_group(group);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/groups/{}/share-link", group_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Group Share",
                        "permission": "view"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = get_response_json(response).await;
    assert_eq!(json["name"], "Group Share");
}

#[tokio::test]
async fn test_group_share_link_create_group_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/groups/00000000-0000-0000-0000-000000000001/share-link")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"permission": "view"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// =============================================================================
// Public Share Access Tests
// =============================================================================

#[tokio::test]
async fn test_public_share_get_content_success() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "public-token", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    // Manually attach the share link to content
    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    // No auth token - public endpoint
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/public-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert_eq!(json["content_type"], "content");
    assert_eq!(json["title"], "Test Content");
    assert_eq!(json["permission"], "view");
    assert_eq!(json["requires_password"], false);
}

#[tokio::test]
async fn test_public_share_get_group_success() {
    let group = ShareLinksMockSqlStorage::create_test_group(TEST_USER_ID);
    let group_id = group.id;

    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "group-token", "download");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_group(group)
        .with_share_link(share_link);

    // Manually attach the share link to group
    sql_storage
        .group_shares
        .write()
        .unwrap()
        .push((group_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/group-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert_eq!(json["content_type"], "group");
    assert_eq!(json["title"], "Test Group");
    assert_eq!(json["permission"], "download");
    assert_eq!(json["file_count"], 5);
}

#[tokio::test]
async fn test_public_share_get_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/nonexistent-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_public_share_get_inactive_link() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "inactive-token", "view");
    share_link.is_active = false;
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/inactive-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn test_public_share_get_expired_link() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "expired-token", "view");
    share_link.expires_at = Some(chrono::Utc::now() - chrono::Duration::days(1));
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/expired-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn test_public_share_get_max_access_exceeded() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "maxed-token", "view");
    share_link.max_access_count = Some(5);
    share_link.access_count = 5; // Already at max
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/maxed-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GONE);
}

#[tokio::test]
async fn test_public_share_get_password_required() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "protected-token", "view");
    share_link.password_hash = Some("hashed_password".to_owned());
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/public/share/protected-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert_eq!(json["requires_password"], true);
}

// =============================================================================
// Public View URL Tests
// =============================================================================

#[tokio::test]
async fn test_public_share_view_url_success() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let share_link = ShareLinksMockSqlStorage::create_test_share_link(
        TEST_USER_ID,
        "view-url-token",
        "download",
    );
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/view-url-token/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "disposition": "inline"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert!(
        json["url"]
            .as_str()
            .unwrap()
            .contains("test.r2.example.com")
    );
    assert!(json["expires_at"].as_str().is_some());
}

#[tokio::test]
async fn test_public_share_view_url_not_found() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/nonexistent/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"disposition": "inline"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_public_share_view_url_password_required() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    // Hash "secret123" using SHA256 (same as in share_links.rs)
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update("secret123".as_bytes());
    let password_hash = format!("{:x}", hasher.finalize());

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "pwd-token", "download");
    share_link.password_hash = Some(password_hash);
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    // Without password
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/pwd-token/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"disposition": "inline"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_public_share_view_url_with_correct_password() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    // Hash "secret123" using SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update("secret123".as_bytes());
    let password_hash = format!("{:x}", hasher.finalize());

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "pwd-token-2", "download");
    share_link.password_hash = Some(password_hash);
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/pwd-token-2/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "password": "secret123",
                        "disposition": "attachment"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_public_share_view_url_with_wrong_password() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update("secret123".as_bytes());
    let password_hash = format!("{:x}", hasher.finalize());

    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "pwd-token-3", "download");
    share_link.password_hash = Some(password_hash);
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/pwd-token-3/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "password": "wrongpassword",
                        "disposition": "inline"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_public_share_view_url_invalid_disposition() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "disp-token", "download");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/disp-token/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "disposition": "invalid"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_public_share_view_url_group_not_supported() {
    let group = ShareLinksMockSqlStorage::create_test_group(TEST_USER_ID);
    let group_id = group.id;

    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "group-view-token", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_group(group)
        .with_share_link(share_link);

    sql_storage
        .group_shares
        .write()
        .unwrap()
        .push((group_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/group-view-token/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"disposition": "inline"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Group shares don't support view-url directly
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_public_share_view_url_view_permission_forces_inline() {
    let content = ShareLinksMockSqlStorage::create_test_content(TEST_USER_ID);
    let content_id = content.id;

    // Create share link with "view" permission (not "download")
    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "view-only-token", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new()
        .with_content(content)
        .with_share_link(share_link);

    sql_storage
        .content_shares
        .write()
        .unwrap()
        .push((content_id, share_link_id));

    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    // Request attachment disposition, but permission is view-only
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/public/share/view-only-token/view-url")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "disposition": "attachment"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    // URL should have inline disposition forced due to view-only permission
    assert!(json["url"].as_str().unwrap().contains("disposition=inline"));
}

// =============================================================================
// Edge Case Tests
// =============================================================================

#[tokio::test]
async fn test_share_links_create_default_permission() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    // Create without specifying permission - should default to "view"
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/share-links")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = get_response_json(response).await;
    assert_eq!(json["permission"], "view");
}

#[tokio::test]
async fn test_share_links_update_clear_name() {
    let share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "clear-name-token", "view");
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_share_link(share_link);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    // Clear the name by setting it to null
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/share-links/{}", share_link_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"name": null}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert!(json["name"].is_null());
}

#[tokio::test]
async fn test_share_links_update_remove_password() {
    let mut share_link =
        ShareLinksMockSqlStorage::create_test_share_link(TEST_USER_ID, "remove-pwd-token", "view");
    share_link.password_hash = Some("some_hash".to_owned());
    let share_link_id = share_link.id;

    let sql_storage = ShareLinksMockSqlStorage::new().with_share_link(share_link);
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    // Remove password by setting it to empty string
    let response = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/v1/share-links/{}", share_link_id))
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"password": ""}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = get_response_json(response).await;
    assert_eq!(json["has_password"], false);
}

#[tokio::test]
async fn test_content_share_link_invalid_content_id() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/contents/not-a-uuid/share-link")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"permission": "view"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_group_share_link_invalid_group_id() {
    let sql_storage = ShareLinksMockSqlStorage::new();
    let user_storage = MockUserStorage::with_users([("testuser", "SECRET123")]);
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

    let token = generate_test_token();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/groups/not-a-uuid/share-link")
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .body(Body::from(json!({"permission": "view"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
