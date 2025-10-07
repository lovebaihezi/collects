# Deployment Guide: `collects-services` on Google Cloud Run with Artifact Registry

This document provides a complete, step-by-step guide for deploying the `collects-services` application to Google Cloud Run.

This workflow uses **Google Artifact Registry** to host the private Docker container image, which is the recommended best practice for deploying to Google Cloud. It simplifies authentication and improves deployment performance. Secrets are managed securely using **Google Secret Manager**, and the image is built using the modern **`docker buildx`** command.

## Table of Contents

1.  **Prerequisites**: Tools and accounts required.
2.  **One-Time Infrastructure Setup**: Configuring Artifact Registry, secrets, and permissions.
3.  **Build and Push Process**: Compiling the application and publishing the Docker image with `docker buildx`.
4.  **Deployment to Google Cloud Run**: Releasing the new version.
5.  **Deployment Checks**: Verifying the service is running correctly.
6.  **Troubleshooting**: Common issues and solutions.

---

## 1. Prerequisites

Before you begin, ensure you have the following:

-   **Google Cloud SDK (`gcloud`)**: Installed and authenticated to your Google Cloud project.
-   **Docker Desktop (v20.10 or later)**: Installed and running locally. `docker buildx` is included by default.
-   **Just**: The command runner used for this project.
-   **A Google Cloud Project**: With billing enabled.

---

## 2. One-Time Infrastructure Setup

These steps configure the necessary Google Cloud services and permissions for a secure deployment pipeline.

### 2.1. Enable Google Cloud APIs

Ensure the necessary APIs are enabled for your project:
```sh
gcloud services enable run.googleapis.com secretmanager.googleapis.com artifactregistry.googleapis.com iam.googleapis.com
```

### 2.2. Create an Artifact Registry Repository

This is where your Docker images will be stored. Create a repository named `services`.

-   Replace `[GCP_REGION]` with your desired region (e.g., `us-central1`). It's best to use the same region where you will deploy your Cloud Run service.

```sh
gcloud artifacts repositories create services \
    --repository-format=docker \
    --location="[GCP_REGION]" \
    --description="Docker repository for collects services"
```

### 2.3. Configure Local Docker Authentication

Configure your local Docker client to authenticate with your new Artifact Registry repository. This command only needs to be run once per machine.

```sh
gcloud auth configure-docker [GCP_REGION]-docker.pkg.dev
```

### 2.4. Store the `DATABASE_URL` in Secret Manager

Sensitive data must never be included in the Docker image. We will store it in Secret Manager and inject it at runtime.

1.  **Create the secret**:
    ```sh
    gcloud secrets create database-url --replication-policy="automatic"
    ```

2.  **Add your database connection string** (e.g., `postgres://user:pass@host:port/db`) as the secret's value:
    ```sh
    gcloud secrets versions add database-url --data-file=-
    # (Paste the DATABASE_URL, press Enter, then Ctrl+D)
    ```

### 2.5. Configure IAM Service Account Permissions

Your Cloud Run service uses a Service Account identity. We need to grant it permission to access the database secret.

1.  **Identify Your Service Account**: We assume you are using the **Default Compute Service Account**. Find its email in the Google Cloud Console under `IAM & Admin > Service Accounts`. It looks like `[PROJECT_NUMBER]-compute@developer.gserviceaccount.com`.

2.  **Grant Access to the Database URL Secret**:
    ```sh
    gcloud secrets add-iam-policy-binding database-url \
      --member="serviceAccount:[YOUR_SERVICE_ACCOUNT_EMAIL]" \
      --role="roles/secretmanager.secretAccessor"
    ```
    *Note: The service account automatically has permission to pull images from Artifact Registry (`roles/artifactregistry.reader`) within the same project, so no extra IAM configuration is needed for that.*

---

## 3. Build and Push Process with Docker Buildx

This is the repeatable process for building and publishing a new version of the service using a single, efficient command.

### 3.1. Compile the Rust Binary

From the project root (`/collects`), run the release build command:
```sh
just services::release
```
This compiles the optimized binary and places it in `target/release/collects-services`.

