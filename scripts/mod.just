install:
    bun i

fmt:
    bun prettier --write **/*.ts package.json tsconfig.json .oxlintrc.json

check-fmt:
    bun prettier --check **/*.ts package.json tsconfig.json .oxlintrc.json

check-lint:
    bun oxlint --type-aware

# Runs the setup script to get the yaml config
actions-setup *args: install
    bun run main.ts actions-setup {{args}}

init-db token project_id: install
    bun run main.ts init-db-secret --token {{token}} --project-id {{project_id}}
