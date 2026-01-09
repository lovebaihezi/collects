//! Shared test utilities for integration tests.
//!
//! This module provides common test infrastructure including:
//! - `MockSqlStorage` - A mock implementation of `SqlStorage` for testing
//! - Test constants and helper functions

use collects_services::{
    config::Config,
    database::{
        ContentGroupItemRow, ContentGroupRow, ContentGroupShareRow, ContentRow,
        ContentShareCreateForLink, ContentShareCreateForUser, ContentShareRow, ContentStatus,
        ContentsInsert, ContentsListParams, ContentsUpdate, GroupCreate, GroupShareCreateForLink,
        GroupShareCreateForUser, GroupStatus, GroupUpdate, GroupsListParams, OtpAttemptRecord,
        OtpRateLimitConfig, ShareLinkCreate, ShareLinkRow, SqlStorage, SqlStorageError, TagCreate,
        TagRow, TagUpdate, UploadInsert, UploadRow,
    },
    routes,
    users::storage::MockUserStorage,
};

/// A fixed UUID for test scenarios to coordinate between MockSqlStorage and MockUserStorage.
#[allow(dead_code)]
pub const TEST_USER_ID: uuid::Uuid = uuid::Uuid::from_u128(0x00000000_0000_0000_0000_000000000001);

/// A fixed UUID for test content.
#[allow(dead_code)]
pub const TEST_CONTENT_ID: uuid::Uuid =
    uuid::Uuid::from_u128(0x00000000_0000_0000_0000_000000000000);

/// JWT secret used for test token generation.
pub const TEST_JWT_SECRET: &str = "test-jwt-secret-key-for-local-development";

/// Mock SQL storage for testing.
#[derive(Clone)]
pub struct MockSqlStorage {
    pub is_connected: bool,
    /// When set, mock methods will use this user ID for ownership checks.
    pub mock_user_id: Option<uuid::Uuid>,
}

impl MockSqlStorage {
    /// Creates a new MockSqlStorage with default settings (connected, no mock user ID).
    pub fn new() -> Self {
        Self {
            is_connected: true,
            mock_user_id: None,
        }
    }

    /// Creates a MockSqlStorage configured to work with a specific user ID.
    #[allow(dead_code)]
    pub fn with_user_id(user_id: uuid::Uuid) -> Self {
        Self {
            is_connected: true,
            mock_user_id: Some(user_id),
        }
    }

    /// Creates a MockSqlStorage that simulates a disconnected database.
    #[allow(dead_code)]
    pub fn disconnected() -> Self {
        Self {
            is_connected: false,
            mock_user_id: None,
        }
    }
}

impl Default for MockSqlStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a MockUserStorage with a test user.
///
/// Note: This uses `MockUserStorage::with_users` which generates random user IDs.
/// For tests that need a specific user ID (e.g., ownership checks), use
/// `MockSqlStorage::with_user_id` to coordinate the mock responses.
#[allow(dead_code)]
pub fn create_test_user_storage() -> MockUserStorage {
    MockUserStorage::with_users([("testuser", "SECRET123")])
}

/// Generate a valid test JWT token for the "testuser" user.
pub fn generate_test_token() -> String {
    collects_services::users::otp::generate_session_token("testuser", TEST_JWT_SECRET).unwrap()
}

/// Create the test app router with default test configuration.
pub async fn create_test_app(
    sql_storage: MockSqlStorage,
    user_storage: MockUserStorage,
) -> axum::Router {
    let config = Config::new_for_test();
    routes(sql_storage, user_storage, config).await
}

impl SqlStorage for MockSqlStorage {
    async fn is_connected(&self) -> bool {
        self.is_connected
    }

    async fn contents_insert(&self, input: ContentsInsert) -> Result<ContentRow, SqlStorageError> {
        Ok(ContentRow {
            id: uuid::Uuid::new_v4(),
            user_id: input.user_id,
            title: input.title,
            description: input.description,
            storage_backend: input.storage_backend,
            storage_profile: input.storage_profile,
            storage_key: input.storage_key,
            content_type: input.content_type,
            file_size: input.file_size,
            status: "active".to_string(),
            visibility: input.visibility.as_db_str().to_string(),
            kind: input.kind.unwrap_or_else(|| "file".to_string()),
            body: input.body,
            trashed_at: None,
            archived_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    }

    async fn contents_get(&self, id: uuid::Uuid) -> Result<Option<ContentRow>, SqlStorageError> {
        if id == TEST_CONTENT_ID
            && let Some(user_id) = self.mock_user_id
        {
            return Ok(Some(ContentRow {
                id: TEST_CONTENT_ID,
                user_id,
                title: "Test Content".to_string(),
                description: None,
                storage_backend: "r2".to_string(),
                storage_profile: "default".to_string(),
                storage_key: format!("{}/test-uuid/test-file.jpg", user_id),
                content_type: "image/jpeg".to_string(),
                file_size: 1234,
                status: "active".to_string(),
                visibility: "private".to_string(),
                kind: "file".to_string(),
                body: None,
                trashed_at: None,
                archived_at: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }));
        }
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

    async fn otp_record_attempt(&self, _input: OtpAttemptRecord) -> Result<(), SqlStorageError> {
        Ok(())
    }

    async fn otp_is_rate_limited(
        &self,
        _username: &str,
        _ip_address: Option<std::net::IpAddr>,
        _config: &OtpRateLimitConfig,
    ) -> Result<bool, SqlStorageError> {
        Ok(false)
    }

    async fn uploads_create(&self, input: UploadInsert) -> Result<UploadRow, SqlStorageError> {
        Ok(UploadRow {
            id: uuid::Uuid::new_v4(),
            user_id: input.user_id,
            storage_backend: input.storage_backend,
            storage_profile: input.storage_profile,
            storage_key: input.storage_key,
            content_type: input.content_type,
            file_size: input.file_size,
            status: "initiated".to_string(),
            expires_at: input.expires_at,
            created_at: chrono::Utc::now(),
            completed_at: None,
        })
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
}
