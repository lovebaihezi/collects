# Collects Services — API TODO (Storage, Content, Sharing)

This document tracks:
1) what the `collects/services` API **already has**
2) what we **need to build next**
3) recommended endpoint shapes (v1)

It’s written to support an app that stores **images, text/markdown, PDF, CBZ, video, audio, etc.**  
Today, native viewing support can be **image-only**, but the API should be future-proof.

---

## MVP Auth primitives (OTP-verified JWT sessions)

We already have OTP verification and issue JWTs. For MVP completeness, we still need to ensure the *auth primitives* are consistently applied across all protected APIs (uploads, contents, groups, tags, sharing).

### Token model (JWT)
- JWT is issued after OTP verification.
- JWT must include at least:
  - `sub` = user id (UUID)
  - `exp` = expiry
  - optional: `iat`, `iss`, `aud`
- JWT is signed with `JWT_SECRET` (already in `Config`).

User checks:
- You can decode a token and confirm `sub` and `exp` are present.
- Tokens are rejected when expired or signature is invalid.

### Request authentication primitive
Implement a single Axum extractor/middleware used by all authenticated routes, e.g.:
- `RequireAuth` / `CurrentUser`

Responsibilities:
- Read JWT from:
  - `Authorization: Bearer <token>` (recommended), or cookie (if you prefer)
- Verify signature + `exp`
- Load user from DB and enforce:
  - `users.status == 'active'`
- Attach user context to request handlers.

User checks:
- All `/v1/*` protected routes return `401` without a token.
- Suspended/archived users get `403` (or `401`) consistently.

### OTP rate limiting primitive (MVP safety)
We have `otp_attempts` table; enforce in OTP verify flow:
- Record every attempt (success/failure)
- Reject when too many attempts:
  - per `username` over time window
  - per `ip_address` over time window
- Use safe error messaging (avoid leaking whether a username exists).

User checks:
- Repeated wrong OTP attempts are throttled/blocked.
- A successful OTP resets or reduces lockout impact (policy-dependent).

### Auth event auditing (recommended for MVP)
Write `audit_logs` entries for:
- `auth.otp_verify_success`
- `auth.otp_verify_failure`
- `auth.logout`

User checks:
- Audit rows exist for auth events and include `ip_address` when available.

### Auth scope for routes (MVP)
Define route categories explicitly:
- Public:
  - `/is-health`
  - `/v1/public/share/*` (if/when sharing is enabled)
- Authenticated (RequireAuth):
  - `/v1/uploads/*`
  - `/v1/contents/*`
  - `/v1/groups/*`
  - `/v1/tags/*`
  - `/v1/share-links/*` (owner management)
- Internal-admin (MUST be secure by construction):
  - `/v1/internal/*` MUST be:
    - compiled only for internal builds (conditional compilation)
    - protected by Cloudflare Zero Trust
    - protected by our JWT auth (so we know which internal user performed an action)
    - require BOTH Zero Trust + JWT (JWT alone is not accepted; Zero Trust alone is not enough to identify the user in our system)

User checks:
- Endpoints are categorized correctly and enforced in routing.
- In non-internal builds, `/internal/*` routes do not exist (404 / not compiled).
- In internal builds, `/internal/*` rejects requests that have only JWT but no Zero Trust token.
- In internal builds, `/internal/*` rejects requests that have only Zero Trust token but no valid JWT.

---

## Multi-backend storage selection (Migration Plan — arbitrary OpenDAL backends)

We want to support uploads/access across **arbitrary OpenDAL-supported backends**, and allow choosing the backend at upload time (and later for access). That requires persisting *which backend profile* a `contents.storage_key` belongs to.

### Why a migration is needed

Right now `contents` includes:
- `storage_key` (path/key in a bucket)
- `content_type`, `file_size`, etc.

But `storage_key` is only meaningful when paired with a storage configuration (R2 vs GCS vs other OpenDAL services). If we want per-upload backend selection, we must store a backend identifier in SQL.

