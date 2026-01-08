# SQLx Migrations

This directory contains database migrations managed by SQLx.

## ⚠️ Important: Migration Integrity

**Once a migration has been applied to any database, it must NEVER be modified or deleted.**

SQLx tracks migration history using checksums. If you modify an already-applied migration:
- The migration will fail on databases that already ran the original version
- Database state becomes inconsistent and hard to recover

This repository enforces migration integrity via:
- **Pre-commit hooks**: Blocks commits that modify locked migrations
- **CI checks**: Fails PRs that modify locked migrations

### Locking New Migrations

After creating and testing a new migration locally, lock it before merging:

```bash
just scripts::migration-lock
```

This adds the migration's checksum to `.checksums.json`. Commit this file along with your migration.

### Checking Migration Integrity

To manually verify no locked migrations have been modified:

```bash
just scripts::migration-check
```

## Creating a New Migration

```bash
just services::add-migrate <migration_name>
```

This will create a new migration file with a timestamp prefix.

## Running Migrations

Run migrations for a specific environment:

```bash
# For local development
just services::migrate local

# For test environment
just services::migrate test

# For production (be careful!)
just services::migrate prod
```

## Checking Migration Status

```bash
just services::migrate-info local
```

## SQLx Offline Cache

After adding or modifying queries, regenerate the offline cache:

```bash
just services::prepare local
```

To verify the cache is up to date:

```bash
just services::prepare-check local
```

**Important:** The `.sqlx` directory must be committed to the repository. The CI will fail if the offline cache is outdated.

## Environment Mapping

| Environment   | Database Secret              | Branch      |
|---------------|------------------------------|-------------|
| prod          | database-url                 | production  |
| nightly       | database-url                 | production  |
| internal      | database-url-internal        | production  |
| test          | database-url-test            | development |
| test-internal | database-url-test-internal   | development |
| pr            | database-url-pr              | development |
| local         | database-url-local           | development |

## Files in This Directory

- `*.sql` - Migration files (timestamped, never modify after applying)
- `.checksums.json` - Integrity checksums for locked migrations (auto-generated)
- `.gitkeep` - Placeholder to ensure directory exists
- `README.md` - This documentation