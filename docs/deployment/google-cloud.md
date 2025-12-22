# Google Cloud Deployment Setup

This document outlines the configuration required to deploy the `collects-services` application to Google Cloud Run using GitHub Actions.

## Prerequisites

1.  **Google Cloud Project**: A valid GCP project.
2.  **Artifact Registry**: A Docker repository named `collects-services` in `us-east1`.
3.  **Cloud Run**: API enabled.
4.  **Secret Manager**: API enabled.

## Authentication (Workload Identity Federation)

The GitHub Actions workflow uses [Workload Identity Federation](https://github.com/google-github-actions/auth#preferred-workload-identity-federation) to authenticate with Google Cloud without long-lived service account keys.

### 1. Create a Service Account

Create a service account that GitHub Actions will impersonate.

```bash
gcloud iam service-accounts create github-actions-deployer \
  --display-name="GitHub Actions Deployer"
```

### 2. Configure Workload Identity Pool and Provider

```bash
# Create the pool
gcloud iam workload-identity-pools create github-actions-pool \
  --location="global" \
  --display-name="GitHub Actions Pool"

# Get the Pool ID (needed for the next step)
POOL_ID=$(gcloud iam workload-identity-pools describe github-actions-pool \
  --location="global" \
  --format="value(name)")

# Create the provider
gcloud iam workload-identity-pools providers create-oidc github-actions-provider \
  --location="global" \
  --workload-identity-pool="github-actions-pool" \
  --display-name="GitHub Actions Provider" \
  --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" \
  --issuer-uri="https://token.actions.githubusercontent.com"
```

### 3. Allow GitHub Actions to Impersonate the Service Account

Replace `OWNER/REPO` with your GitHub repository path (e.g., `user/collects`).

```bash
# Allow the repository to impersonate the service account
gcloud iam service-accounts add-iam-policy-binding "github-actions-deployer@$PROJECT_ID.iam.gserviceaccount.com" \
  --project="$PROJECT_ID" \
  --role="roles/iam.workloadIdentityUser" \
  --member="principalSet://iam.googleapis.com/${POOL_ID}/attribute.repository/OWNER/REPO"
```

### 4. Grant Permissions to the Service Account

The service account needs permissions to:
- Push to Artifact Registry (`roles/artifactregistry.writer`)
- Deploy to Cloud Run (`roles/run.admin`)
- Act as Service Account for Cloud Run (`roles/iam.serviceAccountUser`)

```bash
SA_EMAIL="github-actions-deployer@$PROJECT_ID.iam.gserviceaccount.com"

gcloud projects add-iam-policy-binding "$PROJECT_ID" --member="serviceAccount:$SA_EMAIL" --role="roles/artifactregistry.writer"
gcloud projects add-iam-policy-binding "$PROJECT_ID" --member="serviceAccount:$SA_EMAIL" --role="roles/run.admin"
gcloud projects add-iam-policy-binding "$PROJECT_ID" --member="serviceAccount:$SA_EMAIL" --role="roles/iam.serviceAccountUser"
```

## Database & Neon Branching Strategy

The deployment pipeline is designed to work seamlessly with [Neon](https://neon.tech) database branching. By mapping Google Cloud Secrets to specific environments, you can easily point each deployment environment to the corresponding Neon branch.

### Environment Mapping

| Environment | GitHub Trigger | Cloud Run Service | Secret Name | Recommended Neon Branch |
|---|---|---|---|---|
| **Internal** | Pull Request | `collects-services-internal` | `database-url-internal` | `preview` |
| **Test** | Push to `main` | `collects-services-test` | `database-url-test` | `preview` (or `staging`) |
| **Nightly** | Schedule | `collects-services-nightly` | `database-url-nightly` | `nightly` |
| **Production** | Tag (`v*`) | `collects-services` | `database-url` | `prod` |

### Configuration

To "migrate" or switch databases (e.g., pointing `internal` to a new Neon branch):

1.  **Get the Connection String** from your Neon dashboard for the desired branch.
2.  **Update the Secret** in Google Secret Manager.

```bash
# Example: Pointing the internal environment to the 'preview' branch
echo -n "postgres://user:pass@ep-xyz.us-east-1.aws.neon.tech/neondb" | \
  gcloud secrets versions add database-url-internal --data-file=-
```

The next deployment (or a new PR) will automatically pick up the new connection string. No code changes are required.

## GitHub Secrets

Add the following secrets to your GitHub Repository:

- `GCP_WORKLOAD_IDENTITY_PROVIDER`: The full resource name of the provider.
- `GCP_SERVICE_ACCOUNT`: The email of the service account created in Step 1.