### 3.2. Build and Push the Docker Image

The `docker buildx build` command allows us to build the image and push it to the repository in one step.

1.  **Define Image Variables**: Set up shell variables for your image path.
    -   `[GCP_REGION]`: The region of your Artifact Registry repository.
    -   `[PROJECT_ID]`: Your Google Cloud Project ID.
    -   `[IMAGE_TAG]`: A unique tag, like a timestamp (`YYYYMMDD-HHMMSS`) or commit hash.

    ```sh
    export GCP_REGION="us-central1" # Or your region
    export PROJECT_ID=$(gcloud config get-value project)
    export IMAGE_TAG=$(date -u +'%Y%m%d-%H%M%S')
    export IMAGE_PATH="${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/services/collects-services:${IMAGE_TAG}"
    ```

2.  **Build and Push with `buildx`**:
    ```sh
    docker buildx build \
      --platform linux/amd64 \
      --tag "${IMAGE_PATH}" \
      --file services/Dockerfile \
      --push \
      .
    ```
    **Command Breakdown:**
    -   `buildx build`: Use the modern, high-performance builder.
    -   `--platform linux/amd64`: Explicitly build for the architecture used by Google Cloud Run. This is a crucial best practice.
    -   `--tag`: The full image path and tag in Artifact Registry.
    -   `--file`: Specifies the path to the Dockerfile.
    -   `--push`: Pushes the image to the remote repository upon a successful build.
    -   `.`: Sets the build context to the current directory (the project root).

---

## 4. Deployment to Google Cloud Run

Deploy the container image, securely injecting the `DATABASE_URL`.

-   The `IMAGE_PATH` and `GCP_REGION` variables should still be set from the previous step.
-   Replace `[SERVICE_NAME]` (e.g., `collects-prod`) and `[SERVICE_ACCOUNT_EMAIL]`.

```sh
gcloud run deploy [SERVICE_NAME] \
  --image="${IMAGE_PATH}" \
  --region="${GCP_REGION}" \
  --service-account="[SERVICE_ACCOUNT_EMAIL]" \
  --set-env-vars="DATABASE_URL=secret:database-url:latest" \
  --allow-unauthenticated
```
*   `--allow-unauthenticated`: Add this flag for a public service. Omit it for a private one.

---

## 5. Deployment Checks

1.  **Check Service Status**: The command output will provide a **Service URL**. You can also run:
    ```sh
    gcloud run services describe [SERVICE_NAME] --region="${GCP_REGION}"
    ```
    Look for a `status.conditions` where `type: Ready` is `True`.

2.  **Check Logs**: If the service fails, check the logs for errors.
    ```sh
    gcloud run logs tail [SERVICE_NAME] --region="${GCP_REGION}"
    ```

---

## 6. Troubleshooting

### Error: "Image '...' not found" or "Permission denied" during deployment

This error typically points to a configuration or permissions issue with Artifact Registry.

1.  **Verify the Image Path**: Double-check that the `IMAGE_PATH` you are deploying matches the one you pushed exactly. Verify the `[GCP_REGION]`, `[PROJECT_ID]`, repository name (`services`), and tag.

2.  **Check Artifact Registry for the Image**: Confirm the image exists in the repository.
    ```sh
    gcloud artifacts docker images list "${GCP_REGION}-docker.pkg.dev/${PROJECT_ID}/services"
    ```

3.  **Verify Service Account Permissions**: The service account used by Cloud Run needs the **Artifact Registry Reader** role (`roles/artifactregistry.reader`). This is usually granted by default, but if it was changed, you may need to restore it:
    ```sh
    gcloud projects add-iam-policy-binding [PROJECT_ID] \
      --member="serviceAccount:[YOUR_SERVICE_ACCOUNT_EMAIL]" \
      --role="roles/artifactregistry.reader"
    ```

4.  **Local Authentication Error (When Pushing)**: If `docker buildx` fails with a permission error, your local Docker client may not be authenticated. Re-run `gcloud auth configure-docker`.