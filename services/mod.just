default:
    @echo ""
    @echo "╔══════════════════════════════════════════════════════════════════════════════╗"
    @echo "║                        COLLECTS SERVICES - DATABASE GUIDE                    ║"
    @echo "╚══════════════════════════════════════════════════════════════════════════════╝"
    @echo ""
    @echo "This project uses SQLx with Neon PostgreSQL. Below is a guide for common tasks."
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "1. INITIALIZE DATABASE SECRETS"
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""
    @echo "   Run the following command to initialize database secrets in GCP:"
    @echo ""
    @echo "   $ just scripts::init-db <NEON_API_TOKEN> <NEON_PROJECT_ID>"
    @echo ""
    @echo "   This will:"
    @echo "   - Create database URLs for all environments (prod, internal, test, test-internal, pr, local)"
    @echo "   - Store them as secrets in Google Cloud Secret Manager"
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "2. ADDING A NEW MIGRATION"
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""
    @echo "   Step 1: Create the migration file"
    @echo "   $ just services::add-migrate <migration_name>"
    @echo ""
    @echo "   Step 2: Edit the generated file in services/migrations/"
    @echo "   - Add your SQL statements to the .up.sql file"
    @echo ""
    @echo "   Step 3: Run the migration locally to test"
    @echo "   $ just services::migrate local"
    @echo ""
    @echo "   Step 4: Update the SQLx offline cache"
    @echo "   $ just services::prepare local"
    @echo ""
    @echo "   Step 5: Commit both the migration AND the .sqlx directory"
    @echo "   $ git add services/migrations/ .sqlx/"
    @echo "   $ git commit -m \"Add migration: <description>\""
    @echo ""
    @echo "   IMPORTANT: The .sqlx directory MUST be committed. CI will fail otherwise."
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "3. HOW THE SQLX SYSTEM WORKS"
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""
    @echo "   SQLx provides compile-time SQL verification. Here's how it works:"
    @echo ""
    @echo "   LOCAL DEVELOPMENT:"
    @echo "   - SQLx connects to the database at compile time to verify queries"
    @echo "   - Requires DATABASE_URL to be set"
    @echo ""
    @echo "   CI/CD (OFFLINE MODE):"
    @echo "   - Builds use SQLX_OFFLINE=true (no database connection needed)"
    @echo "   - Query metadata is read from the .sqlx/ directory"
    @echo "   - This is why .sqlx/ must be committed and kept up-to-date"
    @echo ""
    @echo "   WORKFLOW:"
    @echo "   ┌─────────────────┐"
    @echo "   │ check-migrations│ → Detects if migrations changed"
    @echo "   └────────┬────────┘"
    @echo "            ↓"
    @echo "   ┌─────────────────┐"
    @echo "   │ run-migrations  │ → Runs migrations (if changed)"
    @echo "   └────────┬────────┘"
    @echo "            ↓"
    @echo "   ┌─────────────────┐"
    @echo "   │ build-and-deploy│ → Verifies .sqlx cache, builds, deploys"
    @echo "   └─────────────────┘"
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "4. AVAILABLE COMMANDS"
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""
    @echo "   Development:"
    @echo "   $ just services::dev              Run locally with local database"
    @echo "   $ just services::build            Build in debug mode"
    @echo "   $ just services::release          Build release binary (offline mode)"
    @echo ""
    @echo "   Migrations:"
    @echo "   $ just services::add-migrate <name>   Create a new migration"
    @echo "   $ just services::migrate <env>        Run migrations for environment"
    @echo "   $ just services::migrate-info <env>   Show migration status"
    @echo ""
    @echo "   SQLx Cache:"
    @echo "   $ just services::prepare <env>        Generate offline cache"
    @echo "   $ just services::prepare-check <env>  Verify cache is up-to-date"
    @echo ""
    @echo "   Deployment:"
    @echo "   $ just services::gcloud-deploy <env> <tag>   Deploy to Cloud Run"
    @echo ""
    @echo "   Environments: prod, internal, nightly, test, test-internal, pr, local"
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""

build:
    cargo b

# Run locally using the local development database
dev:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url-local) \
        PORT=3000 \
        cargo r

release:
    SQLX_OFFLINE=true RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target=x86_64-unknown-linux-musl

install-sqlx:
    cargo install sqlx-cli --no-default-features --features native-tls,postgres

# Run migrate of sqlx(a bounch of database migrations)

add-migrate *args: install-sqlx
    sqlx migrate add {{args}}

