//! Integration test: Zero Trust enabled + POST /internal/users creates and persists a user.
//!
//! This test is deterministic and does not hit any real PostgreSQL.
//! It uses:
//! - a custom in-memory `UserStorage` implementation to verify persistence
//! - a custom JWKS resolver (no external network)
//! - an RSA (RS256) JWT minted at test time (strong 2048-bit key; "256" refers to SHA-256)

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use collects_services::{
    auth::{AccessClaims, JwksKeyResolver},
    config::Config,
    database::{
        ContentGroupItemRow, ContentGroupRow, ContentGroupShareRow, ContentRow, ContentShareRow,
        ContentStatus, ContentsInsert, ContentsListParams, ContentsUpdate, GroupCreate,
        GroupShareCreateForLink, GroupShareCreateForUser, GroupStatus, GroupUpdate,
        GroupsListParams, ShareLinkCreate, ShareLinkRow, SqlStorage, SqlStorageError, TagCreate,
        TagRow, TagUpdate,
    },
    internal,
    users::AppState,
    users::storage::{StoredUser, UserStorage, UserStorageError},
};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use std::{collections::HashMap, sync::Arc};
use time::{Duration, OffsetDateTime};
use tower::ServiceExt;

/// Minimal mock SQL storage: we never touch a real DB.
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
        _input: collects_services::database::ContentShareCreateForUser,
    ) -> Result<ContentShareRow, SqlStorageError> {
        Err(SqlStorageError::Db(
            "MockSqlStorage.content_shares_create_for_user: unimplemented".to_string(),
        ))
    }

    async fn content_shares_create_for_link(
        &self,
        _input: collects_services::database::ContentShareCreateForLink,
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
}

/// Recording/in-memory UserStorage.
/// We persist users so the test can assert "create user actually stored it".
#[derive(Clone, Default)]
struct RecordingUserStorage {
    users: Arc<std::sync::RwLock<HashMap<String, StoredUser>>>,
}

impl RecordingUserStorage {
    fn new() -> Self {
        Self::default()
    }

    fn get_user(&self, username: &str) -> Option<StoredUser> {
        self.users
            .read()
            .expect("lock poisoned")
            .get(username)
            .cloned()
    }

    fn len(&self) -> usize {
        self.users.read().expect("lock poisoned").len()
    }
}

impl UserStorage for RecordingUserStorage {
    type Error = UserStorageError;

    async fn create_user(&self, username: &str, secret: &str) -> Result<StoredUser, Self::Error> {
        if username.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Username cannot be empty".to_string(),
            ));
        }
        if secret.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Secret cannot be empty".to_string(),
            ));
        }

        let mut map = self.users.write().expect("lock poisoned");
        if map.contains_key(username) {
            return Err(UserStorageError::UserAlreadyExists(username.to_string()));
        }

        let user = StoredUser::new(username, secret);
        map.insert(username.to_string(), user.clone());
        Ok(user)
    }

    async fn get_user_secret(&self, username: &str) -> Result<Option<String>, Self::Error> {
        Ok(self
            .users
            .read()
            .expect("lock poisoned")
            .get(username)
            .map(|u| u.secret.clone()))
    }

    async fn user_exists(&self, username: &str) -> Result<bool, Self::Error> {
        Ok(self
            .users
            .read()
            .expect("lock poisoned")
            .contains_key(username))
    }

    async fn delete_user(&self, username: &str) -> Result<bool, Self::Error> {
        Ok(self
            .users
            .write()
            .expect("lock poisoned")
            .remove(username)
            .is_some())
    }

    async fn list_users(&self) -> Result<Vec<StoredUser>, Self::Error> {
        Ok(self
            .users
            .read()
            .expect("lock poisoned")
            .values()
            .cloned()
            .collect())
    }

    async fn get_user(&self, username: &str) -> Result<Option<StoredUser>, Self::Error> {
        Ok(self
            .users
            .read()
            .expect("lock poisoned")
            .get(username)
            .cloned())
    }

    async fn update_username(
        &self,
        old_username: &str,
        new_username: &str,
    ) -> Result<StoredUser, Self::Error> {
        if new_username.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Username cannot be empty".to_string(),
            ));
        }

        let mut map = self.users.write().expect("lock poisoned");
        let old_user = map
            .get(old_username)
            .cloned()
            .ok_or_else(|| UserStorageError::UserNotFound(old_username.to_string()))?;

        if old_username != new_username && map.contains_key(new_username) {
            return Err(UserStorageError::UserAlreadyExists(
                new_username.to_string(),
            ));
        }

        map.remove(old_username);
        let updated_user = StoredUser::new(new_username, &old_user.secret);
        map.insert(new_username.to_string(), updated_user.clone());
        Ok(updated_user)
    }

    async fn revoke_otp(
        &self,
        username: &str,
        new_secret: &str,
    ) -> Result<StoredUser, Self::Error> {
        if new_secret.trim().is_empty() {
            return Err(UserStorageError::InvalidInput(
                "Secret cannot be empty".to_string(),
            ));
        }

        let mut map = self.users.write().expect("lock poisoned");
        if !map.contains_key(username) {
            return Err(UserStorageError::UserNotFound(username.to_string()));
        }

        let updated_user = StoredUser::new(username, new_secret);
        map.insert(username.to_string(), updated_user.clone());
        Ok(updated_user)
    }

    async fn update_profile(
        &self,
        username: &str,
        nickname: Option<Option<String>>,
        avatar_url: Option<Option<String>>,
    ) -> Result<StoredUser, Self::Error> {
        let mut map = self.users.write().expect("lock poisoned");
        let old_user = map
            .get(username)
            .cloned()
            .ok_or_else(|| UserStorageError::UserNotFound(username.to_string()))?;

        let new_nickname = match nickname {
            Some(value) => value,
            None => old_user.nickname.clone(),
        };
        let new_avatar_url = match avatar_url {
            Some(value) => value,
            None => old_user.avatar_url.clone(),
        };

        let updated_user = StoredUser::with_profile(
            old_user.id,
            username,
            &old_user.secret,
            new_nickname,
            new_avatar_url,
            old_user.created_at,
            chrono::Utc::now(),
        );
        map.insert(username.to_string(), updated_user.clone());
        Ok(updated_user)
    }
}

