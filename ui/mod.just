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

web-fmt: install
    pnpm prettier --write src/worker.ts index.html

web-release: install
    trunk build --release
    du -h dist/*

wk-dev: web-build
    pnpm wrangler dev

wk-build: install web-release
    pnpm wrangler build

wk-deploy env="": install wk-build
    pnpm wrangler deploy {{ if env != "" { "--env " + env } else { "" } }}

wk-type: install
    pnpm wrangler types

dist-native target output_name features="":
    cargo build --release --target {{target}} {{ if features != "" { "--features " + features } else { "" } }}

    # Handle optional .exe extension for Windows
    if [ -f "target/{{target}}/release/collects-ui.exe" ]; then \
        mv target/{{target}}/release/collects-ui.exe {{output_name}}; \
    else \
        mv target/{{target}}/release/collects-ui {{output_name}}; \
    fi
