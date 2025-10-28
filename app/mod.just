run:
    cargo run

build:
    cargo build

release:
    cargo build --release

lint:
    cargo clippy
    trunk fmt --check

install:
    pnpm i

auth-build-css: install
    pnpx @tailwindcss/cli -i auth/index.css -o auth/output.css

auth-build-ts: install
    pnpm tsc auth/auth.ts \
        --lib es2020,dom \
        --outDir auth \
        --target esnext \
        --module esnext \
        --moduleResolution bundler \
        --esModuleInterop \
        --skipLibCheck

[parallel]
auth-build: auth-build-css auth-build-ts
    @echo "finsihed"

web-build: auth-build
    trunk build
    cp -r auth dist
    du -h dist/*

web-release: auth-build
    trunk build --release
    cp -r auth dist
    du -h dist/*

wk-dev: web-build
    pnpm wrangler dev

wk-build: install web-release
    pnpm wrangler build

wk-deploy: install wk-build
    pnpm wrangler deploy

wk-type: install
    pnpm wrangler types