/// A deterministic JWKS resolver backed by an RSA public key we generate for the test.
#[derive(Clone)]
struct TestJwksResolver {
    expected_kid: String,
    decoding_key: DecodingKey,
}

impl JwksKeyResolver for TestJwksResolver {
    fn resolve_decoding_key(
        &self,
        _jwks_url: String,
        kid: String,
    ) -> Pin<Box<dyn Future<Output = Result<DecodingKey, String>> + Send + 'static>> {
        let expected_kid = self.expected_kid.clone();
        let decoding_key = self.decoding_key.clone();

        Box::pin(async move {
            if kid != expected_kid {
                return Err(format!("unexpected kid: got={kid}, want={expected_kid}"));
            }
            Ok(decoding_key)
        })
    }
}

/// Generate a fresh RSA keypair and sign a token with RS256.
/// Uses 2048-bit RSA key material (strong), with SHA-256 (RS256).
fn mint_rs256_token(team_domain: &str, audience: &str, kid: &str) -> (String, DecodingKey) {
    // We add a dev-dependency on `rsa` + `rand` + `base64` in collects/services/Cargo.toml:
    //   rsa = { version = "0.9", features = ["pem"] }
    //   rand = "0.8"
    //   base64 = "0.22"
    //
    // This test assumes those are added.
    use rand::rngs::OsRng;
    use rsa::{RsaPrivateKey, pkcs1::EncodeRsaPrivateKey, pkcs8::EncodePublicKey};

    let mut rng = OsRng;

    // 2048-bit RSA key is a common baseline. If you want stronger, bump to 3072.
    let private = RsaPrivateKey::new(&mut rng, 2048).expect("generate RSA key");
    let public = private.to_public_key();

    // jsonwebtoken wants PEM for RSA keys.
    let private_pem = private
        .to_pkcs1_pem(rsa::pkcs1::LineEnding::LF)
        .expect("private key pem")
        .to_string();

    let public_spki_pem = public
        .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
        .expect("public key pem");

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());

    let now = OffsetDateTime::now_utc();
    let claims = AccessClaims {
        iss: format!("https://{team_domain}"),
        aud: vec![audience.to_string()],
        iat: now.unix_timestamp(),
        exp: (now + Duration::minutes(10)).unix_timestamp(),
        sub: "test-subject".to_string(),
        email: Some("tester@example.com".to_string()),
        custom: json!({"role":"internal-test"}),
    };

    let token = jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(private_pem.as_bytes()).expect("encoding key"),
    )
    .expect("sign jwt");

    let decoding = DecodingKey::from_rsa_pem(public_spki_pem.as_bytes()).expect("decoding key");

    (token, decoding)
}

#[tokio::test]
async fn test_internal_create_user_with_zerotrust_creates_and_persists_user() {
    // Arrange
    let team_domain = "myteam.cloudflareaccess.com";
    let audience = "collects-internal";
    let kid = "test-kid-1";

    let (token, decoding_key) = mint_rs256_token(team_domain, audience, kid);
    let resolver = Arc::new(TestJwksResolver {
        expected_kid: kid.to_string(),
        decoding_key,
    });

    let sql_storage = MockSqlStorage { is_connected: true };
    let user_storage = RecordingUserStorage::new();

    // IMPORTANT: we need Zero Trust enabled (config must carry team/aud).
    let config = Config::new_for_test_internal(team_domain, audience);

    // Build internal routes with injected resolver to avoid network calls.
    let internal_routes = internal::create_internal_routes_with_resolver::<
        MockSqlStorage,
        RecordingUserStorage,
    >(&config, resolver);

    // Create the full app (so state wiring matches production)
    let state = AppState::new(sql_storage, user_storage.clone());
    let app = axum::Router::new()
        .nest("/internal", internal_routes)
        .with_state(state);

    // Act: unauthenticated request should be blocked (when config enables Zero Trust)
    let unauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/users")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username":"alice"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    // If your Config helper hasn't enabled ZT yet, this might be 201.
    // Once enabled correctly, this should become 401.
    // Keep this assert as the target behavior:
    assert_eq!(unauth_response.status(), StatusCode::UNAUTHORIZED);

    // Act: authenticated request should pass and create user
    let auth_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/internal/users")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(r#"{"username":"alice"}"#))
                .expect("request"),
        )
        .await
        .expect("response");

    assert_eq!(auth_response.status(), StatusCode::CREATED);

    // Assert: user persisted into our in-memory storage
    assert_eq!(user_storage.len(), 1);
    let stored = user_storage.get_user("alice").expect("user exists");
    assert_eq!(stored.username, "alice");
    assert!(
        !stored.secret.trim().is_empty(),
        "secret should be generated and stored"
    );
}
