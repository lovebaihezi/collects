# Docker Image Cleanup

This document describes how Docker images are managed and cleaned up in the Collects project.

## Overview

Docker images for the `collects-services` backend are stored in Google Cloud Artifact Registry. To manage storage costs and keep the registry clean, images are automatically deleted based on retention policies.

## Image Types and Tags

| Image Type | Tag Pattern | Example | Created When |
|------------|-------------|---------|--------------|
| **PR builds** | `pr-<number>` | `pr-123` | Pull request CI runs |
| **Nightly builds** | `nightly-YYYYMMDD` | `nightly-20260105` | Daily schedule (midnight UTC) |
| **Main branch builds** | `main-<sha>` | `main-abc1234` | Push to main without version change |
| **Production releases** | `v<version>` | `v2026.1.5` | Push to main with version change |

## Cleanup Mechanisms

### 1. PR Image Cleanup (Immediate)

**Workflow:** `.github/workflows/cleanup-pr.yml`

**Trigger:** When a PR is closed (merged or abandoned)

**Action:** Immediately deletes the Docker image tagged `pr-<PR_NUMBER>`

**Example:**
- PR #123 is closed
- Docker image `pr-123` is deleted from Artifact Registry

### 2. Scheduled Cleanup (Daily)

**Workflow:** `.github/workflows/artifact-cleanup.yml`

**Trigger:** Daily at 2:00 AM UTC (can also be triggered manually)

**Retention Policies:**

| Image Type | Retention Period | Notes |
|------------|-----------------|-------|
| Nightly builds | 7 days | `nightly-YYYYMMDD` tags |
| Main branch builds | 1 day | `main-<sha>` tags |
| Production releases | 30 days | `v<version>` tags |
| PR builds | On PR close | Handled by `cleanup-pr.yml` |

## Verifying Cleanup

To verify that cleanup is working correctly, you can use the `artifact-check` command:

```bash
# List all current images and check cleanup compliance
just scripts::artifact-check

# Dry run - see what would be cleaned up
DRY_RUN=true just scripts::artifact-cleanup
```

### What the Check Reports

The `artifact-check` command will show:

1. **Total images** in the registry
2. **Images by category**:
   - PR images (should be empty if all PRs are closed)
   - Nightly images (how many, oldest age)
   - Main branch images (how many, oldest age)
   - Production releases (how many, oldest age)
3. **Cleanup compliance**:
   - ✅ Images within retention policy
   - ⚠️ Images that should have been deleted
   - ❌ Orphaned PR images (PR closed but image still exists)

## Manual Cleanup

If you need to manually clean up images:

```bash
# Preview what would be deleted
DRY_RUN=true just scripts::artifact-cleanup

# Actually delete old images
just scripts::artifact-cleanup
```

## Configuration

The cleanup scripts use the following defaults:

| Setting | Default Value |
|---------|---------------|
| GCP Region | `us-east1` |
| Repository | `collects-services` |
| Image Name | `collects-services` |

Override with environment variables:
- `GCP_PROJECT_ID`
- `GCP_REGION`
- `GCP_REPOSITORY`
- `GCP_IMAGE_NAME`

## Troubleshooting

### PR image not cleaned up

1. Check if `cleanup-pr.yml` workflow ran when the PR was closed
2. View workflow run logs for errors
3. Manually delete: `gcloud artifacts docker images delete <image_path>:pr-<number> --delete-tags`

### Scheduled cleanup not running

1. Check workflow runs in GitHub Actions
2. Verify Google Cloud authentication is working
3. Run manually: `just scripts::artifact-cleanup`

### Image still exists after retention period

1. Run `just scripts::artifact-check` to see image status
2. Trigger manual cleanup: `just scripts::artifact-cleanup`
3. Check `artifact-cleanup.yml` workflow logs for errors
