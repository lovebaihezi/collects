run:
    cargo run

build:
    cargo build

release:
    cargo build --release

lint:
    cargo clippy
    trunk fmt --check

web-build:
    trunk build
    du -h dist/*

web-release:
    trunk build --release
    du -h dist/*

wk-init:
    pnpm i

wk-dev: web-build
    pnpm wrangler dev

wk-build: web-release
    pnpm wrangler build

wk-deploy: wk-build
    pnpm wrangler deploy

wk-type:
    pnpm wrangler types
