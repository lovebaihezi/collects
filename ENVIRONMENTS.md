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
- URL: `https://collects-api-145756646168.us-east1.run.app`
- Database Secret: `database-url`

**Worker:**
- Worker Name: `collects-app`
- URL: `https://collects.lqxclqxc.com`
- KV Namespace ID: `a4dafe7674c2440b81e4ec2e5889f1ba`
- API Base: `https://collects-api-145756646168.us-east1.run.app`

### Internal Environment

**When deployed:**
- When code is pushed to `main` branch AND the version number has NOT changed
- This is the continuous integration environment for testing changes before a production release

**Services:**
- Service Name: `collects-services-internal`
- URL: `https://collects-api-internal-145756646168.us-east1.run.app`
- Database Secret: `database-url-internal`

**Worker:**
- Worker Name: `collects-app-internal`
- URL: `https://collects-internal.lqxclqxc.com`
- KV Namespace ID: `fac40588d16f4fa8b7c8f36de6445649`
- API Base: `https://collects-api-internal-145756646168.us-east1.run.app`

### Test Environment

**When deployed:**
- When code is pushed to `main` branch AND the version number has NOT changed (alongside internal deployment)

**Services:**
- Service Name: `collects-services-test`
- URL: `https://collects-api-test-145756646168.us-east1.run.app`
- Database Secret: `database-url-test`

**Worker:**
- Worker Name: `collects-app-test`
- URL: `https://collects-test.lqxclqxc.com`
- KV Namespace ID: `fac40588d16f4fa8b7c8f36de6445649`
- API Base: `https://collects-api-test-145756646168.us-east1.run.app`

### Nightly Environment

**When deployed:**
- On a daily schedule (cron: `0 0 * * *` - midnight UTC)
- Used for automated testing and validation

**Services:**
- Service Name: `collects-services-nightly`
- URL: `https://collects-api-nightly-145756646168.us-east1.run.app`
- Database Secret: `database-url` (shares production database)

**Worker:**
- Worker Name: `collects-app-nightly`
- URL: `https://collects-nightly.lqxclqxc.com`
- Configuration: `wrangler.nightly.toml`

### PR Environment

**When deployed:**
- On pull request creation or update
- Used for testing proposed changes

**Services:**
- Service Name: `collects-services-pr`
- URL: `https://collects-api-pr-145756646168.us-east1.run.app`
- Database Secret: `database-url-pr`

**Worker:**
- Worker Name: `collects-app-pr`
- Configuration: `wrangler.pr.toml`

## Deployment Workflows

### Services Deployment (deploy-services.yml)

1. **Version Check**: Compares current version with previous commit
2. **Environment Selection**:
   - `pull_request` → `pr`
   - Version changed → `prod` (with version tag)
   - `push` to main without version change → `test` (then also deploys to `internal`)
   - `schedule` → `nightly`
3. **Build & Push**: Builds Docker image and pushes to Artifact Registry
4. **Deploy**: Deploys to Google Cloud Run

### Worker Deployment (deploy.yml)

1. **Version Check**: Compares current version with previous commit
2. **Environment Selection**:
   - `pull_request` → `pr`
   - Version changed → production (empty ENV)
   - `push` to main without version change → `test` (then also deploys to `internal`)
   - `schedule` → `nightly`
3. **Build & Deploy**: Builds WASM and deploys to Cloudflare Workers

## Key URLs and Endpoints

| Environment | Service URL | Worker URL |
|-------------|-------------|------------|
| Production | `https://collects-api-145756646168.us-east1.run.app` | `https://collects.lqxclqxc.com` |
| Internal | `https://collects-api-internal-145756646168.us-east1.run.app` | `https://collects-internal.lqxclqxc.com` |
| Test | `https://collects-api-test-145756646168.us-east1.run.app` | `https://collects-test.lqxclqxc.com` |
| Nightly | `https://collects-api-nightly-145756646168.us-east1.run.app` | `https://collects-nightly.lqxclqxc.com` |

## Database Configuration

Each environment (except nightly) uses a separate database:
- `database-url` - Production (also used by nightly)
- `database-url-internal` - Internal
- `database-url-test` - Test
- `database-url-pr` - PR
- `database-url-local` - Local development

## Access and Security

For information about authentication and access control, see [ZERO_TRUST.md](./services/ZERO_TRUST.md).