# Run sqlx migrations for a specific environment
# Environments: prod, internal, nightly, test, test-internal, pr, local
# Usage: just services::migrate local
migrate env: install-sqlx
    #!/bin/bash
    set -eux

    # Determine database secret based on environment
    # Mapping:
    #   prod          -> database-url
    #   nightly       -> database-url (uses prod database)
    #   internal      -> database-url-internal
    #   test          -> database-url-test
    #   test-internal -> database-url-test-internal
    #   pr            -> database-url-pr
    #   local         -> database-url-local
    if [ "{{env}}" == "prod" ] || [ "{{env}}" == "nightly" ]; then
        DATABASE_SECRET="database-url"
    else
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Running migrations for {{env}} environment using secret: ${DATABASE_SECRET}"
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL
    sqlx migrate run

# Check if migrations are pending for a specific environment
# Usage: just services::migrate-info local
migrate-info env: install-sqlx
    #!/bin/bash
    set -eux

    if [ "{{env}}" == "prod" ] || [ "{{env}}" == "nightly" ]; then
        DATABASE_SECRET="database-url"
    else
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Checking migration status for {{env}} environment..."
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL
    sqlx migrate info

reset env: install-sqlx
    #!/bin/bash
    set -eux

    if [ "{{env}}" == "prod" ] || [ "{{env}}" == "nightly" ]; then
        DATABASE_SECRET="database-url"
    else
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Resetting database for {{env}} environment..."
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL
    sqlx database reset

# Create sqlx offline query cache for builds
# This generates .sqlx directory with query metadata
# Usage: just services::prepare local
prepare env: install-sqlx
    #!/bin/bash
    set -eux

    if [ "{{env}}" == "prod" ] || [ "{{env}}" == "nightly" ]; then
        DATABASE_SECRET="database-url"
    else
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Generating sqlx offline cache using {{env}} environment..."
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL
    # Run from workspace root to generate .sqlx at the workspace level
    cd .. && cargo sqlx prepare --workspace

# Check if sqlx offline cache is up to date
# Usage: just services::prepare-check local
prepare-check env: install-sqlx
    #!/bin/bash
    set -eux

    if [ "{{env}}" == "prod" ] || [ "{{env}}" == "nightly" ]; then
        DATABASE_SECRET="database-url"
    else
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Checking sqlx offline cache using {{env}} environment..."
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL
    # Run from workspace root where .sqlx directory is located
    cd .. && cargo sqlx prepare --workspace --check

# Run release build locally using the local development database
release-run:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url-local) \
        PORT=3000 \
        cargo r --release

fmt:
    cargo fmt

lint:
    cargo clippy

# Deploys the service to Cloud Run
# Environments: prod, internal, nightly, test, test-internal, pr
# Usage: just services::gcloud-deploy prod 20251017-1
gcloud-deploy env image_tag:
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    # Determine service name and secret suffix based on environment
    # Mapping:
    #   prod          -> collects-services               -> database-url
    #   nightly       -> collects-services-nightly       -> database-url (uses prod database)
    #   internal      -> collects-services-internal      -> database-url-internal
    #   test          -> collects-services-test          -> database-url-test
    #   test-internal -> collects-services-test-internal -> database-url-test-internal
    #   pr            -> collects-services-pr            -> database-url-pr
    if [ "{{env}}" == "prod" ]; then
        SERVICE_NAME="collects-services"
        DATABASE_SECRET="database-url"
    elif [ "{{env}}" == "nightly" ]; then
        SERVICE_NAME="collects-services-nightly"
        DATABASE_SECRET="database-url"
    else
        SERVICE_NAME="collects-services-{{env}}"
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Deploying ${FULL_IMAGE_NAME} to Cloud Run service ${SERVICE_NAME} in ${GCP_REGION}..."
    echo "Using database secret: ${DATABASE_SECRET}"

    gcloud run deploy "${SERVICE_NAME}" \
        --image "${FULL_IMAGE_NAME}" \
        --region "${GCP_REGION}" \
        --platform managed \
        --allow-unauthenticated \
        --startup-probe httpGet.path=/is-health,httpGet.port=8080,initialDelaySeconds=24,timeoutSeconds=240,periodSeconds=240,failureThreshold=3 \
        --set-env-vars "ENV={{env}},GCP_PROJECT_ID=${PROJECT_ID}" \
        --set-secrets "DATABASE_URL=${DATABASE_SECRET}:latest"
