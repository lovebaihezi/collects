mod app
mod services

# Authenticates the local Docker client with Google Artifact Registry.
# It's best practice to run this once per region you interact with.
# Usage: just docker-login us-east1
docker-login region='us-east1':
    gcloud auth configure-docker {{region}}-docker.pkg.dev

# Builds the `collects-services` Docker image.
# This command first builds the static binary and then builds the Docker image.
# Usage: just docker-build 20251017-1
docker-build image_tag: services::release
    #!/usr/bin/env sh
    set -eux

    # --- Configuration ---
    # Define these variables to match your GCP environment.
    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services" # The name of your Artifact Registry repo
    IMAGE_NAME="collects-services"      # The name of the image itself
    # --- End Configuration ---

    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    echo "Building image: ${FULL_IMAGE_NAME}"

    sudo docker build -t "${FULL_IMAGE_NAME}" .

# Pushes the `collects-services` Docker image to Google Artifact Registry.
# Usage: just docker-push 20251017-1
docker-push image_tag: (docker-build image_tag)
    #!/usr/bin/env sh
    set -eux

    # --- Configuration ---
    # Define these variables to match your GCP environment.
    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services" # The name of your Artifact Registry repo
    IMAGE_NAME="collects-services"      # The name of the image itself
    # --- End Configuration ---

    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    echo "Pushing image: ${FULL_IMAGE_NAME}"

    docker push "${FULL_IMAGE_NAME}"

    echo "Successfully pushed image: ${FULL_IMAGE_NAME}"

# Runs the `collects-services` Docker image locally for testing.
# Usage: just docker-run 20251017-1
docker-run image_tag: (docker-build image_tag)
    #!/usr/bin/env sh
    set -eux

    # --- Configuration ---
    # Define these variables to match your GCP environment.
    GCP_REGION="us-east1"
    PROJECT_ID=$(gcloud config get-value project)
    REPOSITORY_NAME="collects-services" # The name of your Artifact Registry repo
    IMAGE_NAME="collects-services"      # The name of the image itself
    # --- End Configuration ---

    FULL_IMAGE_NAME="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/${REPOSITORY_NAME}/${IMAGE_NAME}:{{image_tag}}"

    echo "Running image: ${FULL_IMAGE_NAME}"

    sudo docker run -p 3000:3000 "${FULL_IMAGE_NAME}"
