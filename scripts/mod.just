install:
    bun i

fmt:
    bun prettier --write **/*.ts package.json tsconfig.json .oxlintrc.json

check-fmt:
    bun prettier --check **/*.ts package.json tsconfig.json .oxlintrc.json

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
# Post CI failure feedback to PR (for GitHub Actions)
# Requires environment variables: GITHUB_TOKEN, WORKFLOW_RUN_ID, HEAD_SHA, WORKFLOW_RUN_URL
ci-feedback: install
    bun run main.ts ci-feedback

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
