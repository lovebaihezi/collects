# Collects Services — API TODO

Tracks remaining work for the `collects/services` API. For completed features, see git history.

---



## Code Patterns & Concepts (from recent development)

### 1) Handler Structure Pattern
All v1 handlers follow this pattern in `src/v1/*.rs`:

```rust
pub async fn v1_endpoint_name<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,                    // JWT validation via extractor
    Path(id): Path<String>,               // URL params
    Query(query): Query<QueryType>,       // Query params (optional)
    Json(payload): Json<RequestType>,     // Body (optional)
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
```

### 2) User Resolution Pattern
Handlers resolve user ID from JWT username via `UserStorage`:

```rust
let user = match state.user_storage.get_user(auth.username()).await {
    Ok(Some(user)) => user,
    Ok(None) => return (StatusCode::UNAUTHORIZED, Json(V1ErrorResponse::not_found("User not found"))).into_response(),
    Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(V1ErrorResponse::internal_error("..."))).into_response(),
};
```

### 3) Ownership Verification Pattern
After fetching a resource, verify ownership before returning:

```rust
if row.user_id != user.id {
    return (StatusCode::NOT_FOUND, Json(V1ErrorResponse::not_found("..."))).into_response();
}
```

### 4) SqlStorage Trait
All database operations are abstracted via `SqlStorage` trait (`src/database.rs`):
- Trait methods return `Result<T, SqlStorageError>`
- `SqlStorageError::Unauthorized` for permission violations
- `PgStorage` implements the trait with actual Postgres queries

### 5) Type Conversion Pattern
DB rows → API responses via `From` impl in `src/v1/types.rs`:

```rust
impl From<ContentRow> for V1ContentItem {
    fn from(row: ContentRow) -> Self { ... }
}
```

### 6) Lifecycle Actions Pattern
Lifecycle endpoints (`trash`, `restore`, `archive`, `unarchive`) share a helper:

```rust
async fn v1_contents_set_status<S, U>(state, auth, id, new_status: ContentStatus) -> impl IntoResponse
```

### 7) OpenAPI Documentation (feature-gated)
Types use `#[cfg_attr(feature = "openapi", derive(ToSchema))]` for utoipa integration.

---

## Priority 1: Upload Flow (MVP Blocker)

### R2 Credential Requirement ✅
R2 credentials (`CF_ACCOUNT_ID`, `CF_ACCESS_KEY_ID`, `CF_SECRET_ACCESS_KEY`, `CF_BUCKET`) are now **required** for non-local environments.

**Implementation**:
- Config validation in `src/config.rs` — service fails to start without R2 creds
- CI integration via `scripts::r2-secrets` command
- `gcloud-deploy` automatically includes R2 secrets from Google Secret Manager
- Environments requiring R2: `prod`, `internal`, `nightly`, `pr`
- Environments where R2 is optional: `local`, `test`, `test-internal`

### Presigned URL Generation ✅
- [x] Implement R2 presigning (S3 SigV4) — `src/storage/presign.rs`
  - [x] `presign_put(storage_key, content_type, expires)` → signed PUT URL
  - [x] `presign_get(storage_key, disposition, expires)` → signed GET URL
- [ ] Implement GCS V4 signing (if needed)

### Upload Init Endpoint ✅
`POST /v1/uploads/init` — implemented in `src/v1/uploads.rs`:
- [x] Generate unique `storage_key` (e.g., `{user_id}/{uuid}/{filename}`)
- [x] Generate presigned PUT URL for R2
- [x] Return `{ upload_id, storage_key, upload_url, expires_at }`

### Upload Complete Endpoint ✅
`POST /v1/uploads/complete` — implemented in `src/v1/uploads.rs`:
- [x] Validate object exists via HEAD/stat
- [x] Create `contents` row with `(storage_backend, storage_profile, storage_key)`
- [ ] Write audit log entry (deferred to observability priority)

### View URL Endpoint ✅
`POST /v1/contents/:id/view-url` — implemented in `src/v1/contents.rs`:
- [x] Verify ownership
- [x] Read `(storage_backend, storage_profile, storage_key)` from content
- [x] Generate presigned GET URL
- [x] Return `{ url, expires_at }`

### Text Content Support ✅
- [x] Add migration for `body` and `kind` columns (`20260109101346_add-text-content-support.sql`)
- [x] Update `ContentsInsert` to accept `body` and `kind`
- [x] `POST /v1/contents` — create text content directly (no upload flow)
- [x] `PATCH /v1/contents/:id` — update `body` for text content
- [x] Return `body` in `GET /v1/contents/:id` for `kind=text`

