-- Add migration script here for postgres
-- Every text should uses UTF-8 encoding

CREATE TYPE privacy_kind AS ENUM('public', 'private', 'protected');

-- one collect row includes
-- id, create_time, update_time, tags,
-- field will used for migration
CREATE TABLE collects (
    id SERIAL PRIMARY KEY,
    author_id TEXT NOT NULL REFERENCES neon_auth.users_sync(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    privacy_level privacy_kind DEFAULT 'public',
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE,
    deleted_at TIMESTAMP WITH TIME ZONE
);

-- stored on R2
CREATE TABLE collect_files (
    id SERIAL PRIMARY KEY,
    author_id TEXT NOT NULL REFERENCES neon_auth.users_sync(id) ON DELETE CASCADE,
    collect_id INTEGER NOT NULL REFERENCES collects(id) ON DELETE CASCADE,
    file_url TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP WITH TIME ZONE
);

-- one tag row includes
-- id, name, create_time, delete_time
-- MAXIMUM UTF-8 STRING LENGTH, about 85 CJK characters
CREATE TABLE tags (
    id SERIAL PRIMARY KEY,
    author_id TEXT NOT NULL REFERENCES neon_auth.users_sync(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE collect_tags (
    id SERIAL PRIMARY KEY,
    collect_id INTEGER NOT NULL REFERENCES collects(id) ON DELETE CASCADE,
    tag_id INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    deleted_at TIMESTAMP WITH TIME ZONE
);
