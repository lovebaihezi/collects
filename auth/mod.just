install:
    pnpm install

# Type checking
type-check: install
    pnpm run type-check

# Linting
lint: install
    pnpm run lint

lint-fix: install
    pnpm run lint:fix

# Formatting
format: install
    pnpm run format

format-check: install
    pnpm run format:check

# Run all checks (type-check, lint, format)
check: install
    pnpm run check

# Build frontend
web-release: install
    pnpm build
    du -h dist/*

# Wrangler commands
wk-dev: web-release
    pnpm wrangler dev

wk-build: install web-release
    pnpm wrangler build

wk-deploy: install wk-build
    pnpm wrangler deploy

wk-type: install
    pnpm wrangler types

# Clean build artifacts
clean:
    rm -rf dist node_modules/.vite node_modules/.tmp
    rm -f worker-configuration.d.ts

# Full clean including node_modules
clean-all: clean
    rm -rf node_modules
