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

# Builds and pushes a multi-arch `collects-services` Docker image to Google Artifact Registry.
docker-push image_tag:
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    # We use linux/amd64 because we are building for x86_64-unknown-linux-musl
    docker buildx build --platform linux/amd64 --tag "${FULL_IMAGE_NAME}" . --push

# Deploys the service to Cloud Run
gcloud-deploy env image_tag:
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    # Determine service name and secret suffix based on environment
    if [ "{{env}}" == "prod" ]; then
        SERVICE_NAME="collects-services"
        SECRET_SUFFIX=""
    else
        SERVICE_NAME="collects-services-{{env}}"
        SECRET_SUFFIX="-{{env}}"
    fi

    # Handle the case where prod might not use a suffix (or uses -prod if configured that way, but we assume no suffix for prod per standard)
    # However, if user followed "Option A: Use different secret names per env", prod usually implies the base name or specific prod name.
    # Assuming:
    # internal -> database-url-internal
    # test -> database-url-test
    # nightly -> database-url-nightly
    # prod -> database-url (as implied by existing justfile)

    DATABASE_SECRET="database-url${SECRET_SUFFIX}"
    CLERK_SECRET="clerk-frontend-api${SECRET_SUFFIX}"

    echo "Deploying ${FULL_IMAGE_NAME} to Cloud Run service ${SERVICE_NAME} in ${GCP_REGION}..."

    gcloud run deploy "${SERVICE_NAME}" \
        --image "${FULL_IMAGE_NAME}" \
        --region "${GCP_REGION}" \
        --platform managed \
        --allow-unauthenticated \
        --set-env-vars "ENV=prod" \
        --set-secrets "DATABASE_URL=${DATABASE_SECRET}:latest,CLERK_FRONTEND_API=${CLERK_SECRET}:latest"
