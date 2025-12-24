# Database Setup and Migration Guide

This document covers the database setup, migration workflow, and best practices for working with Neon database branches in the Collects project.

## Table of Contents

1. [Overview](#overview)
2. [Environment Setup](#environment-setup)
3. [Database Migrations](#database-migrations)
4. [Neon Branch Management](#neon-branch-management)
5. [Best Practices](#best-practices)
6. [Troubleshooting](#troubleshooting)

---

## Overview

The Collects project uses:
- **Neon** as the PostgreSQL database provider with branch-based environments
- **sqlx** for database migrations and compile-time query verification
- **Google Cloud Secret Manager** for storing database credentials

### Environment Mapping

| Environment | GitHub Trigger | Cloud Run Service | Secret Name | Neon Branch |
|---|---|---|---|---|
| **Internal** | Manual (admin) | N/A | `database-url-internal` | `main` (admin access) |
| **PR** | Pull Request | `collects-services-pr` | `database-url-pr` | `test` |
| **Test** | Push to `main` | `collects-services-test` | `database-url-test` | `test` |
| **Nightly** | Schedule | `collects-services-nightly` | `database-url-nightly` | `nightly` |
| **Production** | Tag (`v*`) | `collects-services` | `database-url` | `main` |

---

## Environment Setup

### Prerequisites

1. **Google Cloud SDK** (`gcloud`) installed and authenticated
2. **sqlx-cli** installed: `cargo install sqlx-cli --features postgres`
3. **Bun** installed for running helper scripts
4. **Just** command runner installed

### Initial Database Setup

To initialize a new Neon database project with all required branches and secrets:

```bash
# Get a Neon API token from https://console.neon.tech/app/settings/api-keys
just scripts::init-db <your-neon-api-token>
```

This command will:
1. Create a new Neon project
2. Create the `collects` database
3. Create `admin` and `web_user` roles
4. Create `main` and `test` branches
5. Update Google Cloud Secrets with connection strings

---

## Database Migrations

### Running Migrations

Migrations should be run using the `internal` environment (admin credentials) for production changes:

```bash
# Run migrations on internal (admin) - recommended for production
just scripts::migrate internal

# Run migrations on test environment
just scripts::migrate test

# Run migrations on production (use with caution)
just scripts::migrate prod
```

Or using the services module directly:

```bash
# From the services directory
just services::migrate internal
just services::migrate test
```

### Checking Migration Status

```bash
# Check which migrations have been applied
just scripts::migrate-info prod
just scripts::migrate-info test
```

### Creating New Migrations

```bash
# Create a new migration file
just services::migrate-add add_new_table
```

This creates a new migration file in `services/migrations/` with a timestamp prefix.

### Reverting Migrations

```bash
# Revert the last migration (with confirmation)
just scripts::migrate-revert test
```

### Preparing sqlx Offline Data

For compile-time query verification without a live database:

```bash
# Generate sqlx offline data
just services::sqlx-prepare internal
```

This creates `.sqlx/` files that enable `cargo build` to work without a database connection.

---

## Neon Branch Management

### Understanding Neon Branches

Neon branches work like Git branches for your database:
- **main**: Production data, parent branch for all others
- **test**: Used for PR and test environments (copy of main)
- **nightly**: For nightly builds (can be refreshed from main)
- **feature branches**: Created on-demand for development

### Listing Branches

```bash
just scripts::services neon-branches --token <token> --project-id <project-id>
```

### Creating a New Branch

```bash
# Create a branch for a feature or PR
just scripts::services neon-create-branch feature-xyz \
  --token <token> \
  --project-id <project-id> \
  --parent <main-branch-id>
```

### Updating Environment Secrets

When you need to point an environment to a different branch:

```bash
# Update the PR environment to use a specific branch
just scripts::services neon-update-secret pr \
  --token <token> \
  --project-id <project-id> \
  --branch-id <branch-id>
```

### Viewing Database URLs

To verify which database an environment is pointing to:

```bash
just scripts::show-db-url prod
just scripts::show-db-url test
```

---

## Best Practices

### Migration Workflow

1. **Develop locally**: Create and test migrations against a local or test database
2. **PR testing**: Migrations run automatically in PR environments
3. **Staging**: Apply to test/nightly environment after merge
4. **Production**: Apply to production with the internal admin credentials

### Recommended Migration Order

```bash
# 1. First apply to test to verify
just scripts::migrate test

# 2. Check status
just scripts::migrate-info test

# 3. Apply to production using admin credentials
just scripts::migrate internal

# 4. Verify production
just scripts::migrate-info prod
```

### Branch Strategy for Different Use Cases

#### Feature Development
1. Create a new Neon branch from `main`
2. Run migrations on the new branch
3. Update `database-url-pr` secret to point to the feature branch
4. Test in PR environment
5. Delete feature branch after merge

#### Schema Changes
1. Create migration file: `just services::migrate-add <name>`
2. Write forward and (optionally) reverse migrations
3. Test on `test` branch
4. Apply to `main` branch via `internal` environment

#### Data Backfills
1. Use `internal` credentials for direct database access
2. Consider creating a separate migration for data changes
3. Test backfills on `test` branch first

### Security Considerations

- **Never commit database URLs** to version control
- Use `internal` credentials only for migrations, not for application runtime
- Rotate passwords periodically using Neon console
- Review Secret Manager access logs regularly

---

## Troubleshooting

### "Permission denied" when running migrations

Ensure you're using the correct environment. The `internal` environment has admin privileges:

```bash
just scripts::migrate internal
```

### "Migration already applied" error

Check the current migration status:

```bash
just scripts::migrate-info <env>
```

### Connection timeout

Neon endpoints may scale to zero after inactivity. The first connection may take a few seconds:

```bash
# Retry the command
just scripts::migrate <env>
```

### "Secret not found" error

Verify the secret exists in Google Cloud:

```bash
gcloud secrets list | grep database-url
```

If missing, run the initial setup:

```bash
just scripts::init-db <neon-token>
```

### sqlx compile-time verification failing

Regenerate the offline data:

```bash
just services::sqlx-prepare internal
```

---

## Quick Reference

| Task | Command |
|---|---|
| Run migrations | `just scripts::migrate <env>` |
| Check status | `just scripts::migrate-info <env>` |
| Revert migration | `just scripts::migrate-revert <env>` |
| Create migration | `just services::migrate-add <name>` |
| Prepare sqlx | `just services::sqlx-prepare internal` |
| Start service (local) | `just services::dev` |
| Start service (env) | `just services::dev-env test` |
| View DB URL | `just scripts::show-db-url <env>` |
| List branches | `just scripts::services neon-branches --token <t> --project-id <p>` |
