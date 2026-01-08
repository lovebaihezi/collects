-- ============================================================================
-- Migration: Add UUIDv7 support
-- Purpose:
-- - Enable pgcrypto extension for gen_random_bytes()
-- - Create uuid_v7() function for time-ordered UUIDs
-- - Alter existing tables to use uuid_v7() as default for new rows
-- ============================================================================

-- ============================================================================
-- Extensions
-- ============================================================================
-- Required for gen_random_bytes() used by uuid_v7()
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ============================================================================
-- UUIDv7 helper
-- ============================================================================
-- We generate UUIDv7 in Postgres so IDs are roughly time-ordered and still UUID-typed.
-- NOTE: This is a best-effort UUIDv7 implementation using built-in primitives.
CREATE OR REPLACE FUNCTION uuid_v7()
RETURNS UUID
LANGUAGE plpgsql
AS $$
DECLARE
    unix_ms BIGINT;
    time_hex TEXT;
    rand_bytes BYTEA;
    rand_hex TEXT;
    uuid_hex TEXT;
BEGIN
    -- milliseconds since unix epoch
    unix_ms := (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT;

    -- 48-bit timestamp (12 hex chars)
    time_hex := lpad(to_hex(unix_ms), 12, '0');

    -- 10 random bytes (20 hex chars)
    rand_bytes := gen_random_bytes(10);
    rand_hex := encode(rand_bytes, 'hex');

    -- Compose UUIDv7:
    -- - 48 bits timestamp
    -- - 4 bits version (7)
    -- - 12 bits random
    -- - 2 bits variant (RFC 4122 => 10xx)
    -- - 62 bits random
    --
    -- Layout (hex, with hyphens):
    -- time(12) - ver+rand(4) - var+rand(4) - rand(4) - rand(12)
    uuid_hex :=
        substr(time_hex, 1, 8) || '-' ||
        substr(time_hex, 9, 4) || '-' ||
        '7' || substr(rand_hex, 1, 3) || '-' ||
        -- set variant to 10xx by forcing top two bits to 10 (i.e. 8..b)
        to_hex((('x' || substr(rand_hex, 4, 2))::bit(8)::int & 63) | 128)::text || substr(rand_hex, 6, 2) || '-' ||
        substr(rand_hex, 8, 12);

    RETURN uuid_hex::uuid;
END;
$$;

-- ============================================================================
-- Alter tables to use uuid_v7() as default for new rows
-- ============================================================================
-- Note: This does NOT change existing UUIDs, only affects new inserts.

-- users table
ALTER TABLE users ALTER COLUMN id SET DEFAULT uuid_v7();

-- sessions table
ALTER TABLE sessions ALTER COLUMN id SET DEFAULT uuid_v7();

-- otp_attempts table
ALTER TABLE otp_attempts ALTER COLUMN id SET DEFAULT uuid_v7();

-- contents table
ALTER TABLE contents ALTER COLUMN id SET DEFAULT uuid_v7();

-- uploads table
ALTER TABLE uploads ALTER COLUMN id SET DEFAULT uuid_v7();

-- ============================================================================
-- Notes:
-- - Existing rows retain their original UUIDs (gen_random_uuid() v4).
-- - New rows will get time-ordered UUIDv7s, improving index locality.
-- - The uuid_v7() function uses clock_timestamp() for sub-transaction precision.
-- ============================================================================
