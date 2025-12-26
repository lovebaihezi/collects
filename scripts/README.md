# Scripts

This directory contains utilities and tools for managing the Collects project infrastructure and GitHub Actions workflows.

## Structure

```
scripts/
├── main.ts                 # Main CLI entry point
├── package.json            # Shared dependencies
├── tsconfig.json           # TypeScript configuration
├── .oxlintrc.json         # Linter configuration
├── bun.lock               # Bun lockfile
├── services/              # Google Cloud & database management
│   ├── gh-action.ts       # GitHub Actions + Workload Identity setup
│   ├── neon.ts            # Neon database initialization
│   └── utils.ts           # Utility functions
└── gh-actions/            # GitHub Actions utilities
    └── version-check.ts   # Version change detection
```

## Prerequisites

- [Bun](https://bun.sh/) runtime (recommended)
- OR Node.js with npm

## Installation

```bash
cd scripts
bun install
# OR: npm install
```

## Usage

Run the CLI with:

```bash
bun run main.ts <command>
# OR: node main.ts <command>
```

## Commands

### `actions-setup`

Sets up Workload Identity Federation for GitHub Actions to deploy to Google Cloud.

```bash
bun run main.ts actions-setup
bun run main.ts actions-setup --project-id my-project --repo owner/repo
```

### `init-db-secret`

Initializes Neon Database branches and updates Google Cloud Secrets.

```bash
bun run main.ts init-db-secret --token NEON_TOKEN --project-id NEON_PROJECT_ID
```

### `version-check`

Checks if version in a Cargo.toml file has changed (used by GitHub Actions).

```bash
bun run main.ts version-check ui/Cargo.toml
bun run main.ts version-check services/Cargo.toml
```

## Type Checking

Run TypeScript type checking:

```bash
npm run typecheck
```

## GitHub Actions Integration

The `version-check` function is used by the composite action at `.github/actions/check-version/` which is consumed by:

- `.github/workflows/native-release.yml` (UI version checking)
- `.github/workflows/deploy.yml` (UI version checking)
- `.github/workflows/deploy-services.yml` (Services version checking)
