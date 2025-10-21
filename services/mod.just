default: fmt

install-sqlx-cli:
    cargo install sqlx-cli

prepare: install-sqlx-cli
    env \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        cargo sqlx prepare

migrate: install-sqlx-cli
    env \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        cargo sqlx migrate run

build: migrate prepare
    cargo b

dev: prepare
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r

release:
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target=x86_64-unknown-linux-musl

release-run: prepare
    env \
        ENV=production \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r --release

fmt:
    cargo fmt

lint:
    cargo clippy