### Proposed schema changes (minimal and safe)

Add a backend identifier to `contents`:

- `storage_backend TEXT NOT NULL`
  - Examples: `r2`, `gcs`, `azblob`, `sftp`, `webdav`, etc.
  - **No SQL CHECK constraint** (we explicitly want arbitrary backends)

Recommended additional column to support *multiple configs of the same backend* (e.g. multiple buckets/accounts):
- `storage_profile TEXT NOT NULL DEFAULT 'default'`
  - Examples: `default`, `r2-main`, `gcs-archive`, `internal`, etc.

Keep `storage_key` as-is (object path/key within the selected backend profile).

Add indexes:
- `CREATE INDEX idx_contents_storage_backend ON contents(storage_backend);`
- `CREATE INDEX idx_contents_storage_profile ON contents(storage_profile);`

> Note: If you already have rows in `contents`, choose defaults:
> - `storage_backend = 'r2'` (or whatever your current default is)
> - `storage_profile = 'default'`

### Optional: `uploads` table (recommended)

If we implement `POST /v1/uploads/init` and `POST /v1/uploads/complete`, an `uploads` table makes the flow robust and auditable.

Suggested schema:
- `uploads` columns:
  - `id UUID PRIMARY KEY DEFAULT gen_random_uuid()`
  - `user_id UUID NOT NULL REFERENCES users(id)`
  - `storage_backend TEXT NOT NULL`
  - `storage_profile TEXT NOT NULL DEFAULT 'default'`
  - `storage_key TEXT NOT NULL`
  - `content_type VARCHAR(100) NOT NULL`
  - `file_size BIGINT NOT NULL`
  - `status VARCHAR(20) NOT NULL DEFAULT 'initiated'` (`initiated|completed|aborted|expired`)
  - `expires_at TIMESTAMPTZ NOT NULL`
  - `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`
  - `completed_at TIMESTAMPTZ`

Suggested indexes:
- `CREATE INDEX idx_uploads_user_created_at ON uploads(user_id, created_at DESC);`
- `CREATE INDEX idx_uploads_status_expires_at ON uploads(status, expires_at);`

This table allows you to:
- prevent completing someone else’s upload
- garbage-collect expired uploads
- mitigate replay attacks on `uploads/complete`
- persist backend choice made during `/uploads/init`

---

## Secrets & credentials plan (R2 + GCS via Secret Manager)

We need credentials for:
- Cloudflare R2 (S3-compatible): `account_id`, `access_key_id`, `secret_access_key`, `bucket`
- Google Cloud Storage (GCS): `bucket`, plus service account credentials (JSON)

Because we run on Google Cloud Run, best practice is:
- store secrets in **Google Secret Manager**
- mount them into Cloud Run as environment variables (or files), and load them via `Config`

### What should be stored as secrets (recommended)
**R2**
- `CF_ACCOUNT_ID`
- `CF_ACCESS_KEY_ID`
- `CF_SECRET_ACCESS_KEY`
- `CF_BUCKET` (optional to keep as config instead of secret; safe either way)

**GCS**
- Option A (recommended on GCP): use the Cloud Run service account via IAM; avoid raw JSON.
  - store only: `GCS_BUCKET`
  - and grant IAM roles (e.g. `roles/storage.objectAdmin`) to the Cloud Run service account
- Option B (portable): store:
  - `GCS_BUCKET`
  - `GCS_CREDENTIALS` (service account JSON) in Secret Manager

> For arbitrary OpenDAL backends: add more secrets as needed per backend, but keep the same pattern:
> - each backend/profile maps to a set of secret values.
> - nothing long-lived is returned to the client; only signed URLs.

### Storage profile mapping (required for arbitrary backends)
Because we support arbitrary `storage_backend` + `storage_profile`, the service needs an internal config mapping:
- `(storage_backend, storage_profile)` → credentials + bucket/endpoint

