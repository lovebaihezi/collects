# SQLx Migrations

This directory contains database migrations managed by SQLx.

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

| Environment | Database Secret        | Branch      |
|-------------|------------------------|-------------|
| prod        | database-url           | production  |
| nightly     | database-url           | production  |
| internal    | database-url-internal  | production  |
| test        | database-url-test      | development |
| pr          | database-url-pr        | development |
| local       | database-url-local     | development |