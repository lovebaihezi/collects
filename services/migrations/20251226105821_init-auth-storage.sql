-- ============================================================================
-- CORE TABLES
-- ============================================================================

-- ============================================================================
-- TABLE: users
-- ============================================================================
--
-- FEATURE REQUIREMENTS:
-- 1. Users are created ONLY by internal managers (no self-registration)
-- 2. Authentication is OTP-only (TOTP via authenticator app)
-- 3. Users can update their own nickname and avatar
-- 4. Users CANNOT delete their own account
-- 5. Users can be suspended or archived by managers
-- 6. Username must be lowercase alphanumeric with underscores only
--
-- STATUS VALUES:
-- - 'active': Normal operational state, user can login
-- - 'suspended': Temporarily disabled, user cannot login
-- - 'archived': Permanently disabled, user cannot login, kept for audit
--
-- SQLX USAGE (Rust):
--
-- // Create user (internal manager creates user)
-- sqlx::query!(
--     r#"
--     INSERT INTO users (username, otp_secret, created_by)
--     VALUES ($1, $2, $3)
--     RETURNING id, username, created_at
--     "#,
--     username,
--     otp_secret,
--     manager_user_id
-- )
-- .fetch_one(&pool)
-- .await?;
--
-- // Get user by username (for login)
-- sqlx::query_as!(
--     User,
--     r#"
--     SELECT id, username, otp_secret, nickname, avatar_url, status,
--            created_by, created_at, updated_at
--     FROM users
--     WHERE username = $1 AND status = 'active'
--     "#,
--     username
-- )
-- .fetch_optional(&pool)
-- .await?;
--
-- // Get user by ID
-- sqlx::query_as!(
--     User,
--     r#"
--     SELECT id, username, otp_secret, nickname, avatar_url, status,
--            created_by, created_at, updated_at
--     FROM users
--     WHERE id = $1
--     "#,
--     user_id
-- )
-- .fetch_optional(&pool)
-- .await?;
--
-- // Update user profile (nickname and avatar)
-- sqlx::query!(
--     r#"
--     UPDATE users
--     SET nickname = $2, avatar_url = $3
--     WHERE id = $1
--     RETURNING id, nickname, avatar_url, updated_at
--     "#,
--     user_id,
--     nickname,
--     avatar_url
-- )
-- .fetch_one(&pool)
-- .await?;
--
-- // Update user status (manager action)
-- sqlx::query!(
--     r#"
--     UPDATE users
--     SET status = $2
--     WHERE id = $1
--     RETURNING id, status, updated_at
--     "#,
--     user_id,
--     new_status  // 'active', 'suspended', or 'archived'
-- )
-- .fetch_one(&pool)
-- .await?;
--
-- // List all users (for manager dashboard)
-- sqlx::query_as!(
--     User,
--     r#"
--     SELECT id, username, otp_secret, nickname, avatar_url, status,
--            created_by, created_at, updated_at
--     FROM users
--     ORDER BY created_at DESC
--     LIMIT $1 OFFSET $2
--     "#,
--     limit,
--     offset
-- )
-- .fetch_all(&pool)
-- .await?;
--
-- // List users created by a specific manager
-- sqlx::query_as!(
--     User,
--     r#"
--     SELECT id, username, otp_secret, nickname, avatar_url, status,
--            created_by, created_at, updated_at
--     FROM users
--     WHERE created_by = $1
--     ORDER BY created_at DESC
--     "#,
--     manager_user_id
-- )
-- .fetch_all(&pool)
-- .await?;
--
-- // Check if username exists
-- sqlx::query_scalar!(
--     r#"
--     SELECT EXISTS(SELECT 1 FROM users WHERE username = $1) as "exists!"
--     "#,
--     username
-- )
-- .fetch_one(&pool)
-- .await?;
--
-- RUST STRUCT:
--
-- #[derive(Debug, Clone, sqlx::FromRow)]
-- pub struct User {
--     pub id: Uuid,
--     pub username: String,
--     pub otp_secret: String,
--     pub nickname: Option<String>,
--     pub avatar_url: Option<String>,
--     pub status: String,
--     pub created_by: Option<Uuid>,
--     pub created_at: chrono::DateTime<chrono::Utc>,
--     pub updated_at: chrono::DateTime<chrono::Utc>,
-- }
--
-- // Public user info (without sensitive fields)
-- #[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
-- pub struct UserProfile {
--     pub id: Uuid,
--     pub username: String,
--     pub nickname: Option<String>,
--     pub avatar_url: Option<String>,
-- }
--
-- ============================================================================
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL UNIQUE,
    otp_secret TEXT NOT NULL,  -- Base32 encoded, consider encryption at rest
    nickname VARCHAR(100),
    avatar_url TEXT,
    status VARCHAR(20) NOT NULL DEFAULT 'active',  -- 'active', 'suspended', 'archived'
    created_by UUID REFERENCES users(id),  -- Manager who created this user
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT users_status_check CHECK (status IN ('active', 'suspended', 'archived')),
    CONSTRAINT users_username_format CHECK (username ~ '^[a-z0-9_]+$')
);

