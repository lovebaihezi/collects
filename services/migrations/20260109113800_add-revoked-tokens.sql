-- Add revoked_tokens table for JWT session logout support
--
-- When a user logs out, their JWT token hash is stored here to prevent reuse.
-- Tokens are stored as SHA256 hashes (not the actual token) for security.
-- The expires_at column matches the token's expiry, allowing cleanup of old entries.

CREATE TABLE revoked_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash VARCHAR(64) NOT NULL,  -- SHA256 hash of the JWT token (hex-encoded)
    username VARCHAR(50) NOT NULL,     -- For audit purposes
    expires_at TIMESTAMPTZ NOT NULL,   -- When the original token expires (for cleanup)
    revoked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Index for fast lookup during token validation
CREATE INDEX idx_revoked_tokens_token_hash ON revoked_tokens(token_hash);

-- Index for cleanup jobs (remove expired revocations)
CREATE INDEX idx_revoked_tokens_expires_at ON revoked_tokens(expires_at);

-- Add comment explaining the table
COMMENT ON TABLE revoked_tokens IS 'Stores revoked JWT tokens to support logout. Entries can be cleaned up after expires_at.';
