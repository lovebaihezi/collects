-- Add text content support: body column for inline text storage, kind column to distinguish content types
-- Text content < 64KB is stored inline in the body column instead of R2

-- Add kind column to distinguish between file uploads and inline text
-- Values: 'file' (default, uploaded to R2) or 'text' (stored inline in body)
ALTER TABLE contents ADD COLUMN kind VARCHAR(20) NOT NULL DEFAULT 'file';

-- Add body column for inline text storage (only used when kind='text')
ALTER TABLE contents ADD COLUMN body TEXT;

-- Add check constraint to validate kind values
ALTER TABLE contents ADD CONSTRAINT contents_kind_check CHECK (kind IN ('file', 'text'));

-- Add check constraint: body must be set when kind='text', and must be null when kind='file'
ALTER TABLE contents ADD CONSTRAINT contents_body_kind_check CHECK (
    (kind = 'text' AND body IS NOT NULL) OR
    (kind = 'file' AND body IS NULL)
);

-- Index for filtering by kind (useful for listing only text notes or only files)
CREATE INDEX idx_contents_kind ON contents(kind);

-- Add comment explaining the columns
COMMENT ON COLUMN contents.kind IS 'Content type: file (uploaded to R2) or text (stored inline in body)';
COMMENT ON COLUMN contents.body IS 'Inline text content (only used when kind=text, max recommended 64KB)';
