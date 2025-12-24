default: fmt

build:
    cargo b

# Run locally using the local development database
dev:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url-local) \
        PORT=3000 \
        cargo r

release:
    RUSTFLAGS="-C target-feature=+crt-static" cargo build --release --target=x86_64-unknown-linux-musl

# Run release build locally using the local development database
release-run:
    env \
        ENV=local \
        DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url-local) \
        PORT=3000 \
        cargo r --release

fmt:
    cargo fmt

lint:
    cargo clippy

# Deploys the service to Cloud Run
# Environments: prod, internal, test, pr
# Usage: just services::gcloud-deploy prod 20251017-1
gcloud-deploy env image_tag:
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    # Determine service name and secret suffix based on environment
    # Mapping:
    #   prod     -> collects-services          -> database-url
    #   internal -> collects-services-internal -> database-url-internal
    #   test     -> collects-services-test     -> database-url-test
    #   pr       -> collects-services-pr       -> database-url-pr
    if [ "{{env}}" == "prod" ]; then
        SERVICE_NAME="collects-services"
        DATABASE_SECRET="database-url"
    else
        SERVICE_NAME="collects-services-{{env}}"
        DATABASE_SECRET="database-url-{{env}}"
    fi

    echo "Deploying ${FULL_IMAGE_NAME} to Cloud Run service ${SERVICE_NAME} in ${GCP_REGION}..."
    echo "Using database secret: ${DATABASE_SECRET}"

    gcloud run deploy "${SERVICE_NAME}" \
        --image "${FULL_IMAGE_NAME}" \
        --region "${GCP_REGION}" \
        --platform managed \
        --allow-unauthenticated \
        --set-env-vars "ENV={{env}}" \
        --set-secrets "DATABASE_URL=${DATABASE_SECRET}:latest"
