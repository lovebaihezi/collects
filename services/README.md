# Collects App Service

## Database

Auto managed by neon, using Sqlx for migrations and queries.

## Env

Run on Google Cloud Run, using Rust cause low memory footprint and fast cold start, which means there will be no migrations to prevent cold start latency.

## Environment Variables

## Observability

1. Write log to /var/log/collects-service.log using tracing subscriber
2.
