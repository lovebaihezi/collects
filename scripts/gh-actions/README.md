# GitHub Actions Scripts

This directory contains TypeScript utilities used by GitHub Actions workflows.

## Structure

- `version-check.ts` - Version change detection for Cargo.toml files

## Usage

### Version Check

The `version-check.ts` script can be used to detect version changes in Cargo.toml files:

```bash
bun run version-check.ts path/to/Cargo.toml
```

This script is used by the reusable `check-version` action located at `.github/actions/check-version/`.

## Type Checking

To run TypeScript type checking:

```bash
npm install
npm run typecheck
```

## Integration with Workflows

The main integration point is through the composite action at `.github/actions/check-version/action.yml`, which is used by:

- `.github/workflows/native-release.yml` - For UI version checking
- `.github/workflows/deploy.yml` - For UI version checking
- `.github/workflows/deploy-services.yml` - For services version checking