Example (conceptual):
- `(r2, default)` → CF_* secrets
- `(gcs, default)` → GCS_* secrets or IAM
- `(gcs, archive)` → different bucket and/or different service account

User checks:
- You can run one deployment with multiple configured profiles.
- Uploading with `storage_backend=r2` and `storage_backend=gcs` routes correctly.

### Scripts to add (TODO)
We need scripts to create/update secrets and bind them to Cloud Run deployments.

Add under `scripts/services/` (Bun + TypeScript), and expose via `scripts/main.ts` + `scripts/mod.just`:
- `services::gcloud-secret-ensure`
  - ensures a secret exists (idempotent)
  - supports setting/updating the latest version from a value (stdin/env)
- `services::gcloud-secret-bind`
  - updates Cloud Run service to bind secrets to env vars
  - supports per-env (prod/internal/test/pr)
- `services::gcloud-iam-bind`
  - when using GCS IAM (Option A), grants required roles to the Cloud Run service account

User checks:
- Secrets exist in Secret Manager for the target env/project.
- Cloud Run service has env vars (or mounted secret files) wired correctly.
- Service starts without requiring local plaintext credentials.

---

## Step-by-step workflow (each step user checks)

### Step 0 — Decide GCS auth mode (IAM vs JSON)
Action:
- Choose one:
  - Option A: Cloud Run IAM (recommended on GCP)
  - Option B: service account JSON (portable)

User checks:
- You can explain where credentials live and how rotation works.
- No long-lived credentials are committed to git.

#### Step 1 — Create the migration
Action:
- `just services::add-migrate add-storage-routing-to-contents`

User checks:
- A new file exists under `services/migrations/` with the timestamp prefix and the name you chose.
- The migration contains the intended SQL for both:
  - adding columns to `contents`
  - (optionally) creating `uploads`

#### Step 2 — Write migration SQL
Action (choose one):
- Minimal: alter `contents` only
- Recommended: alter `contents` + create `uploads`

User checks:
- Migration SQL is backwards-compatible:
  - existing `contents` rows get valid defaults
- There is no CHECK constraint restricting backend names (we want arbitrary).

#### Step 3 — Apply locally
Action:
- `just services::migrate local`

User checks:
- Migration applies cleanly (no errors)
- `contents` now has `storage_backend` (and `storage_profile` if added)
- If added: `uploads` table exists

#### Step 4 — Update SQLx offline cache
Action:
- `just services::prepare local`

User checks:
- `.sqlx/` directory changes are generated
- You commit `.sqlx/` changes together with the migration

#### Step 5 — Provision secrets and bind to runtime
Action:
- Add scripts (see “Scripts to add”) and run them for the target env.
  - Create/update secrets
  - Bind secrets to Cloud Run env vars (or file mounts)
  - (If using IAM) bind IAM roles to Cloud Run service account

User checks:
- Cloud Run revision has the env vars set (or secret mounts present).
- `GET /is-health` returns OK and service logs show storage backends can initialize.

#### Step 6 — Update API + storage routing code
Action:
- `/v1/uploads/init` accepts `storage_backend` + optional `storage_profile`
- `/v1/uploads/complete` writes `(storage_backend, storage_profile, storage_key)` into `contents`
- `/v1/contents/:id/view-url` reads `(storage_backend, storage_profile, storage_key)` and routes to the correct storage + signing implementation

User checks:
- You can upload the same `storage_key` to two different backends without collisions because backend/profile disambiguates it.
- Access control is still enforced at the API level (only authorized users can request view/download URLs).

#### Step 7 — Verification tests (MVP)
Action:
- Add integration tests for:
  - upload init → complete → list contents
  - view-url generation for at least 2 backends (use mocks if needed)

User checks:
- Tests pass in both:
  - non-internal (`cargo test`)
  - all-features (`cargo test --all-features`) if applicable to services CI.

### API implications

- `POST /v1/uploads/init` should accept:
  - `storage_backend: string`
  - `storage_profile?: string` (default `default`)
