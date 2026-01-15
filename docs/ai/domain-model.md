# Domain Model: Collects Terminology

This document clarifies the core domain terminology used in the Collects application.

## Key Concept: "Collect" = "Group"

**A "Collect" is what users call a Group in the database/API.**

When users say "collect," they mean a **Group** — a collection of files and text notes.
They do NOT mean individual "contents" (single files/texts).

---

## Entity Definitions

### Group (User-facing: "Collect")

A **Group** is a collection/folder that contains multiple content items.

- Database table: `content_groups`
- API resource: `/v1/groups`
- User terminology: "Collect", "Collection"

| Field | Description |
|-------|-------------|
| `id` | UUID |
| `name` | Display name (e.g., "Vacation Photos 2024") |
| `description` | Optional description |
| `visibility` | `private`, `public`, or `restricted` |
| `status` | `active`, `archived`, or `trashed` |

### Content (Internal: individual file or text)

A **Content** is a single file upload or inline text note.

- Database table: `contents`
- API resource: `/v1/contents`
- Kinds: `file` (uploaded to R2) or `text` (stored inline, max 64KB)

| Field | Description |
|-------|-------------|
| `id` | UUID |
| `title` | Display title |
| `body` | Inline text (only when `kind="text"`) |
| `storage_key` | Path in R2 (only when `kind="file"`) |
| `content_type` | MIME type |
| `file_size` | Size in bytes |

### Group ↔ Content Relationship

Contents are linked to Groups via a junction table:

```
┌─────────────────┐         ┌─────────────────────┐         ┌─────────────────┐
│     Group       │         │  content_group_items │         │    Content      │
│   ("Collect")   │ 1─────* │   (junction table)   │ *─────1 │  (File/Text)    │
└─────────────────┘         └─────────────────────┘         └─────────────────┘
```

- One Group can have many Contents
- One Content can belong to multiple Groups
- Junction table tracks `sort_order` for ordering within a group

---

## API Mapping

| User Action | API Endpoint |
|-------------|--------------|
| List my collects | `GET /v1/groups` |
| Create a collect | `POST /v1/groups` |
| View a collect | `GET /v1/groups/{id}` |
| List files in a collect | `GET /v1/groups/{id}/contents` |
| Add file to a collect | `POST /v1/groups/{id}/contents` |
| Trash a collect | `POST /v1/groups/{id}/trash` |
| Archive a collect | `POST /v1/groups/{id}/archive` |

---

## CLI Implications

When building CLI commands:

- `collects list` should list **Groups**, not individual contents
- Show file count per collect (requires calling `GET /v1/groups/{id}/contents` for each)
- Users think in terms of "collects" containing files, not loose files

---

## Common Mistakes to Avoid

1. **Don't confuse "contents" with "collects"**
   - Contents = individual items (files/texts)
   - Collects = Groups (collections of items)

2. **Don't list contents when user asks for collects**
   - User says "show my collects" → call `/v1/groups`
   - User says "show files in this collect" → call `/v1/groups/{id}/contents`

3. **Remember the terminology mapping**
   - User: "Collect" = Code: "Group"
   - User: "Files in collect" = Code: "Contents in Group"