# Environment Configuration:
# - run: Uses test environment URLs (configured in src/env.rs)
# - run_with_local: Starts CF worker on localhost:7788, then runs with dev environment
# - build: Test environment build
# - release: Production environment build

# Run with test environment URLs (default)
run:
    RUST_LOG=info cargo run --features test --no-default-features

# Run with local development (requires worker running on localhost:7788)
run_with_local:
    @echo "Starting Cloudflare Worker on localhost:7788..."
    @just wk-dev &
    @just run

# Build for testing environment
build:
    cargo build --features test --no-default-features

# Build for production
release:
    cargo build --release --features prod --no-default-features

lint:
    cargo clippy
    trunk fmt --check

[parallel]
install:
    cargo install trunk
    pnpm i

web-fmt: install
    pnpm prettier --write index.html src/worker.ts

web-build: install
    trunk build
    du -h dist/*

web-release: install
    trunk build --release
    du -h dist/*

wk-dev: web-build
    pnpm wrangler dev

wk-build: install web-release
    pnpm wrangler build

wk-deploy: install wk-build
    pnpm wrangler deploy

wk-type: install
    pnpm wrangler types