- `POST /v1/uploads/complete` persists:
  - `storage_backend`, `storage_profile`, `storage_key`
- `POST /v1/contents/:id/view-url` uses:
  - `(storage_backend, storage_profile, storage_key)` to choose the correct backend and generate a signed URL

---

## Current State (What We Have)

### Runtime / stack
- Rust + `axum`
- SQL migrations exist (`migrations/20251226105821_init-auth-storage.sql`)
- Storage integration via `opendal`:
  - Cloudflare R2 (S3 API) connectivity + read/write/list/delete via `CFFileStorage`
  - GCS connectivity checker (`GDDisk`) exists; full file storage implementation may be incomplete
- Auth & internal routing shell exists:
  - `GET /is-health`
  - `/auth` routes exist (implemented under `users/*`)
  - `/internal` routes exist, with optional Cloudflare Zero Trust protection (see `ZERO_TRUST.md`)
    - TODO(Security): internal routes must not be “optional protection”; they must be gated and enforced (see “Internal-admin (MUST be secure by construction)” above)

### Database schema (already migrated)
- `users`, `sessions`, `otp_attempts`
- `contents` (file metadata + status + visibility)
- `content_groups`, `content_group_items` (collections)
- `share_links`, `content_shares`, `content_group_shares`, `share_link_accesses`
- `tags`, `content_tags`
- `audit_logs`

### Storage (already implemented at trait level)
- `storage::FileStorage` trait with:
  - `upload_file`, `download_file`, `delete_file`, `list_files`, `file_exists`, `get_file_metadata`
- Cloudflare R2:
  - `CFFileStorage` supports upload/download/delete/list/exists/stat (backed by OpenDAL S3 operator)
- Mock storage exists for tests.

### Notes / gaps in current codebase
- Internal API security gaps (must fix before relying on internal endpoints):
  - Conditional compilation is missing: internal routes are currently compiled/mounted in all builds (requirement: internal-only builds).
  - Protection is incomplete: Zero Trust middleware is currently optional depending on env vars. This can accidentally leave `/internal/*` unprotected.
  - JWT is not enforced on internal endpoints, but we require internal APIs to use JWT to identify the acting user.
  - Policy requirement: internal APIs must require BOTH Zero Trust token + our JWT (reject JWT-only and reject Zero-Trust-only).
- There is no “contents API” implementation yet (`collects` module is placeholder).
- There is no “presign/signed-url” flow implemented.
- If we want “direct upload” to storage without hitting the service, we must implement **presigned URL** support (OpenDAL alone usually won’t produce S3/GCS presigned URLs in a uniform way; see TODO below).

---

## Primary Goals (Short Term)
- Support user storing multi-media files (images first for native viewing)
- Allow listing, uploading, and managing items
- Provide secure access to private assets without proxying bytes through services

---

## API Principles (Best Practice)
- **Do not proxy file bytes through the API** for normal uploads/downloads.
  - Use **direct-to-storage** uploads via **presigned URLs** (or provider-specific signed upload mechanisms).
  - Use **short-lived view/download URLs** for asset access.
- Keep the service responsible for:
  - authorization checks
  - metadata in Postgres
  - generating and returning signed URLs
  - audit logging

---

## API v1 — Endpoints to Implement

### 1) Auth & Session
(Existing route group: `/auth` — confirm exact endpoints in implementation.)

**Required**
- `POST /v1/auth/otp/start`
  - rate-limit by `otp_attempts`
- `POST /v1/auth/otp/verify`
  - create session (`sessions`)
- `POST /v1/auth/logout`
- `GET /v1/me`

**Internal (manager-only)**
- `POST /v1/internal/users` create user (OTP-only account)
- `GET /v1/internal/users` list users
- `PATCH /v1/internal/users/:id` set status `active|suspended|archived`

---

### 2) Upload Flow (Direct-to-Storage)
This is the most important set to enable multi-media support without expensive service bandwidth.