-- Sessions for maintaining login state after OTP verification
CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,  -- SHA256 hash of session token
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_accessed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ip_address INET,
    user_agent TEXT
);

-- Rate limiting for OTP attempts
CREATE TABLE otp_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username VARCHAR(50) NOT NULL,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    success BOOLEAN NOT NULL DEFAULT false,
    ip_address INET
);

-- ============================================================================
-- CONTENT TABLES
-- ============================================================================

-- Content uploaded by users
CREATE TABLE contents (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    title VARCHAR(255) NOT NULL,
    description TEXT,
    storage_key TEXT NOT NULL,  -- Path/key in R2 or GCS
    content_type VARCHAR(100) NOT NULL,
    file_size BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',  -- 'active', 'archived', 'trashed'
    visibility VARCHAR(20) NOT NULL DEFAULT 'private',  -- 'private', 'public', 'restricted'
    trashed_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT contents_status_check CHECK (status IN ('active', 'archived', 'trashed')),
    CONSTRAINT contents_visibility_check CHECK (visibility IN ('private', 'public', 'restricted'))
);

-- Content groups (collections for sharing multiple items together)
CREATE TABLE content_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name VARCHAR(255) NOT NULL,
    description TEXT,
    visibility VARCHAR(20) NOT NULL DEFAULT 'private',
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    trashed_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT content_groups_status_check CHECK (status IN ('active', 'archived', 'trashed')),
    CONSTRAINT content_groups_visibility_check CHECK (visibility IN ('private', 'public', 'restricted'))
);

-- Junction table: contents belonging to groups
CREATE TABLE content_group_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES content_groups(id) ON DELETE CASCADE,
    content_id UUID NOT NULL REFERENCES contents(id) ON DELETE CASCADE,
    sort_order INT NOT NULL DEFAULT 0,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT content_group_items_unique UNIQUE (group_id, content_id)
);

-- ============================================================================
-- SHARING TABLES
-- ============================================================================

-- Share links for public/restricted access
CREATE TABLE share_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    owner_id UUID NOT NULL REFERENCES users(id),
    token VARCHAR(64) NOT NULL UNIQUE,  -- Unique share token for URLs
    name VARCHAR(255),  -- Optional friendly name for the link
    permission VARCHAR(20) NOT NULL DEFAULT 'view',  -- 'view', 'download'
    password_hash TEXT,  -- Optional password protection
    max_access_count INT,  -- Optional limit on number of accesses
    access_count INT NOT NULL DEFAULT 0,
    expires_at TIMESTAMPTZ,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT share_links_permission_check CHECK (permission IN ('view', 'download'))
);

-- Individual content shares (to specific users or via link)
CREATE TABLE content_shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    content_id UUID NOT NULL REFERENCES contents(id) ON DELETE CASCADE,
    shared_with_user_id UUID REFERENCES users(id) ON DELETE CASCADE,  -- NULL if using share_link
    share_link_id UUID REFERENCES share_links(id) ON DELETE CASCADE,  -- NULL if direct user share
    permission VARCHAR(20) NOT NULL DEFAULT 'view',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by UUID NOT NULL REFERENCES users(id),

    CONSTRAINT content_shares_permission_check CHECK (permission IN ('view', 'download')),
    CONSTRAINT content_shares_target_check CHECK (
        (shared_with_user_id IS NOT NULL AND share_link_id IS NULL) OR
        (shared_with_user_id IS NULL AND share_link_id IS NOT NULL)
    )
);

