//! User storage trait and implementations.
//!
//! This module provides a trait-based abstraction for user storage operations,
//! specifically for internal user management (user creation with OTP secrets).
//!
//! # Architecture
//!
//! The module follows the repository pattern with trait-based abstraction:
//! - `UserStorage<E>` trait: Generic interface for user storage operations
//! - `PgUserStorage`: PostgreSQL implementation using the existing `PgStorage`
//! - `MockUserStorage`: In-memory implementation for testing
//!
//! # Usage
//!
//! The trait is generic over an error type `E` to allow different implementations
//! to use their own error types while maintaining a consistent interface.
//!
//! ```rust,ignore
//! use collects_services::users::storage::{UserStorage, MockUserStorage};
//!
//! async fn create_user_example<S: UserStorage<E>, E>(storage: &S) {
//!     let user = storage.create_user("alice", "BASE32SECRET").await;
//!     // handle result...
//! }
//! ```

use crate::database::PgStorage;
use chrono::{DateTime, Utc};
use std::future::Future;
use uuid::Uuid;

/// Represents a stored user with their OTP secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUser {
    /// The unique user ID.
    pub id: Uuid,
    /// The unique username.
    pub username: String,
    /// The base32-encoded OTP secret.
    pub secret: String,
    /// The user's nickname (optional).
    pub nickname: Option<String>,
    /// The user's avatar URL (optional).
    pub avatar_url: Option<String>,
    /// When the user was created.
    pub created_at: DateTime<Utc>,
    /// When the user was last updated.
    pub updated_at: DateTime<Utc>,
}

impl StoredUser {
    /// Creates a new `StoredUser` instance with a generated UUID.
    pub fn new(username: impl Into<String>, secret: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            username: username.into(),
            secret: secret.into(),
            nickname: None,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a new `StoredUser` instance with a specific UUID.
    pub fn with_id(id: Uuid, username: impl Into<String>, secret: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id,
            username: username.into(),
            secret: secret.into(),
            nickname: None,
            avatar_url: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a `StoredUser` with all profile fields.
    pub fn with_profile(
        id: Uuid,
        username: impl Into<String>,
        secret: impl Into<String>,
        nickname: Option<String>,
        avatar_url: Option<String>,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            username: username.into(),
            secret: secret.into(),
            nickname,
            avatar_url,
            created_at,
            updated_at,
        }
    }
}

/// Error type for user storage operations.
#[derive(Debug, thiserror::Error)]
pub enum UserStorageError {
    /// The user already exists.
    #[error("User already exists: {0}")]
    UserAlreadyExists(String),

    /// The user was not found.
    #[error("User not found: {0}")]
    UserNotFound(String),

    /// A database or storage error occurred.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Invalid input was provided.
    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Trait for user storage operations.
///
/// This trait provides an abstraction over user storage, allowing for different
/// implementations (PostgreSQL, in-memory mock, etc.) while maintaining a
/// consistent interface.
///
/// # Type Parameters
///
/// * `E` - The error type used by this storage implementation.
///
/// # Internal Use Only
///
/// This storage interface is designed for internal user management operations
/// and should only be exposed through internal routes protected by Zero Trust
/// or similar access control mechanisms.
pub trait UserStorage: Clone + Send + Sync + 'static {
    /// The error type for storage operations.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Creates a new user with the given username and OTP secret.
    ///
    /// # Arguments
    ///
    /// * `username` - The unique username for the new user.
    /// * `secret` - The base32-encoded OTP secret.
    ///
    /// # Returns
    ///
    /// Returns the created `StoredUser` on success, or an error if:
    /// - The username already exists
    /// - The username or secret is invalid
    /// - A storage error occurs
    fn create_user(
        &self,
        username: &str,
        secret: &str,
    ) -> impl Future<Output = Result<StoredUser, Self::Error>> + Send;