**2.1 Create an upload session**
- `POST /v1/uploads/init`
  - Request:
    - `filename`
    - `content_type` (MIME)
    - `file_size`
    - optional: `sha256` (future dedupe)
    - optional: `title`, `description`
  - Response:
    - `upload_id` (UUID)
    - `storage_key` (object key/path)
    - `method`: `put` | `multipart`
    - `upload_url` (for single PUT) OR `parts` (for multipart)
    - `required_headers` (if any)
    - `expires_at`

**2.2 Complete upload**
- `POST /v1/uploads/complete`
  - Request:
    - `upload_id`
    - `storage_key`
    - `content_type`
    - `file_size`
    - optional: `etag` / `parts` (multipart completion)
    - `title`, `description`, `visibility`
  - Behavior:
    - validate object exists (HEAD/stat via storage operator)
    - create `contents` row
    - write `audit_logs` entry
  - Response:
    - created content object

**2.3 Abort upload (optional)**
- `POST /v1/uploads/abort`
  - For multipart sessions or failed flows

**TODO: Upload session persistence**
- Add a table (recommended) e.g. `uploads`:
  - `id`, `user_id`, `storage_key`, `content_type`, `file_size`, `status`, `expires_at`, `created_at`
- Without this table, you can still do stateless presign, but completion/audit becomes harder.

---

### 3) Contents (Collect Items)
These map to the `contents` table.

**3.1 List contents (grid)**
- `GET /v1/contents`
  - Query:
    - `limit`, `cursor` (prefer cursor pagination)
    - `status=active|archived|trashed`
    - `visibility=private|public|restricted`
    - `type_prefix=image/` OR `content_type=...`
    - `q=` search title/description
    - `tag=` (later)
  - Response:
    - array of content summaries for grid
    - next cursor

**3.2 Get content detail**
- `GET /v1/contents/:id`
  - Response includes metadata and optionally the best “viewer info”:
    - if image: recommend inline view URL endpoint below

**3.3 Update metadata**
- `PATCH /v1/contents/:id`
  - `title`, `description`, `visibility`

**3.4 Lifecycle**
- `POST /v1/contents/:id/trash`
- `POST /v1/contents/:id/restore`
- `POST /v1/contents/:id/archive`
- `POST /v1/contents/:id/unarchive`
- `DELETE /v1/contents/:id` (optional hard delete + delete storage object)

---

### 4) Asset Access (Private-by-default, no service proxy)
We want: access objects stored in R2/GCS **without** routing bytes through services, but still enforcing privileges.

**4.1 Generate a short-lived view URL**
- `POST /v1/contents/:id/view-url`
  - Request:
    - `disposition`: `inline|attachment`
  - Response:
    - `{ url, expires_at }`
  - Server side:
    - verify session
    - verify ownership or share permission
    - generate signed URL to object storage

**4.2 (Optional) Thumbnail URL**
- `POST /v1/contents/:id/thumbnail-url?size=256`
  - If thumbnails aren’t implemented yet:
    - return original view-url or a “not available” marker

**Note**
- For R2, generate **S3 Signature V4 presigned GET**.
- For GCS, generate **V4 signed URL**.
- OpenDAL is great for *IO*, but **presigning is provider-specific**; we will likely need:
  - direct SDKs for signing (AWS SigV4 for S3/R2, Google signing for GCS), or
  - implement signing ourselves with well-tested crates.

---

### 5) Collections (content_groups)
These map to `content_groups` and `content_group_items`.

- `GET /v1/groups`
- `POST /v1/groups`
- `GET /v1/groups/:id`
- `PATCH /v1/groups/:id`
- `POST /v1/groups/:id/trash|restore|archive|unarchive`
- `GET /v1/groups/:id/contents`
- `POST /v1/groups/:id/contents` (add items)
- `DELETE /v1/groups/:id/contents/:content_id`
- `PATCH /v1/groups/:id/contents/reorder` (update sort_order)

---

### 6) Tags
Maps to `tags` and `content_tags`.