---

## Priority 2: Auth Completion

- [ ] `POST /v1/auth/logout` — invalidate session
- [ ] Successful OTP resets lockout impact (optional policy)
- [ ] Auth event auditing (`audit_logs` entries for `auth.*` events)
- [ ] `PATCH /internal/users/:id/status` — set `active|suspended|archived`

---

## Priority 3: Internal API Security

Current gaps:
- Zero Trust + JWT both required but NOT enforced together
- Internal routes compiled in all builds (should be feature-gated)

TODO:
- [ ] Require BOTH Zero Trust token + JWT on `/internal/*`
- [ ] Conditional compilation: `#[cfg(feature = "internal")]` for internal routes
- [ ] Reject JWT-only requests to internal endpoints
- [ ] Reject Zero-Trust-only requests (need JWT to identify user)

---

## Priority 4: Authorization Rules

- [ ] Owner-can-access rule for all private assets
- [ ] Shares grant view/download permissions
- [ ] Visibility enforcement:
  - `private`: owner only (unless shared)
  - `restricted`: shared users/links only
  - `public`: accessible without auth

---

## Priority 5: Sharing API

**Share Links**
- [ ] `POST /v1/share-links` — create link with `name`, `permission`, `password`, `expires_at`, `max_access_count`
- [ ] `GET /v1/share-links` — list user's share links
- [ ] `PATCH /v1/share-links/:id` — update/disable
- [ ] `DELETE /v1/share-links/:id`

**Attach Shares**
- [ ] `POST /v1/contents/:id/share-link`
- [ ] `POST /v1/groups/:id/share-link`

**Public Access**
- [ ] `GET /v1/public/share/:token` — verify link, return metadata
- [ ] `POST /v1/public/share/:token/view-url` — return signed URL

---

## Priority 6: Observability & Safety

- [ ] Upload limits (max file size, allowed MIME types in config)
- [ ] Structured logging for upload init/complete, view-url generation
- [ ] `DELETE /v1/contents/:id` — hard delete + storage object removal

---

## Future (Post-MVP)

- [ ] `GET /v1/capabilities` — supported viewers, max upload, storage backends
- [ ] Multipart upload for large files
- [ ] Thumbnail generation (`POST /v1/contents/:id/thumbnail-url`)
- [ ] Content filtering: `visibility`, `content_type`, `q=` search, `tag=`
- [ ] PDF/CBZ/video viewers

---

## Completed ✅

**Text Content Storage** (hybrid approach)
- Migration: `kind` column (`file`/`text`) + `body` column for inline text
- `POST /v1/contents` — create text content directly (stored in DB, no R2)
- `PATCH /v1/contents/:id` — update `body` for text content
- Threshold: < 64KB stored inline, files go to R2
- Benefits: instant load for notes, full-text search ready, `kind` discriminator

**Auth**
- JWT token model with `RequireAuth` extractor
- OTP rate limiting via `otp_attempts` table
- `/v1/me`, `/auth/verify-otp`, `/auth/validate-token`

**Contents API** — full CRUD + lifecycle
- `GET /v1/contents`, `GET /v1/contents/:id`, `PATCH /v1/contents/:id`
- `POST /v1/contents/:id/{trash,restore,archive,unarchive}`

**Groups API** — full CRUD + lifecycle + items
- `GET/POST /v1/groups`, `GET/PATCH /v1/groups/:id`
- `POST /v1/groups/:id/{trash,restore,archive,unarchive}`
- `GET/POST /v1/groups/:id/contents`, `DELETE /v1/groups/:id/contents/:content_id`
- `PATCH /v1/groups/:id/contents/reorder`

**Tags API** — full CRUD + content association
- `GET/POST /v1/tags`, `PATCH/DELETE /v1/tags/:id`
- `GET/POST /v1/contents/:id/tags`, `DELETE /v1/contents/:id/tags/:tag_id`

**Internal Users API**
- CRUD: create, list, get, update, delete
- Actions: revoke OTP secret, update profile

**Infrastructure**
- SQL queries via `SqlStorage` trait + `PgStorage` impl
- Handler extraction into `src/v1/` submodules
- OpenAPI documentation (utoipa, feature-gated)
- Migration integrity checks (`.checksums.json`)