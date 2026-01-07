//! Integration tests for Cloudflare Zero Trust authentication

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use collects_services::{
    auth::ZeroTrustConfig,
    config::Config,
    database::{
        ContentGroupItemRow, ContentGroupRow, ContentGroupShareRow, ContentRow,
        ContentShareCreateForLink, ContentShareCreateForUser, ContentShareRow, ContentStatus,
        ContentsInsert, ContentsListParams, ContentsUpdate, GroupCreate, GroupShareCreateForLink,
        GroupShareCreateForUser, GroupStatus, GroupUpdate, GroupsListParams, ShareLinkCreate,
        ShareLinkRow, SqlStorage, SqlStorageError, TagCreate, TagRow,
    },
    routes,
    users::storage::MockUserStorage,
};
use tower::ServiceExt;

#[derive(Clone)]
struct MockSqlStorage {
    is_connected: bool,
}

impl SqlStorage for MockSqlStorage {
    async fn is_connected(&self) -> bool {
        self.is_connected
    }

    async fn contents_insert(&self, _input: ContentsInsert) -> Result<ContentRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.contents_insert: unimplemented".to_string(),
        ))
    }

    async fn contents_get(&self, _id: uuid::Uuid) -> Result<Option<ContentRow>, SqlStorageError> {
        Ok(None)
    }

    async fn contents_list_for_user(
        &self,
        _user_id: uuid::Uuid,
        _params: ContentsListParams,
    ) -> Result<Vec<ContentRow>, SqlStorageError> {
        Ok(vec![])
    }

    async fn contents_update_metadata(
        &self,
        _id: uuid::Uuid,
        _user_id: uuid::Uuid,
        _changes: ContentsUpdate,
    ) -> Result<Option<ContentRow>, SqlStorageError> {
        Ok(None)
    }

    async fn contents_set_status(
        &self,
        _id: uuid::Uuid,
        _user_id: uuid::Uuid,
        _new_status: ContentStatus,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ContentRow>, SqlStorageError> {
        Ok(None)
    }

    async fn groups_create(&self, _input: GroupCreate) -> Result<ContentGroupRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.groups_create: unimplemented".to_string(),
        ))
    }

    async fn groups_get(
        &self,
        _id: uuid::Uuid,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        Ok(None)
    }

    async fn groups_list_for_user(
        &self,
        _user_id: uuid::Uuid,
        _params: GroupsListParams,
    ) -> Result<Vec<ContentGroupRow>, SqlStorageError> {
        Ok(vec![])
    }

    async fn groups_update_metadata(
        &self,
        _id: uuid::Uuid,
        _user_id: uuid::Uuid,
        _changes: GroupUpdate,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        Ok(None)
    }

    async fn groups_set_status(
        &self,
        _id: uuid::Uuid,
        _user_id: uuid::Uuid,
        _new_status: GroupStatus,
        _now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        Ok(None)
    }

    async fn group_items_add(
        &self,
        _group_id: uuid::Uuid,
        _content_id: uuid::Uuid,
        _sort_order: i32,
    ) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn group_items_remove(
        &self,
        _group_id: uuid::Uuid,
        _content_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        Ok(false)
    }

    async fn group_items_list(
        &self,
        _group_id: uuid::Uuid,
    ) -> Result<Vec<ContentGroupItemRow>, SqlStorageError> {
        Ok(vec![])
    }

    async fn tags_create(&self, _input: TagCreate) -> Result<TagRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.tags_create: unimplemented".to_string(),
        ))
    }

    async fn tags_list_for_user(
        &self,
        _user_id: uuid::Uuid,
    ) -> Result<Vec<TagRow>, SqlStorageError> {
        Ok(vec![])
    }

    async fn tags_delete(
        &self,
        _user_id: uuid::Uuid,
        _tag_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        Ok(false)
    }

    async fn content_tags_attach(
        &self,
        _content_id: uuid::Uuid,
        _tag_id: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn content_tags_detach(
        &self,
        _content_id: uuid::Uuid,
        _tag_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        Ok(false)
    }

    async fn content_tags_list_for_content(
        &self,
        _content_id: uuid::Uuid,
    ) -> Result<Vec<TagRow>, SqlStorageError> {
        Ok(vec![])
    }

    async fn share_links_create(
        &self,
        _input: ShareLinkCreate,
    ) -> Result<ShareLinkRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.share_links_create: unimplemented".to_string(),
        ))
    }

    async fn share_links_get_by_token(
        &self,
        _token: &str,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        Ok(None)
    }

    async fn share_links_list_for_owner(
        &self,
        _owner_id: uuid::Uuid,
    ) -> Result<Vec<ShareLinkRow>, SqlStorageError> {
        Ok(vec![])
    }

    async fn share_links_deactivate(
        &self,
        _owner_id: uuid::Uuid,
        _share_link_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        Ok(false)
    }

    async fn content_shares_create_for_user(
        &self,
        _input: ContentShareCreateForUser,
    ) -> Result<ContentShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.content_shares_create_for_user: unimplemented".to_string(),
        ))
    }

    async fn content_shares_create_for_link(
        &self,
        _input: ContentShareCreateForLink,
    ) -> Result<ContentShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.content_shares_create_for_link: unimplemented".to_string(),
        ))
    }

    async fn group_shares_create_for_user(
        &self,
        _input: GroupShareCreateForUser,
    ) -> Result<ContentGroupShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.group_shares_create_for_user: unimplemented".to_string(),
        ))
    }

    async fn group_shares_create_for_link(
        &self,
        _input: GroupShareCreateForLink,
    ) -> Result<ContentGroupShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.group_shares_create_for_link: unimplemented".to_string(),
        ))
    }
}

#[tokio::test]
async fn test_internal_route_without_zerotrust_config() {
    // When Zero Trust is not configured, routes should be accessible
    let sql_storage = MockSqlStorage { is_connected: true };
    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

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
    let sql_storage = MockSqlStorage { is_connected: true };
    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

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

    // Should return UNAUTHORIZED (user not found) since we're using real UserStorage now
    // This proves auth routes don't require Zero Trust authentication
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_health_check_always_accessible() {
    // Health check should always be accessible
    let sql_storage = MockSqlStorage { is_connected: true };
    let user_storage = MockUserStorage::new();
    let config = Config::new_for_test();
    let app = routes(sql_storage, user_storage, config).await;

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
