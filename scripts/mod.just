# Install Node.js dependencies using bun
install:
    bun i

# Check that Commands use LatestOnlyUpdater (and do not use the legacy Updater signature)
# Usage: just scripts::command-updater-check
command-updater-check: install
    bun run main.ts command-updater-check

# Format TypeScript and JSON files with prettier
fmt:
    bun prettier --write **/*.ts package.json tsconfig.json .oxlintrc.json

# Check TypeScript and JSON file formatting with prettier
check-fmt:
    bun prettier --check **/*.ts package.json tsconfig.json .oxlintrc.json

# Run oxlint with type-aware checks
check-lint:
    bun oxlint --type-aware

# Run tests
test: install
    bun test

# Get cargo feature flags for an environment (used by other modules)
# Usage: just scripts::env-feature pr
env-feature env="":
    @bun run main.ts env-feature {{env}}

# Get database secret name for an environment (used by other modules)
# Usage: just scripts::env-secret local
env-secret env:
    @bun run main.ts env-secret {{env}}

# Get JWT secret name for an environment (used by other modules)
# Usage: just scripts::jwt-secret pr
jwt-secret env:
    @bun run main.ts jwt-secret {{env}}

# Get R2 storage secrets for an environment (used by other modules)
# Returns comma-separated secrets string for gcloud deploy, or empty if R2 not required
# Usage: just scripts::r2-secrets pr
r2-secrets env:
    @bun run main.ts r2-secrets {{env}}

# Deploy services to Cloud Run with appropriate secrets
# Usage: just scripts::gcloud-deploy prod v2026.1.3
gcloud-deploy env image_tag:
    @bun run main.ts gcloud-deploy {{env}} {{image_tag}}

# List all available environment names
env-list:
    @bun run main.ts env-list

# Runs the setup script to get the yaml config
actions-setup *args: install
    bun run main.ts actions-setup {{args}}

# Migrate workload identity bindings when repository is moved
# Usage: just scripts::actions-migrate-repo --project-id <id> --old-repo <old-owner/old-repo> --new-repo <new-owner/new-repo>
actions-migrate-repo *args: install
    bun run main.ts actions-migrate-repo {{args}}

# Initialize database secrets in Google Cloud Secret Manager
# Usage: just scripts::init-db <NEON_API_TOKEN> <NEON_PROJECT_ID>
init-db token project_id: install
    bun run main.ts init-db-secret --token {{token}} --project-id {{project_id}}

# Validate PR title against conventional commit format
# Usage: just scripts::check-pr-title "feat: add new feature"
check-pr-title title: install
    bun run main.ts check-pr-title "{{title}}"

# Setup Cloudflare R2 secrets in Google Cloud Secret Manager
# Usage: just scripts::r2-setup my-gcp-project-id
r2-setup *args: install
    bun run main.ts r2-setup {{args}}

# List Cloudflare R2 secrets status
# Usage: just scripts::r2-list my-gcp-project-id
r2-list *args: install
    bun run main.ts r2-list {{args}}

# Setup JWT secrets in Google Cloud Secret Manager (auto-generates secure secrets)
# Usage: just scripts::jwt-setup --project-id my-gcp-project-id
jwt-setup *args: install
    bun run main.ts jwt-setup {{args}}

# List JWT secrets status
# Usage: just scripts::jwt-list --project-id my-gcp-project-id
jwt-list *args: install
    bun run main.ts jwt-list {{args}}

# Setup Cloudflare Zero Trust secrets in Google Cloud Secret Manager
# Required for internal environment deployment
# Usage: just scripts::zero-trust-setup --project-id my-gcp-project-id
zero-trust-setup *args: install
    bun run main.ts zero-trust-setup {{args}}

# List Cloudflare Zero Trust secrets status
# Usage: just scripts::zero-trust-list --project-id my-gcp-project-id
zero-trust-list *args: install
    bun run main.ts zero-trust-list {{args}}

# Post CI failure feedback to PR (for GitHub Actions)
# Requires environment variables: GITHUB_TOKEN, WORKFLOW_RUN_ID, HEAD_SHA, WORKFLOW_RUN_URL
ci-feedback: install
    bun run main.ts ci-feedback

# Create GitHub issue when scheduled job fails (for GitHub Actions)
# Requires environment variables: GITHUB_TOKEN, WORKFLOW_RUN_ID, WORKFLOW_NAME, HEAD_SHA, WORKFLOW_RUN_URL
scheduled-job-issue: install
    bun run main.ts scheduled-job-issue

# Cleanup old Docker images from Artifact Registry (for GitHub Actions)
# Environment variables:
#   - GCP_PROJECT_ID: Google Cloud Project ID (optional)
#   - DRY_RUN: Set to "true" for preview mode
artifact-cleanup: install
    bun run main.ts artifact-cleanup

# Check Docker images in Artifact Registry and verify cleanup status
# Environment variables:
#   - GCP_PROJECT_ID: Google Cloud Project ID (optional)
# Usage: just scripts::artifact-check
artifact-check: install
    bun run main.ts artifact-check

# Check that locked migration files haven't been modified
# This verifies migration integrity - locked migrations should never change
# Usage: just scripts::migration-check
migration-check: install
    bun run main.ts migration-check

# Lock new migration files (add them to the checksum file)
# Run this after migrations have been applied to production
# Usage: just scripts::migration-lock
migration-lock: install
    bun run main.ts migration-lock

# Display sccache statistics and cache performance metrics (for GitHub Actions)
# Outputs stats to GitHub Actions and stops the sccache server
# Usage: just scripts::sccache-stats
sccache-stats: install
    bun run main.ts sccache-stats

# Run ast-grep rule tests
# Tests rules defined in rules/ directory against test cases in rule-tests/
# Usage: just scripts::sg-test
sg-test:
    sg test

# Run ast-grep scan on the codebase
# Scans for violations using rules defined in rules/ directory
# Usage: just scripts::sg-scan
sg-scan:
    sg scan
