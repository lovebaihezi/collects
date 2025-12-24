[working-directory: 'services']
install:
    bun i

[working-directory: 'services']
fmt: install
    bun prettier --write main.ts

[working-directory: 'services']
check-fmt:
    bun prettier --check main.ts

# Runs the setup script to get the yaml config
[working-directory: 'services']
services *args: install
    bun run main.ts {{args}}

[working-directory: 'services']
init-db token: install
    bun run main.ts init-db-secret --token {{token}}

# =============================================================================
# Database Migration Commands
# =============================================================================

# Run migrations on a specific environment
# Usage: just scripts::migrate test
# Environments: prod, test, pr, nightly, internal (for admin operations)
[working-directory: 'services']
migrate env='internal': install
    bun run main.ts migrate {{env}}

# Check migration status on a specific environment
# Usage: just scripts::migrate-info prod
[working-directory: 'services']
migrate-info env='prod': install
    bun run main.ts migrate-info {{env}}

# Revert the last migration on a specific environment
# Usage: just scripts::migrate-revert test
[working-directory: 'services']
migrate-revert env: install
    bun run main.ts migrate-revert {{env}}

# Display database URL for an environment (masked)
# Usage: just scripts::show-db-url prod
[working-directory: 'services']
show-db-url env: install
    bun run main.ts show-db-url {{env}}
