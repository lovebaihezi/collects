install:
    bun i

fmt:
    bun prettier --write **/*.ts package.json tsconfig.json .oxlintrc.json

check-fmt:
    bun prettier --check **/*.ts package.json tsconfig.json .oxlintrc.json

check-lint:
    bun oxlint --type-aware

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

init-db token project_id: install
    bun run main.ts init-db-secret --token {{token}} --project-id {{project_id}}

# Validate PR title against conventional commit format
# Usage: just scripts::check-pr-title "feat: add new feature"
check-pr-title title: install
    bun run main.ts check-pr-title "{{title}}"
