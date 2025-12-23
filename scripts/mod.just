[working-directory: 'services']
install:
    bun i

[working-directory: 'services']
fmt: install
    bun prettier --write main.ts

# Runs the setup script to get the yaml config
[working-directory: 'services']
services *args: install
    bun run main.ts {{args}}
