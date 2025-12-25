[working-directory: 'services']
install:
    bun i

[working-directory: 'services']
fmt: install
    bun prettier --write *.ts package.json tsconfig.json .oxlintrc.json

[working-directory: 'services']
check-fmt:
    bun prettier --check *.ts package.json tsconfig.json .oxlintrc.json

[working-directory: 'services']
check-lint:
    bun oxlint --type-aware

# Runs the setup script to get the yaml config
[working-directory: 'services']
actions-setup *args: install
    bun run main.ts actions-setup {{args}}

[working-directory: 'services']
init-db token project_id: install
    bun run main.ts init-db-secret --token {{token}} --project-id {{project_id}}