- `GET /v1/tags`
- `POST /v1/tags`
- `PATCH /v1/tags/:id`
- `DELETE /v1/tags/:id`
- `POST /v1/contents/:id/tags` (attach)
- `DELETE /v1/contents/:id/tags/:tag_id` (detach)

---

### 7) Sharing
Maps to `share_links`, `content_shares`, `content_group_shares`, `share_link_accesses`.

**7.1 Share links**
- `POST /v1/share-links`
  - fields: `name`, `permission=view|download`, `password`, `expires_at`, `max_access_count`
- `GET /v1/share-links`
- `PATCH /v1/share-links/:id` (disable, rotate token, update expiry)
- `DELETE /v1/share-links/:id`

**7.2 Attach shares**
- `POST /v1/contents/:id/share-link` (create or attach a link)
- `POST /v1/groups/:id/share-link`

**7.3 Public read endpoints (no auth)**
- `GET /v1/public/share/:token`
  - returns metadata, verifies link rules (expiry, password, max_count)
  - logs `share_link_accesses`
- `POST /v1/public/share/:token/view-url`
  - returns signed URL for storage object(s) allowed by the share

---

### 8) Capabilities (helps UI evolve)
- `GET /v1/capabilities`
  - `supported_viewers: ["image"]` now
  - `max_upload_bytes`
  - `upload_methods: ["single_put", "multipart"]`
  - maybe `storage_backends_enabled: ["r2"] | ["gcs"] | ["r2","gcs"]`

---

## Implementation TODO Checklist (Engineering Work)

### A) SQL / Storage integration
- [x] Implement `contents` queries with `sqlx`:
  - insert, list, get, update, lifecycle updates
- [x] Implement `content_groups` queries and join table operations
- [x] Implement `tags` queries + content_tags attach/detach
- [x] Implement `share_links` and share join tables
- [x] Write audit log helper; log key actions

### B) Direct upload / signed URL generation
- [ ] Decide upload strategy:
  - [ ] Single PUT URL (good MVP)
  - [ ] Multipart upload (later for large videos)
- [ ] Implement R2 signing:
  - [ ] Presigned PUT for upload
  - [ ] Presigned GET for viewing/downloading
- [ ] Implement GCS signing:
  - [ ] V4 signed PUT/GET URLs
- [ ] Define `uploads` persistence table (recommended) OR ensure stateless flow is safe
- [ ] Validate object exists on completion (stat/HEAD)

### C) Authorization rules (must-have)
- [ ] “Owner can access” rule for all private assets
- [ ] Shares can grant view/download
- [ ] Visibility enforcement:
  - `private`: owner only (unless shared)
  - `restricted`: shared users/links only
  - `public`: accessible without auth (optional; still can use signed URLs)

### D) Observability / safety
- [ ] Add rate limiting for OTP endpoints (use `otp_attempts`)
- [ ] Add upload limits (max file size, allowed MIME types)
- [ ] Add structured logging for:
  - upload init/complete
  - view-url generation
- [x] Add tests:
  - unit tests for SQL queries (or integration tests with test DB)
  - integration tests for API flows using existing test harness approach

---

## Notes on “OpenDAL + direct upload”
OpenDAL is primarily an abstraction for *performing operations* (read/write/stat/list).  
Direct browser/app uploads to R2/GCS typically require **presigned URLs**, which are provider-specific.

Pragmatic approach:
- Keep OpenDAL for server-side HEAD/stat/delete/list.
- Implement signing separately:
  - R2: S3 SigV4 presign
  - GCS: Signed URL (V4)
- The API should never expose long-lived credentials.

---

## MVP Scope Recommendation (Image-view first)
Implement only:
- Auth session (`/v1/me`, OTP verify)
- Upload init + complete
- Contents list + get + update + trash/restore
- View-url endpoint (signed GET)

Then layer:
- groups, tags, share links, thumbnails, multipart uploads, PDF/CBZ viewers.

---