# Collects App Service

## Database

Auto managed by Neon, using sqlx for migrations and queries.

### Migrations

Migrations are stored in `./migrations/` and are run using `sqlx-cli`.

```bash
# Run migrations on a specific environment
just services::migrate internal  # Admin credentials (for production changes)
just services::migrate test      # Test environment
just services::migrate prod      # Production (use with caution)

# Check migration status
just services::migrate-info prod

# Create a new migration
just services::migrate-add <migration_name>

# Revert the last migration
just services::migrate-revert test

# Generate sqlx offline data for compile-time verification
just services::sqlx-prepare internal
```

For detailed documentation, see [Database Setup Guide](/docs/database-setup.md).

## Running the Service

```bash
# Development mode (uses production database)
just services::dev

# Development mode with specific environment
just services::dev-env test  # Uses test database
just services::dev-env pr    # Uses PR database

# Release build
just services::release-run
just services::release-run-env test
```

## Env

Run on Google Cloud Run, using Rust for low memory footprint and fast cold start, which means there will be no migrations during runtime to prevent cold start latency.

## Environment Variables

| Variable | Description | Required |
|---|---|---|
| `ENV` | Environment name (local, prod) | Yes |
| `DATABASE_URL` | PostgreSQL connection string | Yes |
| `PORT` | Server port | Yes |
| `SERVER_ADDR` | Server address (defaults based on ENV) | No |

## Observability

1. Write log to /var/log/collects-service.log using tracing subscriber
2.
