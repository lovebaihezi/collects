mod app
mod services

set shell := ["bash", "-c"]

create-builder:
    sudo docker buildx create --name collects-builder --use

# Authenticates the local Docker client with Google Artifact Registry.
# It's best practice to run this once per region you interact with.
# Usage: just docker-login us-east1
docker-login region='us-east1':
    gcloud auth configure-docker {{region}}-docker.pkg.dev

# Builds the `collects-services` Docker image for the local platform.
# This command first builds the static binary and then builds the Docker image.
# Usage: just docker-build 20251017-1
docker-build image_tag: services::release
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    sudo docker buildx build --load -t "${FULL_IMAGE_NAME}" .

# Builds and pushes a multi-arch `collects-services` Docker image to Google Artifact Registry.
# Usage: just docker-push 20251017-1
docker-push image_tag: services::release
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    sudo docker buildx build --platform linux/amd64,linux/arm64 -t "${FULL_IMAGE_NAME}" . --push

# Runs the `collects-services` Docker image locally for testing.
# Usage: just docker-run 20251017-1
docker-run image_tag: (docker-build image_tag)
    #!/bin/bash
    set -eux

    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services"
    IMAGE_NAME="collects-services"
    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    sudo docker run --rm -p 3000:3000 \
        -e ENV=prod \
        -e PORT=3000 \
        -e DATABASE_URL=$(gcloud secrets versions access latest --secret=database-url) \
        "${FULL_IMAGE_NAME}"
