# Collects App Service

## Database

Auto managed by neon, using Sqlx for migrations and queries.

## Env

Run on Google Cloud Run, using Rust cause low memory footprint and fast cold start, which means there will be no migrations to prevent cold start latency.

## Environment Variables

### Required
- `ENV` - Environment mode: `local` or `prod`
- `DATABASE_URL` - PostgreSQL connection string
- `PORT` - Server port (default varies by environment)

### Optional
- `SERVER_ADDR` - Server bind address (defaults: `127.0.0.1` for local, `0.0.0.0` for prod)

### Storage (Optional)
- `CF_ACCOUNT_ID` - Cloudflare R2 account ID
- `CF_ACCESS_KEY_ID` - Cloudflare R2 access key
- `CF_SECRET_ACCESS_KEY` - Cloudflare R2 secret key
- `CF_BUCKET` - Cloudflare R2 bucket name
- `GCS_BUCKET` - Google Cloud Storage bucket name
- `GCS_CREDENTIALS` - Google Cloud Storage credentials JSON

### Cloudflare Zero Trust Authentication (Optional)
- `CF_ACCESS_TEAM_DOMAIN` - Your Cloudflare Access team domain (e.g., `myteam.cloudflareaccess.com`)
- `CF_ACCESS_AUD` - Application Audience (AUD) tag from Cloudflare Access

When both `CF_ACCESS_TEAM_DOMAIN` and `CF_ACCESS_AUD` are set, internal routes (`/internal/*`) will be protected with JWT token authentication. See [ZERO_TRUST.md](./ZERO_TRUST.md) for details.

## Observability

1. Write log to /var/log/collects-service.log using tracing subscriber
2.