    /// Retrieves the OTP secret for a user.
    ///
    /// # Arguments
    ///
    /// * `username` - The username to look up.
    ///
    /// # Returns
    ///
    /// Returns `Some(secret)` if the user exists, `None` otherwise.
    fn get_user_secret(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<Option<String>, Self::Error>> + Send;

    /// Checks if a user exists.
    ///
    /// # Arguments
    ///
    /// * `username` - The username to check.
    ///
    /// # Returns
    ///
    /// Returns `true` if the user exists, `false` otherwise.
    fn user_exists(&self, username: &str)
    -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Deletes a user by username.
    ///
    /// # Arguments
    ///
    /// * `username` - The username of the user to delete.
    ///
    /// # Returns
    ///
    /// Returns `true` if the user was deleted, `false` if the user didn't exist.
    fn delete_user(&self, username: &str)
    -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Lists all users in the storage.
    ///
    /// # Returns
    ///
    /// Returns a vector of all stored users.
    fn list_users(&self) -> impl Future<Output = Result<Vec<StoredUser>, Self::Error>> + Send;

    /// Retrieves a user by username.
    ///
    /// # Arguments
    ///
    /// * `username` - The username to look up.
    ///
    /// # Returns
    ///
    /// Returns `Some(StoredUser)` if the user exists, `None` otherwise.
    fn get_user(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<Option<StoredUser>, Self::Error>> + Send;

    /// Updates the username of an existing user.
    ///
    /// # Arguments
    ///
    /// * `old_username` - The current username.
    /// * `new_username` - The new username to set.
    ///
    /// # Returns
    ///
    /// Returns the updated `StoredUser` on success, or an error if:
    /// - The old username doesn't exist
    /// - The new username already exists
    /// - The new username is invalid
    fn update_username(
        &self,
        old_username: &str,
        new_username: &str,
    ) -> impl Future<Output = Result<StoredUser, Self::Error>> + Send;

    /// Revokes the OTP secret for a user by generating a new one.
    ///
    /// # Arguments
    ///
    /// * `username` - The username of the user to revoke OTP for.
    /// * `new_secret` - The new base32-encoded OTP secret.
    ///
    /// # Returns
    ///
    /// Returns the updated `StoredUser` on success, or an error if:
    /// - The user doesn't exist
    /// - The new secret is invalid
    fn revoke_otp(
        &self,
        username: &str,
        new_secret: &str,
    ) -> impl Future<Output = Result<StoredUser, Self::Error>> + Send;

    /// Updates the profile (nickname and avatar URL) of an existing user.
    ///
    /// # Arguments
    ///
    /// * `username` - The username of the user to update.
    /// * `nickname` - The new nickname (None to keep existing, Some(None) to remove).
    /// * `avatar_url` - The new avatar URL (None to keep existing, Some(None) to remove).
    ///
    /// # Returns
    ///
    /// Returns the updated `StoredUser` on success, or an error if:
    /// - The user doesn't exist
    fn update_profile(
        &self,
        username: &str,
        nickname: Option<Option<String>>,
        avatar_url: Option<Option<String>>,
    ) -> impl Future<Output = Result<StoredUser, Self::Error>> + Send;
}

/// In-memory mock implementation of `UserStorage` for testing.
///
/// This implementation stores users in a thread-safe `HashMap` and is suitable
/// for unit tests and integration tests that don't require a real database.
///
/// # Example
///
/// ```rust,ignore
/// use collects_services::users::storage::{MockUserStorage, UserStorage};
///
/// #[tokio::test]
/// async fn test_user_creation() {
///     let storage = MockUserStorage::new();
///
///     let user = storage.create_user("alice", "SECRET123").await.unwrap();
///     assert_eq!(user.username, "alice");
///
///     let secret = storage.get_user_secret("alice").await.unwrap();
///     assert_eq!(secret, Some("SECRET123".to_owned()));
/// }
/// ```
#[derive(Clone, Default)]
pub struct MockUserStorage {
    pub(crate) users:
        std::sync::Arc<std::sync::RwLock<std::collections::HashMap<String, StoredUser>>>,
}

impl MockUserStorage {
    /// Creates a new empty `MockUserStorage`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a `MockUserStorage` pre-populated with the given users.
    ///
    /// # Arguments
    ///
    /// * `users` - An iterator of `(username, secret)` tuples.
    pub fn with_users<I, S1, S2>(users: I) -> Self
    where
        I: IntoIterator<Item = (S1, S2)>,
        S1: Into<String>,
        S2: Into<String>,
    {
        let map: std::collections::HashMap<String, StoredUser> = users
            .into_iter()
            .map(|(username, secret)| {
                let username = username.into();
                let user = StoredUser::new(username.clone(), secret);
                (username, user)
            })
            .collect();

        Self {
            users: std::sync::Arc::new(std::sync::RwLock::new(map)),
        }
    }

    /// Returns the number of users in the storage.
    pub fn len(&self) -> usize {
        self.users.read().expect("lock poisoned").len()
    }

    /// Returns `true` if the storage is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clears all users from the storage.
    pub fn clear(&self) {
        self.users.write().expect("lock poisoned").clear();
    }

    /// Inserts a user with a specific ID into the storage (builder pattern).
    ///
    /// This is useful for integration tests that need to coordinate user IDs
    /// between `MockUserStorage` and other mock storages (e.g., `MockSqlStorage`).
    ///
    /// # Example
    ///
    /// ```
    /// use collects_services::users::storage::{MockUserStorage, StoredUser};
    /// use uuid::Uuid;
    ///
    /// let test_user_id = Uuid::from_u128(0x00000000_0000_0000_0000_000000000001);
    /// let user = StoredUser::with_id(test_user_id, "testuser", "SECRET123");
    /// let storage = MockUserStorage::new().with_user(user);
    ///
    /// // Now get_user("testuser") will return a user with test_user_id
    /// ```
    pub fn with_user(self, user: StoredUser) -> Self {
        self.users
            .write()
            .expect("lock poisoned")
            .insert(user.username.clone(), user);
        self
    }
}

impl UserStorage for MockUserStorage {
    type Error = UserStorageError;

    async fn create_user(&self, username: &str, secret: &str) -> Result<StoredUser, Self::Error> {
        if username.is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Username cannot be empty".to_owned(),
            ));
        }

