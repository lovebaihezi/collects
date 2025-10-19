default: fmt

prepare:
    env \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        cargo sqlx prepare

build: prepare
    cargo b

dev: prepare
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r

release: prepare
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
