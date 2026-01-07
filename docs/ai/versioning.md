# Versioning rules (`{env}:{info}`)

This repo uses a single version display format in both UI and services:

- **Format:** `{env}:{info}`

`env` is a short environment name; `info` is the environment-specific identifier (PR number, commit, date, or semantic version).

---

## Required environment mappings

Use these exact mappings:

| Environment | Format | Example |
|------------|--------|---------|
| PR | `pr:{number}` | `pr:123` |
| Nightly | `nightly:{date}` | `nightly:2026-01-03` |
| Internal | `internal:{commit}` | `internal:abc1234` |
| Test-Internal | `test-internal:{commit}` | `test-internal:abc1234` |
| Test/Main | `main:{commit}` | `main:abc1234` |
| Production | `stable:{version}` | `stable:2026.1.2` |

**Notes**
- `{commit}` is the **short** git hash.
- `{date}` should be **YYYY-MM-DD**.
- `{version}` comes from the package version (UI: `Cargo.toml`).

---

## Where the version string is produced

### UI
- Must use: `collects_business::version_info::format_env_version()`
- UI display convention: `UI: {env}:{info}`

### Services
- Must use: `format_version_header()` in `services/src/lib.rs`
- Services send version via a header: `x-service-version: {env}:{info}`

---

## Build-time inputs (do not hardcode)

Both UI and services rely on build-time environment variables:

- `BUILD_COMMIT` — short git commit hash
- `BUILD_DATE` — build timestamp (RFC3339)
- `CARGO_PKG_VERSION` — package version from Cargo metadata
- `PR_NUMBER` — PR number (PR builds only)
- `SERVICE_ENV` — environment name (services only)

---

## Rules for changes

- Do not invent new formatting. If a new environment is introduced, extend the table above and update the shared formatting function(s).
- Keep output stable and machine-readable (`{env}:{info}` only; no extra words inside the value).
- Prefer generating `{env}:{info}` in one place and reusing it everywhere (no duplicate logic).

---