# sccache with Cloudflare R2 Setup Guide

This document explains how sccache is configured to use Cloudflare R2 as a remote cache backend for Rust compilation in GitHub Actions.

## Overview

sccache is a compiler caching tool that stores compilation results in a remote storage backend (Cloudflare R2). When the same code is compiled again, sccache retrieves the cached result instead of recompiling, significantly reducing build times.

### Benefits

- **Faster CI builds**: Cache hits avoid recompilation
- **Cross-job caching**: All CI jobs share the same cache
- **Cross-PR caching**: PRs benefit from cache built by main branch
- **Reduced disk space**: Artifacts stored remotely instead of locally

## Required GitHub Secrets

The following secrets must be configured in your GitHub repository settings:

| Secret Name | Description |
|-------------|-------------|
| `CLOUDFLARE_ACCOUNT_ID` | Your Cloudflare account ID |
| `SCCACHE_R2_BUCKET` | Name of the R2 bucket for sccache (e.g., `collects-sccache`) |
| `SCCACHE_R2_ACCESS_KEY_ID` | R2 API token access key ID |
| `SCCACHE_R2_SECRET_ACCESS_KEY` | R2 API token secret access key |

### Creating R2 Credentials

1. Go to Cloudflare Dashboard → R2 → Manage R2 API Tokens
2. Create a new API token with:
   - **Permissions**: Object Read & Write
   - **Scope**: Specific bucket (your sccache bucket)
3. Save the Access Key ID and Secret Access Key
4. Add them as GitHub repository secrets

## R2 Bucket Lifecycle Management

To control cache size and object lifetime, configure lifecycle rules in Cloudflare R2:

### Setting Up Lifecycle Rules

1. Go to Cloudflare Dashboard → R2 → Your Bucket → Settings → Lifecycle Rules
2. Add a rule to expire objects after N days:

```json
{
  "rules": [
    {
      "id": "expire-old-cache",
      "status": "Enabled",
      "filter": {
        "prefix": ""
      },
      "expiration": {
        "days": 30
      }
    }
  ]
}
```

### Recommended Lifecycle Configuration

| Use Case | TTL (Days) | Reason |
|----------|------------|--------|
| Active development | 14-30 | Balances cache freshness with storage costs |
| Infrequent releases | 60-90 | Longer retention for stable codebases |
| Storage conscious | 7 | Minimizes R2 storage usage |

### Cache Key Prefixes

Different build targets use different cache prefixes to avoid collisions:

| Prefix | Used By |
|--------|---------|
| (default) | Linux x86_64 native builds |
| `linux-x86_64` | Native Linux release builds |
| `windows-x86_64` | Native Windows release builds |
| `macos-aarch64` | Native macOS ARM release builds |
| `wasm32` | WASM builds |
| `linux-musl` | Services Docker builds (musl target) |

You can create separate lifecycle rules for each prefix if needed.

## sccache Statistics

After each build, the CI shows sccache statistics:

```
=== sccache Statistics ===
Compile requests                    100
Cache hits                          80
Cache misses                        20
Cache hit rate                      80%
Cache bytes read                120 MB
Cache bytes written              30 MB
```

### Interpreting Statistics

- **Cache Hit Rate**: Percentage of compilations served from cache
  - First build: ~0% (cold cache)
  - Subsequent builds: 70-95% (warm cache)
  - After dependency updates: Lower temporarily
  
- **Compile Requests**: Total compilation units processed
- **Cache Hits**: Compilations retrieved from R2
- **Cache Misses**: Compilations that had to be performed

## Troubleshooting

### Low Cache Hit Rate

1. **Different toolchain versions**: Cache keys include compiler version
2. **Feature flag changes**: Different feature combinations create different cache entries
3. **Dependency updates**: New dependencies aren't in cache yet

### Cache Not Working

1. Verify R2 credentials are correct
2. Check R2 bucket exists and is accessible
3. Ensure `RUSTC_WRAPPER=sccache` is set
4. Check sccache version compatibility

### Storage Costs

R2 pricing (as of 2024):
- Storage: $0.015/GB/month
- Class A operations (writes): $4.50/million
- Class B operations (reads): $0.36/million
- Egress: Free

Typical sccache storage for this project: 5-20 GB

## Workflow Integration

sccache is integrated into the following workflows:

- `ci.yml`: All lint, test, and build jobs
- `native-release.yml`: Multi-platform release builds
- `deploy.yml`: WASM worker deployments
- `deploy-services.yml`: Services Docker builds

The configuration is centralized in workflow-level environment variables:

```yaml
env:
  SCCACHE_BUCKET: ${{ secrets.SCCACHE_R2_BUCKET }}
  SCCACHE_ENDPOINT: https://${{ secrets.CLOUDFLARE_ACCOUNT_ID }}.r2.cloudflarestorage.com
  SCCACHE_REGION: auto
  AWS_ACCESS_KEY_ID: ${{ secrets.SCCACHE_R2_ACCESS_KEY_ID }}
  AWS_SECRET_ACCESS_KEY: ${{ secrets.SCCACHE_R2_SECRET_ACCESS_KEY }}
  RUSTC_WRAPPER: sccache
```

## Related Actions

- `.github/actions/sccache-stats/`: Displays sccache statistics after builds
- `.github/actions/setup-sccache/`: (Optional) Composite action for custom setups

## References

- [sccache GitHub](https://github.com/mozilla/sccache)
- [sccache Configuration](https://github.com/mozilla/sccache/blob/main/docs/Configuration.md)
- [Cloudflare R2 Lifecycle Rules](https://developers.cloudflare.com/r2/buckets/object-lifecycles/)
- [mozilla-actions/sccache-action](https://github.com/Mozilla-Actions/sccache-action)
