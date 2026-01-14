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
        ShareLinkRow, ShareLinkUpdate, SqlStorage, SqlStorageError, TagCreate, TagRow, TagUpdate,
        UploadInsert, UploadRow,
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
            "MockSqlStorage.contents_insert: unimplemented".to_owned(),
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
            "MockSqlStorage.groups_create: unimplemented".to_owned(),
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

    async fn group_items_reorder(
        &self,
        _group_id: uuid::Uuid,
        _user_id: uuid::Uuid,
        _items: &[(uuid::Uuid, i32)],
    ) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn tags_create(&self, _input: TagCreate) -> Result<TagRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.tags_create: unimplemented".to_owned(),
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

    async fn tags_update(
        &self,
        _user_id: uuid::Uuid,
        _tag_id: uuid::Uuid,
        _input: TagUpdate,
    ) -> Result<Option<TagRow>, SqlStorageError> {
        Ok(None)
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
            "MockSqlStorage.share_links_create: unimplemented".to_owned(),
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

    async fn share_links_get(
        &self,
        _id: uuid::Uuid,
        _owner_id: uuid::Uuid,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        Ok(None)
    }

    async fn share_links_update(
        &self,
        _id: uuid::Uuid,
        _owner_id: uuid::Uuid,
        _input: ShareLinkUpdate,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        Ok(None)
    }

    async fn share_links_delete(
        &self,
        _id: uuid::Uuid,
        _owner_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        Ok(false)
    }

    async fn share_links_increment_access(&self, _id: uuid::Uuid) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn content_shares_attach_link(
        &self,
        _content_id: uuid::Uuid,
        _share_link_id: uuid::Uuid,
        _created_by: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn group_shares_attach_link(
        &self,
        _group_id: uuid::Uuid,
        _share_link_id: uuid::Uuid,
        _created_by: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn contents_get_by_share_token(
        &self,
        _token: &str,
    ) -> Result<Option<(ContentRow, ShareLinkRow)>, SqlStorageError> {
        Ok(None)
    }

    async fn groups_get_by_share_token(
        &self,
        _token: &str,
    ) -> Result<Option<(ContentGroupRow, ShareLinkRow, i64)>, SqlStorageError> {
        Ok(None)
    }

    async fn content_shares_create_for_user(
        &self,
        _input: ContentShareCreateForUser,
    ) -> Result<ContentShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.content_shares_create_for_user: unimplemented".to_owned(),
        ))
    }

    async fn content_shares_create_for_link(
        &self,
        _input: ContentShareCreateForLink,
    ) -> Result<ContentShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.content_shares_create_for_link: unimplemented".to_owned(),
        ))
    }

    async fn group_shares_create_for_user(
        &self,
        _input: GroupShareCreateForUser,
    ) -> Result<ContentGroupShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.group_shares_create_for_user: unimplemented".to_owned(),
        ))
    }

    async fn group_shares_create_for_link(
        &self,
        _input: GroupShareCreateForLink,
    ) -> Result<ContentGroupShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.group_shares_create_for_link: unimplemented".to_owned(),
        ))
    }

    async fn otp_record_attempt(
        &self,
        _input: collects_services::database::OtpAttemptRecord,
    ) -> Result<(), SqlStorageError> {
        // Mock: silently succeed
        Ok(())
    }

    async fn otp_is_rate_limited(
        &self,
        _username: &str,
        _ip_address: Option<std::net::IpAddr>,
        _config: &collects_services::database::OtpRateLimitConfig,
    ) -> Result<bool, SqlStorageError> {
        // Mock: never rate limited
        Ok(false)
    }

    async fn uploads_create(&self, _input: UploadInsert) -> Result<UploadRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.uploads_create: unimplemented".to_owned(),
        ))
    }

    async fn uploads_get(&self, _id: uuid::Uuid) -> Result<Option<UploadRow>, SqlStorageError> {
        Ok(None)
    }

    async fn uploads_complete(
        &self,
        _id: uuid::Uuid,
        _user_id: uuid::Uuid,
    ) -> Result<Option<UploadRow>, SqlStorageError> {
        Ok(None)
    }

    async fn revoked_tokens_add(
        &self,
        _token_hash: &str,
        _username: &str,
        _expires_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), SqlStorageError> {
        // Mock: silently succeed
        Ok(())
    }

    async fn revoked_tokens_is_revoked(&self, _token_hash: &str) -> Result<bool, SqlStorageError> {
        // Mock: tokens are never revoked
        Ok(false)
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
        "myteam.cloudflareaccess.com".to_owned(),
        "test-aud-123".to_owned(),
    );

    assert_eq!(config.team_domain, "myteam.cloudflareaccess.com");
    assert_eq!(config.audience, "test-aud-123");
    assert_eq!(
        config.jwks_url(),
        "https://myteam.cloudflareaccess.com/cdn-cgi/access/certs"
    );
}
