-- ============================================================================
-- Migration: Add storage routing to contents + create uploads table
-- Purpose:
-- - Support arbitrary OpenDAL backends selected at upload time
-- - Disambiguate storage_key by backend + profile
-- - Introduce a robust upload-init -> upload-complete workflow
-- ============================================================================

-- ============================================================================
-- 1) contents: add storage_backend + storage_profile
-- ============================================================================

ALTER TABLE contents
    ADD COLUMN storage_backend TEXT NOT NULL DEFAULT 'r2';

ALTER TABLE contents
    ADD COLUMN storage_profile TEXT NOT NULL DEFAULT 'default';

-- Indexes for filtering/debugging
CREATE INDEX IF NOT EXISTS idx_contents_storage_backend ON contents(storage_backend);
CREATE INDEX IF NOT EXISTS idx_contents_storage_profile ON contents(storage_profile);

-- ============================================================================
-- 2) uploads: track upload sessions for direct-to-storage flow
--    (init -> complete -> create row in contents)
-- ============================================================================

CREATE TABLE uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id),

    storage_backend TEXT NOT NULL,
    storage_profile TEXT NOT NULL DEFAULT 'default',
    storage_key TEXT NOT NULL,

    content_type VARCHAR(100) NOT NULL,
    file_size BIGINT NOT NULL,

    status VARCHAR(20) NOT NULL DEFAULT 'initiated',  -- initiated|completed|aborted|expired
    expires_at TIMESTAMPTZ NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,

    CONSTRAINT uploads_status_check CHECK (status IN ('initiated', 'completed', 'aborted', 'expired'))
);

CREATE INDEX idx_uploads_user_created_at ON uploads(user_id, created_at DESC);
CREATE INDEX idx_uploads_status_expires_at ON uploads(status, expires_at);

-- ============================================================================
-- Notes:
-- - We intentionally do NOT add a CHECK constraint for storage_backend values because
--   we want to support arbitrary OpenDAL backends.
-- - The default contents.storage_backend is set to 'r2' for backward compatibility with
--   existing rows. Change this default if your primary backend differs.
-- ============================================================================