        if secret.is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Secret cannot be empty".to_owned(),
            ));
        }

        let mut users = self.users.write().expect("lock poisoned");

        if users.contains_key(username) {
            return Err(UserStorageError::UserAlreadyExists(username.to_owned()));
        }

        let user = StoredUser::new(username, secret);
        users.insert(username.to_owned(), user.clone());

        Ok(user)
    }

    async fn get_user_secret(&self, username: &str) -> Result<Option<String>, Self::Error> {
        let users = self.users.read().expect("lock poisoned");
        Ok(users.get(username).map(|u| u.secret.clone()))
    }

    async fn user_exists(&self, username: &str) -> Result<bool, Self::Error> {
        let users = self.users.read().expect("lock poisoned");
        Ok(users.contains_key(username))
    }

    async fn delete_user(&self, username: &str) -> Result<bool, Self::Error> {
        let mut users = self.users.write().expect("lock poisoned");
        Ok(users.remove(username).is_some())
    }

    async fn list_users(&self) -> Result<Vec<StoredUser>, Self::Error> {
        let users = self.users.read().expect("lock poisoned");
        Ok(users.values().cloned().collect())
    }

    async fn get_user(&self, username: &str) -> Result<Option<StoredUser>, Self::Error> {
        let users = self.users.read().expect("lock poisoned");
        Ok(users.get(username).cloned())
    }

    async fn update_username(
        &self,
        old_username: &str,
        new_username: &str,
    ) -> Result<StoredUser, Self::Error> {
        if new_username.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Username cannot be empty".to_owned(),
            ));
        }

        let mut users = self.users.write().expect("lock poisoned");

        // Check if old user exists
        let old_user = users
            .get(old_username)
            .cloned()
            .ok_or_else(|| UserStorageError::UserNotFound(old_username.to_owned()))?;

        // Check if new username is already taken (unless it's the same)
        if old_username != new_username && users.contains_key(new_username) {
            return Err(UserStorageError::UserAlreadyExists(new_username.to_owned()));
        }

        // Remove old entry and insert new one, preserving profile data
        users.remove(old_username);
        let updated_user = StoredUser::with_profile(
            old_user.id,
            new_username,
            &old_user.secret,
            old_user.nickname,
            old_user.avatar_url,
            old_user.created_at,
            Utc::now(),
        );
        users.insert(new_username.to_owned(), updated_user.clone());

        Ok(updated_user)
    }

    async fn revoke_otp(
        &self,
        username: &str,
        new_secret: &str,
    ) -> Result<StoredUser, Self::Error> {
        if new_secret.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Secret cannot be empty".to_owned(),
            ));
        }

        let mut users = self.users.write().expect("lock poisoned");

        // Check if user exists
        let old_user = users
            .get(username)
            .cloned()
            .ok_or_else(|| UserStorageError::UserNotFound(username.to_owned()))?;

        // Update the secret, preserving profile data
        let updated_user = StoredUser::with_profile(
            old_user.id,
            username,
            new_secret,
            old_user.nickname,
            old_user.avatar_url,
            old_user.created_at,
            Utc::now(),
        );
        users.insert(username.to_owned(), updated_user.clone());

        Ok(updated_user)
    }

    async fn update_profile(
        &self,
        username: &str,
        nickname: Option<Option<String>>,
        avatar_url: Option<Option<String>>,
    ) -> Result<StoredUser, Self::Error> {
        let mut users = self.users.write().expect("lock poisoned");

        // Check if user exists
        let old_user = users
            .get(username)
            .cloned()
            .ok_or_else(|| UserStorageError::UserNotFound(username.to_owned()))?;

        // Update the profile fields
        let new_nickname = match nickname {
            Some(value) => value,      // Explicitly set (or clear)
            None => old_user.nickname, // Keep existing
        };
        let new_avatar_url = match avatar_url {
            Some(value) => value,        // Explicitly set (or clear)
            None => old_user.avatar_url, // Keep existing
        };

        let updated_user = StoredUser::with_profile(
            old_user.id,
            username,
            &old_user.secret,
            new_nickname,
            new_avatar_url,
            old_user.created_at,
            Utc::now(),
        );
        users.insert(username.to_owned(), updated_user.clone());

        Ok(updated_user)
    }
}

