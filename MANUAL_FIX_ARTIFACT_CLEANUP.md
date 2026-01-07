# Manual Fix: Grant Artifact Registry Delete Permission

## Issue

The scheduled artifact cleanup job is failing with permission errors:

```
ERROR: (gcloud.artifacts.docker.images.delete) PERMISSION_DENIED: 
Permission 'artifactregistry.tags.delete' denied on resource
```

**Related Issue:** #169

## Root Cause

The service account `github-actions-sa@braided-case-416903.iam.gserviceaccount.com` currently has the `roles/artifactregistry.writer` role, which allows:
- ✅ Uploading Docker images
- ✅ Tagging images
- ❌ **Deleting images** (missing permission)

The artifact cleanup workflow needs to delete old images based on retention policies, which requires the `artifactregistry.tags.delete` permission.

## Solution

Grant the `roles/artifactregistry.repoAdmin` role to the service account. This role includes:
- All permissions from `artifactregistry.writer` (upload, tag)
- Additional delete permissions (`artifactregistry.tags.delete`, `artifactregistry.versions.delete`)

## How to Apply the Fix

### Option 1: Using gcloud CLI (Recommended)

Run this command to grant the required role:

```bash
gcloud projects add-iam-policy-binding braided-case-416903 \
  --member="serviceAccount:github-actions-sa@braided-case-416903.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.repoAdmin" \
  --condition=None
```

Expected output:
```
Updated IAM policy for project [braided-case-416903].
bindings:
- members:
  - serviceAccount:github-actions-sa@braided-case-416903.iam.gserviceaccount.com
  role: roles/artifactregistry.repoAdmin
...
```

### Option 2: Using Google Cloud Console

1. Go to [Google Cloud Console - IAM](https://console.cloud.google.com/iam-admin/iam?project=braided-case-416903)
2. Find the service account: `github-actions-sa@braided-case-416903.iam.gserviceaccount.com`
3. Click the pencil icon (Edit principal)
4. Click "ADD ANOTHER ROLE"
5. Select "Artifact Registry Repository Administrator" (`roles/artifactregistry.repoAdmin`)
6. Click "SAVE"

### Option 3: Remove old role and grant new role

If you want to remove the old `artifactregistry.writer` role first:

```bash
# Remove the old writer role
gcloud projects remove-iam-policy-binding braided-case-416903 \
  --member="serviceAccount:github-actions-sa@braided-case-416903.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.writer"

# Grant the new repoAdmin role
gcloud projects add-iam-policy-binding braided-case-416903 \
  --member="serviceAccount:github-actions-sa@braided-case-416903.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.repoAdmin" \
  --condition=None
```

## Verification

After applying the fix, verify the permissions:

```bash
# Check the service account's roles
gcloud projects get-iam-policy braided-case-416903 \
  --flatten="bindings[].members" \
  --filter="bindings.members:github-actions-sa@braided-case-416903.iam.gserviceaccount.com" \
  --format="table(bindings.role)"
```

You should see `roles/artifactregistry.repoAdmin` in the output.

## Testing the Fix

1. **Manual test**: Try running the cleanup job manually:
   ```bash
   # Trigger the workflow manually
   gh workflow run artifact-cleanup.yml
   ```

2. **Wait for scheduled run**: The cleanup job runs daily at 2:00 AM UTC. Check the next scheduled run.

3. **Check workflow status**: 
   - Go to [Actions tab](https://github.com/lqxc-org/collects/actions/workflows/artifact-cleanup.yml)
   - Verify the next run completes successfully

## Future Prevention

The setup script (`scripts/services/gcloud.ts`) has been updated to use `roles/artifactregistry.repoAdmin` instead of `roles/artifactregistry.writer` for new installations. This manual fix is only needed for the existing service account.

## References

- [Google Cloud Artifact Registry IAM roles](https://cloud.google.com/artifact-registry/docs/access-control)
- [Artifact Registry Repository Administrator role](https://cloud.google.com/iam/docs/understanding-roles#artifactregistry.repoAdmin)
