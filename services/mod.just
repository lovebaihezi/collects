default: fmt

build:
    cargo b

dev:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r

release:
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target=x86_64-unknown-linux-musl

release-run:
    env \
        ENV=production \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        PORT=3000 \
        cargo r --release

fmt:
    cargo fmt

lint:
    cargo clippy