/// PostgreSQL implementation of `UserStorage` for production use.
///
/// This implementation uses the existing `PgStorage` connection pool
/// to persist user data in a PostgreSQL database.
///
/// # Table Schema
///
/// This implementation uses the existing `users` table schema:
///
/// ```sql
/// CREATE TABLE users (
///     id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
///     username VARCHAR(50) NOT NULL UNIQUE,
///     otp_secret TEXT NOT NULL,  -- Base32 encoded OTP secret
///     -- ... other fields
/// );
/// ```
///
/// # Example
///
/// ```rust,ignore
/// use collects_services::database::PgStorage;
/// use collects_services::users::storage::{PgUserStorage, UserStorage};
///
/// async fn example(pg_storage: PgStorage) {
///     let user_storage = PgUserStorage::new(pg_storage);
///     let user = user_storage.create_user("alice", "SECRET123").await?;
///     println!("Created user: {}", user.username);
/// }
/// ```
#[derive(Clone)]
pub struct PgUserStorage {
    storage: PgStorage,
}

/// Row type for user queries with all fields.
#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    username: String,
    otp_secret: String,
    nickname: Option<String>,
    avatar_url: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl PgUserStorage {
    /// Creates a new `PgUserStorage` instance wrapping the given `PgStorage`.
    pub fn new(storage: PgStorage) -> Self {
        Self { storage }
    }

    /// Returns a reference to the underlying `PgStorage`.
    pub fn inner(&self) -> &PgStorage {
        &self.storage
    }
}

impl UserStorage for PgUserStorage {
    type Error = UserStorageError;

