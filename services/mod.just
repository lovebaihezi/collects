# Show the database guide and available commands for services
default:
    @echo ""
    @echo "╔══════════════════════════════════════════════════════════════════════════════╗"
    @echo "║                        COLLECTS SERVICES - SETUP GUIDE                       ║"
    @echo "╚══════════════════════════════════════════════════════════════════════════════╝"
    @echo ""
    @echo "This guide covers environment setup and common tasks for the services module."
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "1. ENVIRONMENT SETUP FOR DEPLOYMENT"
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""
    @echo "   Before deploying, set up the required secrets in GCP Secret Manager:"
    @echo ""
    @echo "   Step 1: Set up GitHub Actions (Workload Identity Federation)"
    @echo "   $ just scripts::actions-setup"
    @echo ""
    @echo "   Step 2: Initialize database secrets"
    @echo "   $ just scripts::init-db <NEON_API_TOKEN> <NEON_PROJECT_ID>"
    @echo ""
    @echo "   Step 3: Set up JWT secrets (for session tokens)"
    @echo "   $ just scripts::jwt-setup --project-id <GCP_PROJECT_ID>"
    @echo ""
    @echo "   Step 4: Set up Zero Trust secrets (required for internal env)"
    @echo "   $ just scripts::zero-trust-setup --project-id <GCP_PROJECT_ID>"
    @echo ""
    @echo "   Step 5: (Optional) Set up R2 storage secrets"
    @echo "   $ just scripts::r2-setup --project-id <GCP_PROJECT_ID>"
    @echo ""
    @echo "   ┌─────────────────────────────────────────────────────────────────────────┐"
    @echo "   │ ENVIRONMENT SECRET REQUIREMENTS                                         │"
    @echo "   ├─────────────┬───────────┬───────────┬────────────┬─────────────────────┤"
    @echo "   │ Environment │ Database  │ JWT       │ Zero Trust │ R2 Storage          │"
    @echo "   ├─────────────┼───────────┼───────────┼────────────┼─────────────────────┤"
    @echo "   │ prod        │ Required  │ Required  │ Optional   │ Optional            │"
    @echo "   │ internal    │ Required  │ Required  │ REQUIRED   │ Optional            │"
    @echo "   │ nightly     │ Required  │ Required  │ Optional   │ Optional            │"
    @echo "   │ test        │ Required  │ Default   │ Not used   │ Optional            │"
    @echo "   │ test-internal│ Required │ Default   │ REQUIRED   │ Optional            │"
    @echo "   │ pr          │ Required  │ Required  │ Not used   │ Optional            │"
    @echo "   │ local       │ Required  │ Default   │ Not used   │ Optional            │"
    @echo "   └─────────────┴───────────┴───────────┴────────────┴─────────────────────┘"
    @echo ""
    @echo "   Note: 'internal' and 'test-internal' WILL FAIL TO START without Zero Trust secrets."
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "2. DATABASE SECRETS"
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""
    @echo "   Initialize database secrets for all environments:"
    @echo ""
    @echo "   $ just scripts::init-db <NEON_API_TOKEN> <NEON_PROJECT_ID>"
    @echo ""
    @echo "   This creates database URLs for: prod, internal, test, test-internal, pr, local"
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "3. ADDING A NEW MIGRATION"
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
    @echo "4. HOW THE SQLX SYSTEM WORKS"
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
    @echo "   │ run-migrations  │ → Runs migrations (always, every deploy)"
    @echo "   └────────┬────────┘"
    @echo "            ↓"
    @echo "   ┌─────────────────┐"
    @echo "   │ build-and-deploy│ → Verifies .sqlx cache, builds, deploys"
    @echo "   └─────────────────┘"
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo "5. AVAILABLE COMMANDS"
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
    @echo "   Secret Setup (run these first for new projects):"
    @echo "   $ just scripts::actions-setup                Set up GitHub Actions + IAM"
    @echo "   $ just scripts::init-db <token> <project>    Initialize database secrets"
    @echo "   $ just scripts::jwt-setup --project-id <id>  Set up JWT secrets"
    @echo "   $ just scripts::zero-trust-setup --project-id <id>  Set up Zero Trust"
    @echo "   $ just scripts::r2-setup --project-id <id>   Set up R2 storage (optional)"
    @echo ""
    @echo "   Secret Status:"
    @echo "   $ just scripts::jwt-list --project-id <id>        Check JWT secrets"
    @echo "   $ just scripts::zero-trust-list --project-id <id> Check Zero Trust secrets"
    @echo "   $ just scripts::r2-list --project-id <id>         Check R2 secrets"
    @echo ""
    @echo "   Environments: prod, internal, nightly, test, test-internal, pr, local"
    @echo ""
    @echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    @echo ""

