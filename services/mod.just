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
    cargo b --release

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