    async fn create_user(&self, username: &str, secret: &str) -> Result<StoredUser, Self::Error> {
        if username.is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Username cannot be empty".to_owned(),
            ));
        }

        if secret.is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Secret cannot be empty".to_owned(),
            ));
        }

        // Insert the user into the database and return all fields
        let result = sqlx::query_as!(
            UserRow,
            r#"
            INSERT INTO users (username, otp_secret)
            VALUES ($1, $2)
            ON CONFLICT (username) DO NOTHING
            RETURNING id, username, otp_secret, nickname, avatar_url, created_at, updated_at
            "#,
            username,
            secret,
        )
        .fetch_optional(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        match result {
            Some(row) => Ok(StoredUser::with_profile(
                row.id,
                row.username,
                row.otp_secret,
                row.nickname,
                row.avatar_url,
                row.created_at,
                row.updated_at,
            )),
            None => Err(UserStorageError::UserAlreadyExists(username.to_owned())),
        }
    }

    async fn get_user_secret(&self, username: &str) -> Result<Option<String>, Self::Error> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT otp_secret FROM users WHERE username = $1 AND status = 'active'
            "#,
            username,
        )
        .fetch_optional(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        Ok(result)
    }

    async fn user_exists(&self, username: &str) -> Result<bool, Self::Error> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(SELECT 1 FROM users WHERE username = $1 AND status = 'active') AS "exists!"
            "#,
            username,
        )
        .fetch_one(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        Ok(result)
    }

    async fn delete_user(&self, username: &str) -> Result<bool, Self::Error> {
        let result = sqlx::query!(
            r#"
            DELETE FROM users WHERE username = $1
            "#,
            username,
        )
        .execute(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn list_users(&self) -> Result<Vec<StoredUser>, Self::Error> {
        let rows = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, username, otp_secret, nickname, avatar_url, created_at, updated_at
            FROM users
            WHERE status = 'active'
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| {
                StoredUser::with_profile(
                    row.id,
                    row.username,
                    row.otp_secret,
                    row.nickname,
                    row.avatar_url,
                    row.created_at,
                    row.updated_at,
                )
            })
            .collect())
    }

    async fn get_user(&self, username: &str) -> Result<Option<StoredUser>, Self::Error> {
        let result = sqlx::query_as!(
            UserRow,
            r#"
            SELECT id, username, otp_secret, nickname, avatar_url, created_at, updated_at
            FROM users
            WHERE username = $1 AND status = 'active'
            "#,
            username,
        )
        .fetch_optional(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        Ok(result.map(|row| {
            StoredUser::with_profile(
                row.id,
                row.username,
                row.otp_secret,
                row.nickname,
                row.avatar_url,
                row.created_at,
                row.updated_at,
            )
        }))
    }

    async fn update_username(
        &self,
        old_username: &str,
        new_username: &str,
    ) -> Result<StoredUser, Self::Error> {
        if new_username.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Username cannot be empty".to_owned(),
            ));
        }

        // Update the username and return the updated user
        let result = sqlx::query_as!(
            UserRow,
            r#"
            UPDATE users
            SET username = $2
            WHERE username = $1 AND status = 'active'
            RETURNING id, username, otp_secret, nickname, avatar_url, created_at, updated_at
            "#,
            old_username,
            new_username,
        )
        .fetch_optional(&self.storage.pool)
        .await
        .map_err(|e| {
            // Check for unique constraint violation
            let error_str = e.to_string();
            if error_str.contains("duplicate key") || error_str.contains("unique constraint") {
                return UserStorageError::UserAlreadyExists(new_username.to_owned());
            }
            UserStorageError::StorageError(error_str)
        })?;

        result
            .map(|row| {
                StoredUser::with_profile(
                    row.id,
                    row.username,
                    row.otp_secret,
                    row.nickname,
                    row.avatar_url,
                    row.created_at,
                    row.updated_at,
                )
            })
            .ok_or_else(|| UserStorageError::UserNotFound(old_username.to_owned()))
    }

    async fn revoke_otp(
        &self,
        username: &str,
        new_secret: &str,
    ) -> Result<StoredUser, Self::Error> {
        if new_secret.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Secret cannot be empty".to_owned(),
            ));
        }

        // Update the OTP secret and return the updated user
        let result = sqlx::query_as!(
            UserRow,
            r#"
            UPDATE users
            SET otp_secret = $2
            WHERE username = $1 AND status = 'active'
            RETURNING id, username, otp_secret, nickname, avatar_url, created_at, updated_at
            "#,
            username,
            new_secret,
        )
        .fetch_optional(&self.storage.pool)
        .await
        .map_err(|e| UserStorageError::StorageError(e.to_string()))?;

        result
            .map(|row| {
                StoredUser::with_profile(
                    row.id,
                    row.username,
                    row.otp_secret,
                    row.nickname,
                    row.avatar_url,
                    row.created_at,
                    row.updated_at,
                )
            })
            .ok_or_else(|| UserStorageError::UserNotFound(username.to_owned()))
    }

    async fn update_profile(
        &self,
        username: &str,
        nickname: Option<Option<String>>,
        avatar_url: Option<Option<String>>,
    ) -> Result<StoredUser, Self::Error> {
        // Build the query dynamically based on which fields are being updated
        let result = match (nickname, avatar_url) {
            (Some(nick), Some(avatar)) => sqlx::query_as!(
                UserRow,
                r#"
                UPDATE users
                SET nickname = $2, avatar_url = $3
                WHERE username = $1 AND status = 'active'
                RETURNING id, username, otp_secret, nickname, avatar_url, created_at, updated_at
                "#,
                username,
                nick,
                avatar,
            )
            .fetch_optional(&self.storage.pool)
            .await
            .map_err(|e| UserStorageError::StorageError(e.to_string()))?,
            (Some(nick), None) => sqlx::query_as!(
                UserRow,
                r#"
                UPDATE users
                SET nickname = $2
                WHERE username = $1 AND status = 'active'
                RETURNING id, username, otp_secret, nickname, avatar_url, created_at, updated_at
                "#,
                username,
                nick,
            )
            .fetch_optional(&self.storage.pool)
            .await
            .map_err(|e| UserStorageError::StorageError(e.to_string()))?,
            (None, Some(avatar)) => sqlx::query_as!(
                UserRow,
                r#"
                UPDATE users
                SET avatar_url = $2
                WHERE username = $1 AND status = 'active'
                RETURNING id, username, otp_secret, nickname, avatar_url, created_at, updated_at
                "#,
                username,
                avatar,
            )
            .fetch_optional(&self.storage.pool)
            .await
            .map_err(|e| UserStorageError::StorageError(e.to_string()))?,
            (None, None) => {
                // No updates, just fetch the user
                return self
                    .get_user(username)
                    .await?
                    .ok_or_else(|| UserStorageError::UserNotFound(username.to_owned()));
            }
        };

        result
            .map(|row| {
                StoredUser::with_profile(
                    row.id,
                    row.username,
                    row.otp_secret,
                    row.nickname,
                    row.avatar_url,
                    row.created_at,
                    row.updated_at,
                )
            })
            .ok_or_else(|| UserStorageError::UserNotFound(username.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_create_user_success() {
        let storage = MockUserStorage::new();

        let user = storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        assert_eq!(user.username, "alice");
        assert_eq!(user.secret, "SECRET123");
        assert_eq!(storage.len(), 1);
    }

    #[tokio::test]
    async fn test_mock_create_user_duplicate() {
        let storage = MockUserStorage::new();

        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        let result = storage.create_user("alice", "ANOTHER_SECRET").await;

        assert!(result.is_err());
        match result {
            Err(UserStorageError::UserAlreadyExists(username)) => {
                assert_eq!(username, "alice");
            }
            _ => panic!("Expected UserAlreadyExists error"),
        }
    }

    #[tokio::test]
    async fn test_mock_create_user_empty_username() {
        let storage = MockUserStorage::new();

        let result = storage.create_user("", "SECRET123").await;

        assert!(result.is_err());
        assert!(matches!(result, Err(UserStorageError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_mock_create_user_empty_secret() {
        let storage = MockUserStorage::new();

        let result = storage.create_user("alice", "").await;

        assert!(result.is_err());
        assert!(matches!(result, Err(UserStorageError::InvalidInput(_))));
    }

    #[tokio::test]
    async fn test_mock_get_user_secret_exists() {
        let storage = MockUserStorage::new();

        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        let secret = storage
            .get_user_secret("alice")
            .await
            .expect("should not error");

        assert_eq!(secret, Some("SECRET123".to_owned()));
    }

    #[tokio::test]
    async fn test_mock_get_user_secret_not_exists() {
        let storage = MockUserStorage::new();

        let secret = storage
            .get_user_secret("nonexistent")
            .await
            .expect("should not error");

        assert_eq!(secret, None);
    }

    #[tokio::test]
    async fn test_mock_user_exists() {
        let storage = MockUserStorage::new();

        assert!(
            !storage
                .user_exists("alice")
                .await
                .expect("should not error")
        );

        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        assert!(
            storage
                .user_exists("alice")
                .await
                .expect("should not error")
        );
    }

    #[tokio::test]
    async fn test_mock_delete_user() {
        let storage = MockUserStorage::new();

        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        assert!(
            storage
                .user_exists("alice")
                .await
                .expect("should not error")
        );

        let deleted = storage
            .delete_user("alice")
            .await
            .expect("should not error");
        assert!(deleted);

        assert!(
            !storage
                .user_exists("alice")
                .await
                .expect("should not error")
        );
    }

    #[tokio::test]
    async fn test_mock_delete_nonexistent_user() {
        let storage = MockUserStorage::new();

        let deleted = storage
            .delete_user("nonexistent")
            .await
            .expect("should not error");
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_mock_with_users() {
        let storage = MockUserStorage::with_users([
            ("alice", "SECRET_A"),
            ("bob", "SECRET_B"),
            ("charlie", "SECRET_C"),
        ]);

        assert_eq!(storage.len(), 3);
        assert!(
            storage
                .user_exists("alice")
                .await
                .expect("should not error")
        );
        assert!(storage.user_exists("bob").await.expect("should not error"));
        assert!(
            storage
                .user_exists("charlie")
                .await
                .expect("should not error")
        );

        assert_eq!(
            storage
                .get_user_secret("bob")
                .await
                .expect("should not error"),
            Some("SECRET_B".to_owned())
        );
    }

    #[tokio::test]
    async fn test_mock_clear() {
        let storage = MockUserStorage::with_users([("alice", "SECRET_A"), ("bob", "SECRET_B")]);

        assert_eq!(storage.len(), 2);

        storage.clear();

        assert!(storage.is_empty());
        assert!(
            !storage
                .user_exists("alice")
                .await
                .expect("should not error")
        );
    }

    #[tokio::test]
    async fn test_stored_user_new() {
        let user = StoredUser::new("alice", "SECRET123");

        assert_eq!(user.username, "alice");
        assert_eq!(user.secret, "SECRET123");
    }

    #[tokio::test]
    async fn test_mock_storage_is_clone() {
        let storage1 = MockUserStorage::new();
        storage1
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        let storage2 = storage1.clone();

        // Both should see the same data (Arc shared)
        assert!(
            storage2
                .user_exists("alice")
                .await
                .expect("should not error")
        );

        // Changes through one should be visible in the other
        storage2
            .create_user("bob", "SECRET456")
            .await
            .expect("should create user");
        assert!(storage1.user_exists("bob").await.expect("should not error"));
    }

    // Test that the trait works with generic functions
    async fn generic_create_user<S: UserStorage>(
        storage: &S,
        username: &str,
        secret: &str,
    ) -> Result<StoredUser, S::Error> {
        storage.create_user(username, secret).await
    }

    #[tokio::test]
    async fn test_generic_trait_usage() {
        let storage = MockUserStorage::new();

        let user = generic_create_user(&storage, "alice", "SECRET123")
            .await
            .expect("should create user");

        assert_eq!(user.username, "alice");
    }

    #[tokio::test]
    async fn test_mock_list_users() {
        let storage = MockUserStorage::new();

        // List should be empty initially
        let users = storage.list_users().await.expect("should not error");
        assert!(users.is_empty());

        // Add some users
        storage
            .create_user("alice", "SECRET_A")
            .await
            .expect("should create user");
        storage
            .create_user("bob", "SECRET_B")
            .await
            .expect("should create user");

        // List should contain both users
        let users = storage.list_users().await.expect("should not error");
        assert_eq!(users.len(), 2);

        let usernames: Vec<&str> = users.iter().map(|u| u.username.as_str()).collect();
        assert!(usernames.contains(&"alice"));
        assert!(usernames.contains(&"bob"));
    }

    #[tokio::test]
    async fn test_mock_update_profile_nickname_and_avatar() {
        let storage = MockUserStorage::new();

        // Create a user first
        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        // Update profile with nickname and avatar
        let updated = storage
            .update_profile(
                "alice",
                Some(Some("Alice Nickname".to_owned())),
                Some(Some("https://example.com/avatar.png".to_owned())),
            )
            .await
            .expect("should update profile");

        assert_eq!(updated.username, "alice");
        assert_eq!(updated.nickname, Some("Alice Nickname".to_owned()));
        assert_eq!(
            updated.avatar_url,
            Some("https://example.com/avatar.png".to_owned())
        );
    }

    #[tokio::test]
    async fn test_mock_update_profile_nickname_only() {
        let storage = MockUserStorage::new();

        // Create a user first
        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        // Update only nickname
        let updated = storage
            .update_profile("alice", Some(Some("New Nickname".to_owned())), None)
            .await
            .expect("should update profile");

        assert_eq!(updated.nickname, Some("New Nickname".to_owned()));
        assert_eq!(updated.avatar_url, None);
    }

    #[tokio::test]
    async fn test_mock_update_profile_clear_fields() {
        let storage = MockUserStorage::new();

        // Create a user first
        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        // Set initial values
        storage
            .update_profile(
                "alice",
                Some(Some("Initial".to_owned())),
                Some(Some("https://example.com/initial.png".to_owned())),
            )
            .await
            .expect("should update profile");

        // Clear fields by setting to None
        let updated = storage
            .update_profile("alice", Some(None), Some(None))
            .await
            .expect("should update profile");

        assert_eq!(updated.nickname, None);
        assert_eq!(updated.avatar_url, None);
    }

    #[tokio::test]
    async fn test_mock_update_profile_user_not_found() {
        let storage = MockUserStorage::new();

        let result = storage
            .update_profile("nonexistent", Some(Some("Nick".to_owned())), Some(None))
            .await;

        assert!(result.is_err());
        assert!(matches!(result, Err(UserStorageError::UserNotFound(_))));
    }

    #[tokio::test]
    async fn test_mock_update_profile_no_changes() {
        let storage = MockUserStorage::new();

        // Create a user first
        storage
            .create_user("alice", "SECRET123")
            .await
            .expect("should create user");

        // Call update_profile with no changes (both None)
        let updated = storage
            .update_profile("alice", None, None)
            .await
            .expect("should update profile");

        assert_eq!(updated.username, "alice");
        // User should be returned unchanged
    }

    #[tokio::test]
    async fn test_stored_user_with_profile() {
        let now = Utc::now();
        let id = Uuid::new_v4();
        let user = StoredUser::with_profile(
            id,
            "alice",
            "SECRET123",
            Some("Nickname".to_owned()),
            Some("https://example.com/avatar.png".to_owned()),
            now,
            now,
        );

        assert_eq!(user.id, id);
        assert_eq!(user.username, "alice");
        assert_eq!(user.secret, "SECRET123");
        assert_eq!(user.nickname, Some("Nickname".to_owned()));
        assert_eq!(
            user.avatar_url,
            Some("https://example.com/avatar.png".to_owned())
        );
        assert_eq!(user.created_at, now);
        assert_eq!(user.updated_at, now);
    }
}
