use crate::config::Config;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::future::Future;

/// Initialize a PostgreSQL connection pool
pub async fn create_pool(config: &Config) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().connect(config.database_url()).await?;

    tracing::info!("Database connection pool established");

    Ok(pool)
}

/// SQL storage abstraction.
///
/// This trait is designed to be used via generics (`S: SqlStorage`) so the SQL layer can be mocked
/// in unit tests, while `PgStorage` provides the real `sqlx` implementation.
///
/// Notes:
/// - Methods return `impl Future` (instead of needing `async-trait`) to keep callsites ergonomic.
/// - This file uses SQLx *macros* (`query!` / `query_as!`) for compile-time verification.
///   Remember to run `just services::prepare <env>` after changing queries or schema.
pub trait SqlStorage: Clone + Send + Sync + 'static {
    // -------------------------------------------------------------------------
    // Health / connectivity
    // -------------------------------------------------------------------------
    fn is_connected(&self) -> impl Future<Output = bool> + Send;

    // -------------------------------------------------------------------------
    // Contents
    // -------------------------------------------------------------------------
    fn contents_insert(
        &self,
        input: ContentsInsert,
    ) -> impl Future<Output = Result<ContentRow, SqlStorageError>> + Send;

    fn contents_get(
        &self,
        id: uuid::Uuid,
    ) -> impl Future<Output = Result<Option<ContentRow>, SqlStorageError>> + Send;

    fn contents_list_for_user(
        &self,
        user_id: uuid::Uuid,
        params: ContentsListParams,
    ) -> impl Future<Output = Result<Vec<ContentRow>, SqlStorageError>> + Send;

    fn contents_update_metadata(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        changes: ContentsUpdate,
    ) -> impl Future<Output = Result<Option<ContentRow>, SqlStorageError>> + Send;

    fn contents_set_status(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        new_status: ContentStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> impl Future<Output = Result<Option<ContentRow>, SqlStorageError>> + Send;

    // -------------------------------------------------------------------------
    // Content groups + join table
    // -------------------------------------------------------------------------
    fn groups_create(
        &self,
        input: GroupCreate,
    ) -> impl Future<Output = Result<ContentGroupRow, SqlStorageError>> + Send;

    fn groups_get(
        &self,
        id: uuid::Uuid,
    ) -> impl Future<Output = Result<Option<ContentGroupRow>, SqlStorageError>> + Send;

    fn groups_list_for_user(
        &self,
        user_id: uuid::Uuid,
        params: GroupsListParams,
    ) -> impl Future<Output = Result<Vec<ContentGroupRow>, SqlStorageError>> + Send;

    fn groups_update_metadata(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        changes: GroupUpdate,
    ) -> impl Future<Output = Result<Option<ContentGroupRow>, SqlStorageError>> + Send;

    fn groups_set_status(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        new_status: GroupStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> impl Future<Output = Result<Option<ContentGroupRow>, SqlStorageError>> + Send;

    fn group_items_add(
        &self,
        group_id: uuid::Uuid,
        content_id: uuid::Uuid,
        sort_order: i32,
    ) -> impl Future<Output = Result<(), SqlStorageError>> + Send;

    fn group_items_remove(
        &self,
        group_id: uuid::Uuid,
        content_id: uuid::Uuid,
    ) -> impl Future<Output = Result<bool, SqlStorageError>> + Send;

    fn group_items_list(
        &self,
        group_id: uuid::Uuid,
    ) -> impl Future<Output = Result<Vec<ContentGroupItemRow>, SqlStorageError>> + Send;

    // -------------------------------------------------------------------------
    // Tags + attach/detach
    // -------------------------------------------------------------------------
    fn tags_create(
        &self,
        input: TagCreate,
    ) -> impl Future<Output = Result<TagRow, SqlStorageError>> + Send;

    fn tags_list_for_user(
        &self,
        user_id: uuid::Uuid,
    ) -> impl Future<Output = Result<Vec<TagRow>, SqlStorageError>> + Send;

    fn tags_delete(
        &self,
        user_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> impl Future<Output = Result<bool, SqlStorageError>> + Send;

    fn tags_update(
        &self,
        user_id: uuid::Uuid,
        tag_id: uuid::Uuid,
        input: TagUpdate,
    ) -> impl Future<Output = Result<Option<TagRow>, SqlStorageError>> + Send;

    fn content_tags_attach(
        &self,
        content_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> impl Future<Output = Result<(), SqlStorageError>> + Send;

    fn content_tags_detach(
        &self,
        content_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> impl Future<Output = Result<bool, SqlStorageError>> + Send;

    fn content_tags_list_for_content(
        &self,
        content_id: uuid::Uuid,
    ) -> impl Future<Output = Result<Vec<TagRow>, SqlStorageError>> + Send;

    // -------------------------------------------------------------------------
    // Share links + share join tables
    // -------------------------------------------------------------------------
    fn share_links_create(
        &self,
        input: ShareLinkCreate,
    ) -> impl Future<Output = Result<ShareLinkRow, SqlStorageError>> + Send;

    fn share_links_get_by_token(
        &self,
        token: &str,
    ) -> impl Future<Output = Result<Option<ShareLinkRow>, SqlStorageError>> + Send;

    fn share_links_list_for_owner(
        &self,
        owner_id: uuid::Uuid,
    ) -> impl Future<Output = Result<Vec<ShareLinkRow>, SqlStorageError>> + Send;

    fn share_links_deactivate(
        &self,
        owner_id: uuid::Uuid,
        share_link_id: uuid::Uuid,
    ) -> impl Future<Output = Result<bool, SqlStorageError>> + Send;

    fn content_shares_create_for_user(
        &self,
        input: ContentShareCreateForUser,
    ) -> impl Future<Output = Result<ContentShareRow, SqlStorageError>> + Send;

    fn content_shares_create_for_link(
        &self,
        input: ContentShareCreateForLink,
    ) -> impl Future<Output = Result<ContentShareRow, SqlStorageError>> + Send;

    fn group_shares_create_for_user(
        &self,
        input: GroupShareCreateForUser,
    ) -> impl Future<Output = Result<ContentGroupShareRow, SqlStorageError>> + Send;

    fn group_shares_create_for_link(
        &self,
        input: GroupShareCreateForLink,
    ) -> impl Future<Output = Result<ContentGroupShareRow, SqlStorageError>> + Send;
}

/// Minimal error type for SQL storage operations.
///
/// This is deliberately small so mocks can return deterministic errors without
/// pulling in sqlx error types.
#[derive(Debug, thiserror::Error)]
pub enum SqlStorageError {
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("unauthorized")]
    Unauthorized,
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("database error: {0}")]
    Db(String),
}

#[derive(Clone)]
pub struct PgStorage {
    pub pool: PgPool,
}

impl PgStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

// -----------------------------------------------------------------------------
// Domain types for DB I/O
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentStatus {
    Active,
    Archived,
    Trashed,
}

impl ContentStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Trashed => "trashed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupStatus {
    Active,
    Archived,
    Trashed,
}

impl GroupStatus {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Trashed => "trashed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Visibility {
    Private,
    Public,
    Restricted,
}

impl Visibility {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Public => "public",
            Self::Restricted => "restricted",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ContentRow {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub title: String,
    pub description: Option<String>,
    pub storage_backend: String,
    pub storage_profile: String,
    pub storage_key: String,
    pub content_type: String,
    pub file_size: i64,
    pub status: String,
    pub visibility: String,
    pub trashed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ContentsInsert {
    pub user_id: uuid::Uuid,
    pub title: String,
    pub description: Option<String>,
    pub storage_backend: String,
    pub storage_profile: String,
    pub storage_key: String,
    pub content_type: String,
    pub file_size: i64,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Default)]
pub struct ContentsListParams {
    pub limit: i64,
    pub offset: i64,
    pub status: Option<ContentStatus>,
}

#[derive(Debug, Clone, Default)]
pub struct ContentsUpdate {
    pub title: Option<String>,
    /// `None` => no change; `Some(None)` => clear; `Some(Some(v))` => set
    pub description: Option<Option<String>>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone)]
pub struct ContentGroupRow {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub visibility: String,
    pub status: String,
    pub trashed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ContentGroupItemRow {
    pub id: uuid::Uuid,
    pub group_id: uuid::Uuid,
    pub content_id: uuid::Uuid,
    pub sort_order: i32,
    pub added_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct GroupCreate {
    pub user_id: uuid::Uuid,
    pub name: String,
    pub description: Option<String>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone, Default)]
pub struct GroupsListParams {
    pub limit: i64,
    pub offset: i64,
    pub status: Option<GroupStatus>,
}

#[derive(Debug, Clone, Default)]
pub struct GroupUpdate {
    pub name: Option<String>,
    /// `None` => no change; `Some(None)` => clear; `Some(Some(v))` => set
    pub description: Option<Option<String>>,
    pub visibility: Option<Visibility>,
}

#[derive(Debug, Clone)]
pub struct TagRow {
    pub id: uuid::Uuid,
    pub user_id: uuid::Uuid,
    pub name: String,
    pub color: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct TagCreate {
    pub user_id: uuid::Uuid,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct TagUpdate {
    pub name: Option<String>,
    pub color: Option<Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SharePermission {
    View,
    Download,
}

impl SharePermission {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Download => "download",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShareLinkRow {
    pub id: uuid::Uuid,
    pub owner_id: uuid::Uuid,
    pub token: String,
    pub name: Option<String>,
    pub permission: String,
    pub password_hash: Option<String>,
    pub max_access_count: Option<i32>,
    pub access_count: i32,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct ShareLinkCreate {
    pub owner_id: uuid::Uuid,
    pub token: String,
    pub name: Option<String>,
    pub permission: SharePermission,
    pub password_hash: Option<String>,
    pub max_access_count: Option<i32>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct ContentShareRow {
    pub id: uuid::Uuid,
    pub content_id: uuid::Uuid,
    pub shared_with_user_id: Option<uuid::Uuid>,
    pub share_link_id: Option<uuid::Uuid>,
    pub permission: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct ContentShareCreateForUser {
    pub content_id: uuid::Uuid,
    pub shared_with_user_id: uuid::Uuid,
    pub permission: SharePermission,
    pub created_by: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct ContentShareCreateForLink {
    pub content_id: uuid::Uuid,
    pub share_link_id: uuid::Uuid,
    pub permission: SharePermission,
    pub created_by: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct ContentGroupShareRow {
    pub id: uuid::Uuid,
    pub group_id: uuid::Uuid,
    pub shared_with_user_id: Option<uuid::Uuid>,
    pub share_link_id: Option<uuid::Uuid>,
    pub permission: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub created_by: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct GroupShareCreateForUser {
    pub group_id: uuid::Uuid,
    pub shared_with_user_id: uuid::Uuid,
    pub permission: SharePermission,
    pub created_by: uuid::Uuid,
}

#[derive(Debug, Clone)]
pub struct GroupShareCreateForLink {
    pub group_id: uuid::Uuid,
    pub share_link_id: uuid::Uuid,
    pub permission: SharePermission,
    pub created_by: uuid::Uuid,
}

// -----------------------------------------------------------------------------
// PgStorage implementation (SQLx macros)
// -----------------------------------------------------------------------------

impl SqlStorage for PgStorage {
    async fn is_connected(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn contents_insert(&self, input: ContentsInsert) -> Result<ContentRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO contents (
                user_id, title, description,
                storage_backend, storage_profile, storage_key,
                content_type, file_size, visibility
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            RETURNING
                id, user_id, title, description,
                storage_backend, storage_profile, storage_key,
                content_type, file_size, status, visibility,
                trashed_at, archived_at, created_at, updated_at
            "#,
            input.user_id,
            input.title,
            input.description,
            input.storage_backend,
            input.storage_profile,
            input.storage_key,
            input.content_type,
            input.file_size,
            input.visibility.as_db_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ContentRow {
            id: rec.id,
            user_id: rec.user_id,
            title: rec.title,
            description: rec.description,
            storage_backend: rec.storage_backend,
            storage_profile: rec.storage_profile,
            storage_key: rec.storage_key,
            content_type: rec.content_type,
            file_size: rec.file_size,
            status: rec.status,
            visibility: rec.visibility,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        })
    }

    async fn contents_get(&self, id: uuid::Uuid) -> Result<Option<ContentRow>, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            SELECT
                id, user_id, title, description,
                storage_backend, storage_profile, storage_key,
                content_type, file_size, status, visibility,
                trashed_at, archived_at, created_at, updated_at
            FROM contents
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ContentRow {
            id: rec.id,
            user_id: rec.user_id,
            title: rec.title,
            description: rec.description,
            storage_backend: rec.storage_backend,
            storage_profile: rec.storage_profile,
            storage_key: rec.storage_key,
            content_type: rec.content_type,
            file_size: rec.file_size,
            status: rec.status,
            visibility: rec.visibility,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }))
    }

    async fn contents_list_for_user(
        &self,
        user_id: uuid::Uuid,
        params: ContentsListParams,
    ) -> Result<Vec<ContentRow>, SqlStorageError> {
        let limit = if params.limit <= 0 { 50 } else { params.limit };
        let offset = if params.offset < 0 { 0 } else { params.offset };
        let status = params.status.map(|s| s.as_db_str().to_string());

        let recs = sqlx::query!(
            r#"
            SELECT
                id, user_id, title, description,
                storage_backend, storage_profile, storage_key,
                content_type, file_size, status, visibility,
                trashed_at, archived_at, created_at, updated_at
            FROM contents
            WHERE user_id = $1
              AND ($2::text IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            user_id,
            status,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(recs
            .into_iter()
            .map(|rec| ContentRow {
                id: rec.id,
                user_id: rec.user_id,
                title: rec.title,
                description: rec.description,
                storage_backend: rec.storage_backend,
                storage_profile: rec.storage_profile,
                storage_key: rec.storage_key,
                content_type: rec.content_type,
                file_size: rec.file_size,
                status: rec.status,
                visibility: rec.visibility,
                trashed_at: rec.trashed_at,
                archived_at: rec.archived_at,
                created_at: rec.created_at,
                updated_at: rec.updated_at,
            })
            .collect())
    }

    async fn contents_update_metadata(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        changes: ContentsUpdate,
    ) -> Result<Option<ContentRow>, SqlStorageError> {
        let visibility = changes.visibility.map(|v| v.as_db_str().to_string());
        let description_set = changes.description.is_some();
        let description_value = changes.description.unwrap_or(None);

        // We need to handle "no change" vs "clear": COALESCE can't distinguish.
        // Strategy:
        // - If description was provided, use `$4` directly (even if NULL) via CASE when.
        // - If not provided, keep existing.
        let rec = sqlx::query!(
            r#"
            UPDATE contents
            SET
                title = COALESCE($3, title),
                description = CASE
                    WHEN $4::bool THEN $5
                    ELSE description
                END,
                visibility = COALESCE($6, visibility)
            WHERE id = $1 AND user_id = $2
            RETURNING
                id, user_id, title, description,
                storage_backend, storage_profile, storage_key,
                content_type, file_size, status, visibility,
                trashed_at, archived_at, created_at, updated_at
            "#,
            id,
            user_id,
            changes.title,
            description_set,
            description_value,
            visibility,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ContentRow {
            id: rec.id,
            user_id: rec.user_id,
            title: rec.title,
            description: rec.description,
            storage_backend: rec.storage_backend,
            storage_profile: rec.storage_profile,
            storage_key: rec.storage_key,
            content_type: rec.content_type,
            file_size: rec.file_size,
            status: rec.status,
            visibility: rec.visibility,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }))
    }

    async fn contents_set_status(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        new_status: ContentStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ContentRow>, SqlStorageError> {
        let (trashed_at, archived_at) = match new_status {
            ContentStatus::Trashed => (Some(now), None),
            ContentStatus::Archived => (None, Some(now)),
            ContentStatus::Active => (None, None),
        };

        let rec = sqlx::query!(
            r#"
            UPDATE contents
            SET
                status = $3,
                trashed_at = $4,
                archived_at = $5
            WHERE id = $1 AND user_id = $2
            RETURNING
                id, user_id, title, description,
                storage_backend, storage_profile, storage_key,
                content_type, file_size, status, visibility,
                trashed_at, archived_at, created_at, updated_at
            "#,
            id,
            user_id,
            new_status.as_db_str(),
            trashed_at,
            archived_at
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ContentRow {
            id: rec.id,
            user_id: rec.user_id,
            title: rec.title,
            description: rec.description,
            storage_backend: rec.storage_backend,
            storage_profile: rec.storage_profile,
            storage_key: rec.storage_key,
            content_type: rec.content_type,
            file_size: rec.file_size,
            status: rec.status,
            visibility: rec.visibility,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }))
    }

    async fn groups_create(&self, input: GroupCreate) -> Result<ContentGroupRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO content_groups (user_id, name, description, visibility)
            VALUES ($1,$2,$3,$4)
            RETURNING
                id, user_id, name, description,
                visibility, status,
                trashed_at, archived_at, created_at, updated_at
            "#,
            input.user_id,
            input.name,
            input.description,
            input.visibility.as_db_str(),
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ContentGroupRow {
            id: rec.id,
            user_id: rec.user_id,
            name: rec.name,
            description: rec.description,
            visibility: rec.visibility,
            status: rec.status,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        })
    }

    async fn groups_get(&self, id: uuid::Uuid) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            SELECT
                id, user_id, name, description,
                visibility, status,
                trashed_at, archived_at, created_at, updated_at
            FROM content_groups
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ContentGroupRow {
            id: rec.id,
            user_id: rec.user_id,
            name: rec.name,
            description: rec.description,
            visibility: rec.visibility,
            status: rec.status,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }))
    }

    async fn groups_list_for_user(
        &self,
        user_id: uuid::Uuid,
        params: GroupsListParams,
    ) -> Result<Vec<ContentGroupRow>, SqlStorageError> {
        let limit = if params.limit <= 0 { 50 } else { params.limit };
        let offset = if params.offset < 0 { 0 } else { params.offset };
        let status = params.status.map(|s| s.as_db_str().to_string());

        let recs = sqlx::query!(
            r#"
            SELECT
                id, user_id, name, description,
                visibility, status,
                trashed_at, archived_at, created_at, updated_at
            FROM content_groups
            WHERE user_id = $1
              AND ($2::text IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
            user_id,
            status,
            limit,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(recs
            .into_iter()
            .map(|rec| ContentGroupRow {
                id: rec.id,
                user_id: rec.user_id,
                name: rec.name,
                description: rec.description,
                visibility: rec.visibility,
                status: rec.status,
                trashed_at: rec.trashed_at,
                archived_at: rec.archived_at,
                created_at: rec.created_at,
                updated_at: rec.updated_at,
            })
            .collect())
    }

    async fn groups_update_metadata(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        changes: GroupUpdate,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        let visibility = changes.visibility.map(|v| v.as_db_str().to_string());
        let description_set = changes.description.is_some();
        let description_value = changes.description.unwrap_or(None);

        let rec = sqlx::query!(
            r#"
            UPDATE content_groups
            SET
                name = COALESCE($3, name),
                description = CASE
                    WHEN $4::bool THEN $5
                    ELSE description
                END,
                visibility = COALESCE($6, visibility)
            WHERE id = $1 AND user_id = $2
            RETURNING
                id, user_id, name, description,
                visibility, status,
                trashed_at, archived_at, created_at, updated_at
            "#,
            id,
            user_id,
            changes.name,
            description_set,
            description_value,
            visibility,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ContentGroupRow {
            id: rec.id,
            user_id: rec.user_id,
            name: rec.name,
            description: rec.description,
            visibility: rec.visibility,
            status: rec.status,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }))
    }

    async fn groups_set_status(
        &self,
        id: uuid::Uuid,
        user_id: uuid::Uuid,
        new_status: GroupStatus,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<Option<ContentGroupRow>, SqlStorageError> {
        let (trashed_at, archived_at) = match new_status {
            GroupStatus::Trashed => (Some(now), None),
            GroupStatus::Archived => (None, Some(now)),
            GroupStatus::Active => (None, None),
        };

        let rec = sqlx::query!(
            r#"
            UPDATE content_groups
            SET
                status = $3,
                trashed_at = $4,
                archived_at = $5
            WHERE id = $1 AND user_id = $2
            RETURNING
                id, user_id, name, description,
                visibility, status,
                trashed_at, archived_at, created_at, updated_at
            "#,
            id,
            user_id,
            new_status.as_db_str(),
            trashed_at,
            archived_at
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ContentGroupRow {
            id: rec.id,
            user_id: rec.user_id,
            name: rec.name,
            description: rec.description,
            visibility: rec.visibility,
            status: rec.status,
            trashed_at: rec.trashed_at,
            archived_at: rec.archived_at,
            created_at: rec.created_at,
            updated_at: rec.updated_at,
        }))
    }

    async fn group_items_add(
        &self,
        group_id: uuid::Uuid,
        content_id: uuid::Uuid,
        sort_order: i32,
    ) -> Result<(), SqlStorageError> {
        sqlx::query!(
            r#"
            INSERT INTO content_group_items (group_id, content_id, sort_order)
            VALUES ($1,$2,$3)
            ON CONFLICT (group_id, content_id) DO UPDATE SET sort_order = EXCLUDED.sort_order
            "#,
            group_id,
            content_id,
            sort_order
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(())
    }

    async fn group_items_remove(
        &self,
        group_id: uuid::Uuid,
        content_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        let res = sqlx::query!(
            r#"
            DELETE FROM content_group_items
            WHERE group_id = $1 AND content_id = $2
            "#,
            group_id,
            content_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn group_items_list(
        &self,
        group_id: uuid::Uuid,
    ) -> Result<Vec<ContentGroupItemRow>, SqlStorageError> {
        let recs = sqlx::query!(
            r#"
            SELECT id, group_id, content_id, sort_order, added_at
            FROM content_group_items
            WHERE group_id = $1
            ORDER BY sort_order ASC, added_at ASC
            "#,
            group_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(recs
            .into_iter()
            .map(|rec| ContentGroupItemRow {
                id: rec.id,
                group_id: rec.group_id,
                content_id: rec.content_id,
                sort_order: rec.sort_order,
                added_at: rec.added_at,
            })
            .collect())
    }

    async fn tags_create(&self, input: TagCreate) -> Result<TagRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO tags (user_id, name, color)
            VALUES ($1,$2,$3)
            RETURNING id, user_id, name, color, created_at
            "#,
            input.user_id,
            input.name,
            input.color
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e
                && db.constraint() == Some("tags_unique_per_user")
            {
                return SqlStorageError::Conflict;
            }
            SqlStorageError::Db(e.to_string())
        })?;

        Ok(TagRow {
            id: rec.id,
            user_id: rec.user_id,
            name: rec.name,
            color: rec.color,
            created_at: rec.created_at,
        })
    }

    async fn tags_list_for_user(
        &self,
        user_id: uuid::Uuid,
    ) -> Result<Vec<TagRow>, SqlStorageError> {
        let recs = sqlx::query!(
            r#"
            SELECT id, user_id, name, color, created_at
            FROM tags
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(recs
            .into_iter()
            .map(|rec| TagRow {
                id: rec.id,
                user_id: rec.user_id,
                name: rec.name,
                color: rec.color,
                created_at: rec.created_at,
            })
            .collect())
    }

    async fn tags_delete(
        &self,
        user_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        let res = sqlx::query!(
            r#"
            DELETE FROM tags
            WHERE id = $1 AND user_id = $2
            "#,
            tag_id,
            user_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn tags_update(
        &self,
        user_id: uuid::Uuid,
        tag_id: uuid::Uuid,
        input: TagUpdate,
    ) -> Result<Option<TagRow>, SqlStorageError> {
        // Build dynamic update query based on provided fields
        let rec = sqlx::query!(
            r#"
            UPDATE tags
            SET
                name = COALESCE($3, name),
                color = CASE WHEN $4 THEN $5 ELSE color END
            WHERE id = $1 AND user_id = $2
            RETURNING id, user_id, name, color, created_at
            "#,
            tag_id,
            user_id,
            input.name,
            input.color.is_some(), // $4: whether to update color
            input.color.flatten(), // $5: the new color value (can be NULL)
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e
                && db.constraint() == Some("tags_unique_per_user")
            {
                return SqlStorageError::Conflict;
            }
            SqlStorageError::Db(e.to_string())
        })?;

        Ok(rec.map(|rec| TagRow {
            id: rec.id,
            user_id: rec.user_id,
            name: rec.name,
            color: rec.color,
            created_at: rec.created_at,
        }))
    }

    async fn content_tags_attach(
        &self,
        content_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> Result<(), SqlStorageError> {
        sqlx::query!(
            r#"
            INSERT INTO content_tags (content_id, tag_id)
            VALUES ($1,$2)
            ON CONFLICT (content_id, tag_id) DO NOTHING
            "#,
            content_id,
            tag_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(())
    }

    async fn content_tags_detach(
        &self,
        content_id: uuid::Uuid,
        tag_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        let res = sqlx::query!(
            r#"
            DELETE FROM content_tags
            WHERE content_id = $1 AND tag_id = $2
            "#,
            content_id,
            tag_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn content_tags_list_for_content(
        &self,
        content_id: uuid::Uuid,
    ) -> Result<Vec<TagRow>, SqlStorageError> {
        let recs = sqlx::query!(
            r#"
            SELECT t.id, t.user_id, t.name, t.color, t.created_at
            FROM content_tags ct
            JOIN tags t ON t.id = ct.tag_id
            WHERE ct.content_id = $1
            ORDER BY t.created_at DESC
            "#,
            content_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(recs
            .into_iter()
            .map(|rec| TagRow {
                id: rec.id,
                user_id: rec.user_id,
                name: rec.name,
                color: rec.color,
                created_at: rec.created_at,
            })
            .collect())
    }

    async fn share_links_create(
        &self,
        input: ShareLinkCreate,
    ) -> Result<ShareLinkRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO share_links (
                owner_id, token, name, permission, password_hash,
                max_access_count, expires_at, is_active
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,true)
            RETURNING
                id, owner_id, token, name, permission, password_hash,
                max_access_count, access_count, expires_at, is_active, created_at
            "#,
            input.owner_id,
            input.token,
            input.name,
            input.permission.as_db_str(),
            input.password_hash,
            input.max_access_count,
            input.expires_at
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ShareLinkRow {
            id: rec.id,
            owner_id: rec.owner_id,
            token: rec.token,
            name: rec.name,
            permission: rec.permission,
            password_hash: rec.password_hash,
            max_access_count: rec.max_access_count,
            access_count: rec.access_count,
            expires_at: rec.expires_at,
            is_active: rec.is_active,
            created_at: rec.created_at,
        })
    }

    async fn share_links_get_by_token(
        &self,
        token: &str,
    ) -> Result<Option<ShareLinkRow>, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            SELECT
                id, owner_id, token, name, permission, password_hash,
                max_access_count, access_count, expires_at, is_active, created_at
            FROM share_links
            WHERE token = $1
            "#,
            token
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(rec.map(|rec| ShareLinkRow {
            id: rec.id,
            owner_id: rec.owner_id,
            token: rec.token,
            name: rec.name,
            permission: rec.permission,
            password_hash: rec.password_hash,
            max_access_count: rec.max_access_count,
            access_count: rec.access_count,
            expires_at: rec.expires_at,
            is_active: rec.is_active,
            created_at: rec.created_at,
        }))
    }

    async fn share_links_list_for_owner(
        &self,
        owner_id: uuid::Uuid,
    ) -> Result<Vec<ShareLinkRow>, SqlStorageError> {
        let recs = sqlx::query!(
            r#"
            SELECT
                id, owner_id, token, name, permission, password_hash,
                max_access_count, access_count, expires_at, is_active, created_at
            FROM share_links
            WHERE owner_id = $1
            ORDER BY created_at DESC
            "#,
            owner_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(recs
            .into_iter()
            .map(|rec| ShareLinkRow {
                id: rec.id,
                owner_id: rec.owner_id,
                token: rec.token,
                name: rec.name,
                permission: rec.permission,
                password_hash: rec.password_hash,
                max_access_count: rec.max_access_count,
                access_count: rec.access_count,
                expires_at: rec.expires_at,
                is_active: rec.is_active,
                created_at: rec.created_at,
            })
            .collect())
    }

    async fn share_links_deactivate(
        &self,
        owner_id: uuid::Uuid,
        share_link_id: uuid::Uuid,
    ) -> Result<bool, SqlStorageError> {
        let res = sqlx::query!(
            r#"
            UPDATE share_links
            SET is_active = false
            WHERE id = $1 AND owner_id = $2
            "#,
            share_link_id,
            owner_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(res.rows_affected() > 0)
    }

    async fn content_shares_create_for_user(
        &self,
        input: ContentShareCreateForUser,
    ) -> Result<ContentShareRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO content_shares (
                content_id, shared_with_user_id, share_link_id, permission, created_by
            )
            VALUES ($1,$2,NULL,$3,$4)
            RETURNING id, content_id, shared_with_user_id, share_link_id, permission, created_at, created_by
            "#,
            input.content_id,
            input.shared_with_user_id,
            input.permission.as_db_str(),
            input.created_by
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ContentShareRow {
            id: rec.id,
            content_id: rec.content_id,
            shared_with_user_id: rec.shared_with_user_id,
            share_link_id: rec.share_link_id,
            permission: rec.permission,
            created_at: rec.created_at,
            created_by: rec.created_by,
        })
    }

    async fn content_shares_create_for_link(
        &self,
        input: ContentShareCreateForLink,
    ) -> Result<ContentShareRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO content_shares (
                content_id, shared_with_user_id, share_link_id, permission, created_by
            )
            VALUES ($1,NULL,$2,$3,$4)
            RETURNING id, content_id, shared_with_user_id, share_link_id, permission, created_at, created_by
            "#,
            input.content_id,
            input.share_link_id,
            input.permission.as_db_str(),
            input.created_by
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ContentShareRow {
            id: rec.id,
            content_id: rec.content_id,
            shared_with_user_id: rec.shared_with_user_id,
            share_link_id: rec.share_link_id,
            permission: rec.permission,
            created_at: rec.created_at,
            created_by: rec.created_by,
        })
    }

    async fn group_shares_create_for_user(
        &self,
        input: GroupShareCreateForUser,
    ) -> Result<ContentGroupShareRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO content_group_shares (
                group_id, shared_with_user_id, share_link_id, permission, created_by
            )
            VALUES ($1,$2,NULL,$3,$4)
            RETURNING id, group_id, shared_with_user_id, share_link_id, permission, created_at, created_by
            "#,
            input.group_id,
            input.shared_with_user_id,
            input.permission.as_db_str(),
            input.created_by
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ContentGroupShareRow {
            id: rec.id,
            group_id: rec.group_id,
            shared_with_user_id: rec.shared_with_user_id,
            share_link_id: rec.share_link_id,
            permission: rec.permission,
            created_at: rec.created_at,
            created_by: rec.created_by,
        })
    }

    async fn group_shares_create_for_link(
        &self,
        input: GroupShareCreateForLink,
    ) -> Result<ContentGroupShareRow, SqlStorageError> {
        let rec = sqlx::query!(
            r#"
            INSERT INTO content_group_shares (
                group_id, shared_with_user_id, share_link_id, permission, created_by
            )
            VALUES ($1,NULL,$2,$3,$4)
            RETURNING id, group_id, shared_with_user_id, share_link_id, permission, created_at, created_by
            "#,
            input.group_id,
            input.share_link_id,
            input.permission.as_db_str(),
            input.created_by
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| SqlStorageError::Db(e.to_string()))?;

        Ok(ContentGroupShareRow {
            id: rec.id,
            group_id: rec.group_id,
            shared_with_user_id: rec.shared_with_user_id,
            share_link_id: rec.share_link_id,
            permission: rec.permission,
            created_at: rec.created_at,
            created_by: rec.created_by,
        })
    }
}