-- Group shares (share entire collection)
CREATE TABLE content_group_shares (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES content_groups(id) ON DELETE CASCADE,
    shared_with_user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    share_link_id UUID REFERENCES share_links(id) ON DELETE CASCADE,
    permission VARCHAR(20) NOT NULL DEFAULT 'view',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by UUID NOT NULL REFERENCES users(id),

    CONSTRAINT group_shares_permission_check CHECK (permission IN ('view', 'download')),
    CONSTRAINT group_shares_target_check CHECK (
        (shared_with_user_id IS NOT NULL AND share_link_id IS NULL) OR
        (shared_with_user_id IS NULL AND share_link_id IS NOT NULL)
    )
);

-- Track share link accesses
CREATE TABLE share_link_accesses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    share_link_id UUID NOT NULL REFERENCES share_links(id) ON DELETE CASCADE,
    accessed_by_user_id UUID REFERENCES users(id),  -- NULL if anonymous
    ip_address INET,
    user_agent TEXT,
    accessed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- TAGS (for organization)
-- ============================================================================

CREATE TABLE tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),
    name VARCHAR(100) NOT NULL,
    color VARCHAR(7),  -- Hex color code
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT tags_unique_per_user UNIQUE (user_id, name)
);

CREATE TABLE content_tags (
    content_id UUID NOT NULL REFERENCES contents(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (content_id, tag_id)
);

-- ============================================================================
-- AUDIT LOG
-- ============================================================================

CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id),  -- NULL for system events
    action VARCHAR(100) NOT NULL,
    entity_type VARCHAR(50) NOT NULL,
    entity_id UUID,
    details JSONB,
    ip_address INET,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============================================================================
-- INDEXES
-- ============================================================================

-- Users
CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_status ON users(status) WHERE status = 'active';
CREATE INDEX idx_users_created_by ON users(created_by);

-- Sessions
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
CREATE INDEX idx_sessions_token_hash ON sessions(token_hash);
CREATE INDEX idx_sessions_expires_at ON sessions(expires_at);

-- OTP Attempts (for rate limiting queries)
CREATE INDEX idx_otp_attempts_username_time ON otp_attempts(username, attempted_at DESC);
CREATE INDEX idx_otp_attempts_ip_time ON otp_attempts(ip_address, attempted_at DESC);

-- Contents
CREATE INDEX idx_contents_user_id ON contents(user_id);
CREATE INDEX idx_contents_status ON contents(status);
CREATE INDEX idx_contents_visibility ON contents(visibility);
CREATE INDEX idx_contents_trashed_at ON contents(trashed_at) WHERE trashed_at IS NOT NULL;

-- Content Groups
CREATE INDEX idx_content_groups_user_id ON content_groups(user_id);
CREATE INDEX idx_content_groups_status ON content_groups(status);

-- Content Group Items
CREATE INDEX idx_content_group_items_group_id ON content_group_items(group_id);
CREATE INDEX idx_content_group_items_content_id ON content_group_items(content_id);

-- Share Links
CREATE INDEX idx_share_links_token ON share_links(token);
CREATE INDEX idx_share_links_owner_id ON share_links(owner_id);

-- Content Shares
CREATE INDEX idx_content_shares_content_id ON content_shares(content_id);
CREATE INDEX idx_content_shares_shared_with ON content_shares(shared_with_user_id);
CREATE INDEX idx_content_shares_link_id ON content_shares(share_link_id);

-- Group Shares
CREATE INDEX idx_group_shares_group_id ON content_group_shares(group_id);
CREATE INDEX idx_group_shares_shared_with ON content_group_shares(shared_with_user_id);

-- Tags
CREATE INDEX idx_tags_user_id ON tags(user_id);
CREATE INDEX idx_content_tags_content_id ON content_tags(content_id);
CREATE INDEX idx_content_tags_tag_id ON content_tags(tag_id);

-- Audit Logs
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_entity ON audit_logs(entity_type, entity_id);
CREATE INDEX idx_audit_logs_created_at ON audit_logs(created_at DESC);

-- ============================================================================
-- FUNCTIONS
-- ============================================================================

-- Auto-update updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Triggers for updated_at
CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_contents_updated_at
    BEFORE UPDATE ON contents
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

CREATE TRIGGER update_content_groups_updated_at
    BEFORE UPDATE ON content_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();
 to trash
