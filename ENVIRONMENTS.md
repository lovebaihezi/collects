# Environment Documentation

This document describes the different deployment environments available in the Collects application.

## Overview

The application has two main components:
1. **Services** (Backend API) - Deployed on Google Cloud Run
2. **Worker** (Frontend/Static Assets) - Deployed on Cloudflare Workers
3. **WASM Storage** (PR WASM files) - Stored in Cloudflare R2

## Cloudflare R2 Storage

For PR environments, WASM files are uploaded to Cloudflare R2 with version-specific paths. This allows:
- Each PR to have its own isolated WASM bundle
- Automatic cleanup when PRs are closed
- Version tracking for debugging and rollback

**R2 Configuration:**
- Bucket Name: `collects-wasm`
- Path Format: `pr-{pr_number}/{filename}`
- Example: `pr-123/collects-ui-abc123.wasm`

**Required Secrets:**
- `CLOUDFLARE_API_TOKEN` - API token with R2 read/write permissions
- `CLOUDFLARE_ACCOUNT_ID` - Cloudflare account ID

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
- WASM files uploaded to R2 with PR-specific paths

**Services:**
- Service Name: `collects-services-pr`
- URL: `https://collects-services-pr-145756646168.us-east1.run.app`
- Database Secret: `database-url-pr`

**Worker:**
- Worker Name: `collects-app-pr`
- URL: `https://collects-pr.lqxclqxc.com`
- Configuration: `wrangler.pr.toml`
- API Base: `https://collects-services-pr-145756646168.us-east1.run.app`
- R2 Bucket: `collects-wasm`
- R2 Path: `pr-{pr_number}/`

**Cleanup:**
- When a PR is closed (merged or abandoned), the cleanup workflow deletes:
  - R2 WASM files for the PR
  - Docker image for the PR

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

## Database Configuration

Each environment (except nightly) uses a separate database:
- `database-url` - Production (also used by nightly)
- `database-url-internal` - Internal (production branch, admin role)
- `database-url-test` - Test (development branch)
- `database-url-test-internal` - Test-Internal (development branch, admin role)
- `database-url-pr` - PR
- `database-url-local` - Local development

## Access and Security

For information about authentication and access control, see [ZERO_TRUST.md](./services/ZERO_TRUST.md).
