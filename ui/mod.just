run:
    RUST_LOG=info cargo run

build:
    cargo build

release:
    cargo build --release

lint:
    cargo clippy
    trunk fmt --check

[parallel]
install:
    cargo install trunk
    pnpm i

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

wk-deploy env='dev': install wk-build
    pnpm wrangler deploy --env {{env}}

wk-type: install
    pnpm wrangler types
