# Environment Documentation

This document describes the different deployment environments available in the Collects application.

## Overview

The application has two main components:
1. **Services** (Backend API) - Deployed on Google Cloud Run
2. **Worker** (Frontend/Static Assets) - Deployed on Cloudflare Workers

## Environments

### Production Environment

**When deployed:**
- When code is pushed to `main` branch AND the version number in the respective `Cargo.toml` file has changed

**Services:**
- Service Name: `collects-services`
- URL: `https://collects-services-145756646168.us-east1.run.app`
- Database Secret: `database-url`

**Worker:**
- Worker Name: `collects-app`
- URL: `https://collects.lqxclqxc.com`
- KV Namespace ID: `a4dafe7674c2440b81e4ec2e5889f1ba`
- API Base: `https://collects-services-145756646168.us-east1.run.app`

### Internal Environment

**When deployed:**
- When code is pushed to `main` branch AND the version number HAS changed (alongside production deployment)
- This environment uses the production database branch with admin role for migrations and internal operations

**Services:**
- Service Name: `collects-services-internal`
- URL: `https://collects-services-internal-145756646168.us-east1.run.app`
- Database Secret: `database-url-internal`

**Worker:**
- Worker Name: `collects-app-internal`
- URL: `https://collects-internal.lqxclqxc.com`
- KV Namespace ID: `fac40588d16f4fa8b7c8f36de6445649`
- API Base: `https://collects-services-internal-145756646168.us-east1.run.app`

### Test Environment

**When deployed:**
- When code is pushed to `main` branch AND the version number has NOT changed (alongside test-internal deployment)

**Services:**
- Service Name: `collects-services-test`
- URL: `https://collects-services-test-145756646168.us-east1.run.app`
- Database Secret: `database-url-test`

**Worker:**
- Worker Name: `collects-app-test`
- URL: `https://collects-test.lqxclqxc.com`
- KV Namespace ID: `fac40588d16f4fa8b7c8f36de6445649`
- API Base: `https://collects-services-test-145756646168.us-east1.run.app`

### Test-Internal Environment

**When deployed:**
- When code is pushed to `main` branch AND the version number has NOT changed (alongside test deployment)
- This is the internal environment with admin database access for testing/development data

**Services:**
- Service Name: `collects-services-test-internal`
- URL: `https://collects-services-test-internal-145756646168.us-east1.run.app`
- Database Secret: `database-url-test-internal`

**Worker:**
- Worker Name: `collects-app-test-internal`
- URL: `https://collects-test-internal.lqxclqxc.com`
- KV Namespace ID: `fac40588d16f4fa8b7c8f36de6445649`
- API Base: `https://collects-services-test-internal-145756646168.us-east1.run.app`

### Nightly Environment

**When deployed:**
- On a daily schedule (cron: `0 0 * * *` - midnight UTC)
- Used for automated testing and validation

**Services:**
- Service Name: `collects-services-nightly`
- URL: `https://collects-services-nightly-145756646168.us-east1.run.app`
- Database Secret: `database-url` (shares production database)

**Worker:**
- Worker Name: `collects-app-nightly`
- URL: `https://collects-nightly.lqxclqxc.com`
- Configuration: `wrangler.nightly.toml`
- API Base: `https://collects-services-nightly-145756646168.us-east1.run.app`

### PR Environment

**When deployed:**
- On pull request creation or update
- Used for testing proposed changes

**Services:**
- Service Name: `collects-services-pr`
- URL: `https://collects-services-pr-145756646168.us-east1.run.app`
- Database Secret: `database-url-pr`

**Worker:**
- Worker Name: `collects-app-pr`
- URL: `https://collects-pr.lqxclqxc.com`
- Configuration: `wrangler.pr.toml`
- API Base: `https://collects-services-pr-145756646168.us-east1.run.app`

## Deployment Workflows

### Services Deployment (deploy-services.yml)

1. **Version Check**: Compares current version with previous commit
2. **Environment Selection**:
   - `pull_request` → `pr`
   - Version changed → `prod` (then also deploys to `internal`)
   - `push` to main without version change → `test` (then also deploys to `test-internal`)
   - `schedule` → `nightly`
3. **Build & Push**: Builds Docker image and pushes to Artifact Registry
4. **Deploy**: Deploys to Google Cloud Run

### Worker Deployment (deploy.yml)

1. **Version Check**: Compares current version with previous commit
2. **Environment Selection**:
   - `pull_request` → `pr`
   - Version changed → production (empty ENV), then also deploys to `internal`
   - `push` to main without version change → `test`, then also deploys to `test-internal`
   - `schedule` → `nightly`
