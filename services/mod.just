default: fmt

# =============================================================================
# Helper function for mapping environment to secret name
# This is sourced by all environment-aware commands
# =============================================================================
# Usage: SECRET_NAME=$(get_secret_name <env>)
# Environments: prod, test, pr, nightly, internal
_get_secret_name := '''
get_secret_name() {
    local env="$1"
    case "$env" in
        prod)     echo "database-url" ;;
        internal) echo "database-url-internal" ;;
        *)        echo "database-url-$env" ;;
    esac
}
'''

build:
    cargo b

# Run the service in local development mode (uses prod database by default)
dev:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r

# Run the service using a specific environment's database
# Usage: just services::dev-env test
# Environments: test, pr, nightly, prod (default), internal
dev-env env='prod':
    #!/bin/bash
    set -eu
    {{_get_secret_name}}
    SECRET_NAME=$(get_secret_name "{{env}}")
    DATABASE_URL=$(gcloud secrets versions access latest --secret="${SECRET_NAME}")
    ENV=local DATABASE_URL="${DATABASE_URL}" PORT=3000 cargo r

release:
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target=x86_64-unknown-linux-musl

release-run:
    env \
        ENV=production \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r --release

# Run the release build using a specific environment's database
# Usage: just services::release-run-env test
# Environments: test, pr, nightly, prod (default), internal
release-run-env env='prod':
    #!/bin/bash
    set -eu
    {{_get_secret_name}}
    SECRET_NAME=$(get_secret_name "{{env}}")
    DATABASE_URL=$(gcloud secrets versions access latest --secret="${SECRET_NAME}")
    ENV=production DATABASE_URL="${DATABASE_URL}" PORT=3000 cargo r --release

fmt:
    cargo fmt

lint:
    cargo clippy

# =============================================================================
# Database Migration Commands (using sqlx-cli)
# =============================================================================

# Run migrations on a specific environment's database
# Usage: just services::migrate test
# Environments: test, pr, nightly, prod (default), internal (for admin operations)
migrate env='prod':
    #!/bin/bash
    set -eu
    {{_get_secret_name}}
    SECRET_NAME=$(get_secret_name "{{env}}")
    DATABASE_URL=$(gcloud secrets versions access latest --secret="${SECRET_NAME}")
    echo "Running migrations on {{env}} environment..."
    DATABASE_URL="${DATABASE_URL}" sqlx migrate run --source ./migrations

# Revert the last migration on a specific environment's database
# Usage: just services::migrate-revert test
migrate-revert env='prod':
    #!/bin/bash
    set -eu
    {{_get_secret_name}}
    SECRET_NAME=$(get_secret_name "{{env}}")
    DATABASE_URL=$(gcloud secrets versions access latest --secret="${SECRET_NAME}")
    echo "Reverting last migration on {{env}} environment..."
    DATABASE_URL="${DATABASE_URL}" sqlx migrate revert --source ./migrations

# Check migration status on a specific environment's database
# Usage: just services::migrate-info test
migrate-info env='prod':
    #!/bin/bash
    set -eu
    {{_get_secret_name}}
    SECRET_NAME=$(get_secret_name "{{env}}")
    DATABASE_URL=$(gcloud secrets versions access latest --secret="${SECRET_NAME}")
    echo "Migration status for {{env}} environment:"
    DATABASE_URL="${DATABASE_URL}" sqlx migrate info --source ./migrations

# Create a new migration file
# Usage: just services::migrate-add add_users_table
migrate-add name:
    sqlx migrate add {{name}} --source ./migrations

# Prepare sqlx offline data (for compile-time verification)
# Usage: just services::sqlx-prepare internal
sqlx-prepare env='internal':
    #!/bin/bash
    set -eu
    {{_get_secret_name}}
    SECRET_NAME=$(get_secret_name "{{env}}")
    DATABASE_URL=$(gcloud secrets versions access latest --secret="${SECRET_NAME}")
    echo "Preparing sqlx offline data from {{env}} environment..."
    DATABASE_URL="${DATABASE_URL}" cargo sqlx prepare --workspace

# Deploys the service to Cloud Run
gcloud-deploy env image_tag:
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    # Determine service name and secret suffix based on environment
    if [ "{{env}}" == "prod" ]; then
        SERVICE_NAME="collects-services"
        SECRET_SUFFIX=""
    else
        SERVICE_NAME="collects-services-{{env}}"
        SECRET_SUFFIX="-{{env}}"
    fi

    # Assuming:
    # pr -> database-url-pr
    # test -> database-url-test
    # nightly -> database-url-nightly
    # prod -> database-url
    DATABASE_SECRET="database-url${SECRET_SUFFIX}"

    echo "Deploying ${FULL_IMAGE_NAME} to Cloud Run service ${SERVICE_NAME} in ${GCP_REGION}..."

    gcloud run deploy "${SERVICE_NAME}" \
        --image "${FULL_IMAGE_NAME}" \
        --region "${GCP_REGION}" \
        --platform managed \
        --allow-unauthenticated \
        --set-env-vars "ENV={{env}}" \
        --set-secrets "DATABASE_URL=${DATABASE_SECRET}:latest"