# Build services in debug mode
build:
    cargo b

# Run locally using the local development database
dev:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url-local) \
        PORT=3000 \
        cargo r

# Build services in release mode for production (static binary for Linux musl)
release:
    SQLX_OFFLINE=true RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target=x86_64-unknown-linux-musl

# Install sqlx-cli for database migrations
install-sqlx:
    cargo install sqlx-cli --no-default-features --features native-tls,postgres

# Add a new SQLx migration file
# Usage: just services::add-migrate <migration_name>
add-migrate *args: install-sqlx
    sqlx migrate add {{args}}

# Run sqlx migrations for a specific environment
# Environments: prod, internal, nightly, test, test-internal, pr, local
# Usage: just services::migrate local
migrate env: install-sqlx
    #!/bin/bash
    set -euo pipefail

    # Get database secret using centralized env config
    DATABASE_SECRET=$(just scripts::env-secret {{env}})

    echo "Running migrations for {{env}} environment using secret: ${DATABASE_SECRET}"

    # Avoid leaking the secret via xtrace in CI logs (never enable xtrace).
    set +x
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL

    sqlx migrate run

# Check if migrations are pending for a specific environment
# Usage: just services::migrate-info local
migrate-info env: install-sqlx
    #!/bin/bash
    set -euo pipefail

    DATABASE_SECRET=$(just scripts::env-secret {{env}})

    echo "Checking migration status for {{env}} environment..."

    # Avoid leaking the secret via xtrace in CI logs (never enable xtrace).
    set +x
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL

    sqlx migrate info

# Reset database for a specific environment (drops and recreates)
# Usage: just services::reset <env>
reset env: install-sqlx
    #!/bin/bash
    set -euo pipefail

    DATABASE_SECRET=$(just scripts::env-secret {{env}})

    echo "Resetting database for {{env}} environment..."

    # Avoid leaking the secret via xtrace in CI logs (never enable xtrace).
    set +x
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL

    sqlx database reset

# Create sqlx offline query cache for builds
# This generates .sqlx directory with query metadata
# Usage: just services::prepare local
prepare env: install-sqlx
    #!/bin/bash
    set -euo pipefail

    DATABASE_SECRET=$(just scripts::env-secret {{env}})

    echo "Generating sqlx offline cache using {{env}} environment..."

    # Avoid leaking the secret via xtrace in CI logs (never enable xtrace).
    set +x
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL

    # Run from workspace root to generate .sqlx at the workspace level
    cd .. && cargo sqlx prepare --workspace

# Check if sqlx offline cache is up to date
# Usage: just services::prepare-check local
prepare-check env: install-sqlx
    #!/bin/bash
    set -euo pipefail

    DATABASE_SECRET=$(just scripts::env-secret {{env}})

    echo "Checking sqlx offline cache using {{env}} environment..."

    # Avoid leaking the secret via xtrace in CI logs (never enable xtrace).
    set +x
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

# Format Rust code in services
fmt:
    cargo fmt

# Run cargo clippy lints for services
lint:
    cargo clippy

# Run tests for services using gcloud secrets
# Environments: prod, internal, nightly, test, test-internal, pr, local
# Usage: just services::test local
test env="local":
    #!/bin/bash
    set -euo pipefail

    DATABASE_SECRET=$(just scripts::env-secret {{env}})

    echo "Running tests for {{env}} environment..."

    # Fetch database URL
    set +x
    DATABASE_URL=$(gcloud secrets versions access latest --secret=${DATABASE_SECRET})
    export DATABASE_URL

    # Fetch R2 secrets if available (optional for tests)
    if gcloud secrets versions access latest --secret=cf-account-id 2>/dev/null; then
        export CF_ACCOUNT_ID=$(gcloud secrets versions access latest --secret=cf-account-id)
        export CF_ACCESS_KEY_ID=$(gcloud secrets versions access latest --secret=cf-access-key-id)
        export CF_SECRET_ACCESS_KEY=$(gcloud secrets versions access latest --secret=cf-secret-access-key)
        export CF_BUCKET=$(gcloud secrets versions access latest --secret=cf-bucket)
        echo "R2 secrets loaded"
    else
        echo "R2 secrets not found, running tests without R2 configuration"
    fi

    cargo test -p collects-services

# Run tests without database (unit tests only, offline mode)
test-unit:
    SQLX_OFFLINE=true cargo test -p collects-services --lib

# Deploys the service to Cloud Run
# Environments: prod, internal, nightly, test, test-internal, pr
# Usage: just services::gcloud-deploy prod 20251017-1
gcloud-deploy env image_tag:
    just scripts::gcloud-deploy {{env}} {{image_tag}}