3. **Build & Deploy**: Builds WASM and deploys to Cloudflare Workers

## Key URLs and Endpoints

| Environment | Service URL | Worker URL |
|-------------|-------------|------------|
| Production | `https://collects-services-145756646168.us-east1.run.app` | `https://collects.lqxclqxc.com` |
| Internal | `https://collects-services-internal-145756646168.us-east1.run.app` | `https://collects-internal.lqxclqxc.com` |
| Test | `https://collects-services-test-145756646168.us-east1.run.app` | `https://collects-test.lqxclqxc.com` |
| Test-Internal | `https://collects-services-test-internal-145756646168.us-east1.run.app` | `https://collects-test-internal.lqxclqxc.com` |
| Nightly | `https://collects-services-nightly-145756646168.us-east1.run.app` | `https://collects-nightly.lqxclqxc.com` |
| PR | `https://collects-services-pr-145756646168.us-east1.run.app` | `https://collects-pr.lqxclqxc.com` |

## OpenAPI Documentation

OpenAPI documentation is **only available** in internal environments (`internal` and `test-internal`) and is protected by Cloudflare Zero Trust authentication.

### Accessing OpenAPI Documentation

| Environment | OpenAPI UI (Scalar) | OpenAPI JSON |
|-------------|---------------------|--------------|
| Internal | `https://collects-services-internal-145756646168.us-east1.run.app/docs` | `https://collects-services-internal-145756646168.us-east1.run.app/openapi.json` |
| Test-Internal | `https://collects-services-test-internal-145756646168.us-east1.run.app/docs` | `https://collects-services-test-internal-145756646168.us-east1.run.app/openapi.json` |

**Note**: You must be authenticated via Cloudflare Access to view the documentation. Contact your team administrator if you don't have access.

## Database Configuration

Each environment (except nightly) uses a separate database:
- `database-url` - Production (also used by nightly)
- `database-url-internal` - Internal (production branch, admin role)
- `database-url-test` - Test (development branch)
- `database-url-test-internal` - Test-Internal (development branch, admin role)
- `database-url-pr` - PR
- `database-url-local` - Local development

## JWT Secret Configuration

JWT secrets are used for session token signing. The following secrets must be created in Google Cloud Secret Manager:
- `jwt-secret` - Used by Production, Internal, and Nightly environments
- `jwt-secret-pr` - Used by PR environment

Local, Test, and Test-Internal environments use a default local secret and do not require a GCP secret.

## Zero Trust Secret Configuration

Zero Trust secrets are used for Cloudflare Access authentication on internal routes. The following secrets must be created in Google Cloud Secret Manager:
- `cf-access-team-domain` - Your Cloudflare Access team domain (e.g., `myteam.cloudflareaccess.com`)
- `cf-access-aud` - Your Cloudflare Access application audience tag

**Required for:** Internal and Test-Internal environments (service will fail to start without these secrets).

### Automated Setup (Recommended)

Run the following command to create and configure Zero Trust secrets:
```bash
just scripts::zero-trust-setup --project-id <your-gcp-project-id>
```

### Getting Cloudflare Access Credentials

1. Go to [Cloudflare Zero Trust dashboard](https://one.dash.cloudflare.com)
2. Navigate to **Access > Applications**
3. Create or select a **Self-hosted Application**
4. Note the **Team Domain** (shown at the top) and **Application Audience (AUD)** from application settings

For more details, see [ZERO_TRUST.md](./services/ZERO_TRUST.md).

### Automated Setup (Recommended)

Run the following command to automatically generate and create JWT secrets:
```bash
just scripts::jwt-setup --project-id <your-gcp-project-id>
```

This will:
1. Create the secrets in Google Cloud Secret Manager if they don't exist
2. Generate cryptographically secure random secrets (32 bytes, base64 encoded)
3. Update the secret values

To check the status of JWT secrets:
```bash
just scripts::jwt-list --project-id <your-gcp-project-id>
```

### Manual Setup

If you prefer to create secrets manually:
```bash
# Generate and create jwt-secret for production/internal/nightly
openssl rand -base64 32 | gcloud secrets create jwt-secret --data-file=-

# Generate and create jwt-secret-pr for PR environment
openssl rand -base64 32 | gcloud secrets create jwt-secret-pr --data-file=-
```

After creating the secrets, run `just scripts::actions-setup` to grant the Compute Service Account access to these secrets.

## Access and Security

For information about authentication and access control, see [ZERO_TRUST.md](./services/ZERO_TRUST.md).
