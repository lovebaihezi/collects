[working-directory: 'services']
install:
    bun i

[working-directory: 'services']
fmt: install
    bun prettier --write main.ts

[working-directory: 'services']
check-fmt:
    bun prettier --check main.ts neon.ts utils.ts

[working-directory: 'services']
check-lint:
    bun oxlint --type-aware

# Runs the setup script to get the yaml config
[working-directory: 'services']
services *args: install
    bun run main.ts {{args}}

[working-directory: 'services']
init-db token project_id: install
    bun run main.ts init-db-secret --token {{token}} --project-id {{project_id}}
