#!/usr/bin/env bun
import { cac } from "cac";
import * as p from "@clack/prompts";
import { initDbSecret } from "./services/neon.ts";
import {
  buildSetupContext,
  setupGitHubActions,
  type BuildSetupOptions,
  buildMigrateRepoContext,
  migrateRepoBindings,
  type BuildMigrateRepoOptions,
} from "./services/gcloud.ts";
import { runVersionCheck } from "./gh-actions/version-check.ts";
import {
  runCIFeedbackCLI,
  runPostJobFeedbackCLI,
} from "./gh-actions/ci-feedback.ts";
import { runScheduledJobIssueCLI } from "./gh-actions/scheduled-job-issue.ts";
import { runArtifactCleanupCLI } from "./gh-actions/artifact-cleanup.ts";
import { runArtifactCheckCLI } from "./gh-actions/artifact-check.ts";
import {
  getCargoFeature,
  getDatabaseSecret,
  getJwtSecret,
  listEnvironments,
} from "./services/env-config.ts";
import { runPrTitleCheck } from "./services/pr-title.ts";
import {
  listR2Secrets,
  promptForR2Credentials,
  setupR2Secrets,
} from "./services/r2-setup.ts";

const cli = cac("services");

cli
  .command(
    "actions-setup",
    "Setup GitHub Actions with Google Cloud Workload Identity Federation",
  )
  .option("--project-id <projectId>", "Google Cloud Project ID")
  .option("--repo <repo>", "GitHub Repository (owner/repo)")
  .action(async (options) => {
    p.intro("GitHub Actions + Google Cloud Workload Identity Federation Setup");

    // CAC converts `--project-id` into `options.projectId` (camelCase),
    // but our `buildSetupContext` expects `projectId`/`repo`.
    const ctx = await buildSetupContext({
      projectId: options.projectId,
      repo: options.repo,
    } as BuildSetupOptions);
    await setupGitHubActions(ctx);
  });

cli
  .command(
    "actions-migrate-repo",
    "Migrate workload identity bindings when repository is moved to a new org/name",
  )
  .option("--project-id <projectId>", "Google Cloud Project ID")
  .option("--old-repo <oldRepo>", "Old GitHub Repository (owner/repo)")
  .option("--new-repo <newRepo>", "New GitHub Repository (owner/repo)")
  .action(async (options) => {
    p.intro("GitHub Actions Repository Migration");

    const ctx = await buildMigrateRepoContext({
      projectId: options.projectId,
      oldRepo: options.oldRepo,
      newRepo: options.newRepo,
    } as BuildMigrateRepoOptions);
    await migrateRepoBindings(ctx);
  });

cli
  .command(
    "init-db-secret",
    "Initialize Neon Database branches and update Secrets",
  )
  .option("--token <token>", "Neon API Token")
  .option("--project-id <projectId>", "Neon Project ID")
  .action(async (options) => {
    p.intro("Neon Database Secret Setup");

    // Prompt for token if not provided
    const token = options.token
      ? options.token
      : await p.text({
          message: "Enter your Neon API Token:",
          placeholder: "neon_api_xxxxx",
          validate: (value) => {
            if (!value) return "Neon API Token is required";
          },
        });

    if (p.isCancel(token)) {
      p.cancel("Operation cancelled.");
      process.exit(0);
    }

    // Prompt for project ID if not provided
    const projectId = options.projectId
      ? options.projectId
      : await p.text({
          message: "Enter your Neon Project ID:",
          placeholder: "project-id-xxxx",
          validate: (value) => {
            if (!value) return "Neon Project ID is required";
          },
        });

    if (p.isCancel(projectId)) {
      p.cancel("Operation cancelled.");
      process.exit(0);
    }

    await initDbSecret(token as string, projectId as string);
  });

cli
  .command("version-check <path>", "Check if version in Cargo.toml has changed")
  .action((path: string) => {
    runVersionCheck(path);
  });

cli
  .command(
    "env-feature [env]",
    "Get cargo feature flags for an environment (used by justfiles)",
  )
  .action((env: string = "") => {
    // Output only the feature flags, suitable for command substitution
    // Empty env means production/default (no feature flag)
    if (!env) {
      console.log("");
      return;
    }
    console.log(getCargoFeature(env));
  });

cli
  .command(
    "env-secret <env>",
    "Get database secret name for an environment (used by justfiles)",
  )
  .action((env: string) => {
    // Output only the secret name, suitable for command substitution
    console.log(getDatabaseSecret(env));
  });

cli
  .command(
    "jwt-secret <env>",
    "Get JWT secret name for an environment (used by justfiles)",
  )
  .action((env: string) => {
    // Output only the secret name, suitable for command substitution
    // Returns empty string if the environment uses default local secret
    console.log(getJwtSecret(env));
  });

cli.command("env-list", "List all available environment names").action(() => {
  console.log(listEnvironments().join("\n"));
});

cli
  .command(
    "check-pr-title <title>",
    "Validate PR title format (conventional commits)",
  )
  .action((title: string) => {
    runPrTitleCheck(title);
  });

cli
  .command(
    "r2-setup",
    "Setup Cloudflare R2 secrets in Google Cloud Secret Manager",
  )
  .option("--project-id <projectId>", "Google Cloud Project ID")
  .action(async (options) => {
    p.intro("Cloudflare R2 Storage Setup");

    // Prompt for project ID if not provided
    const projectId = options.projectId
      ? options.projectId
      : await p.text({
          message: "Enter your Google Cloud Project ID:",
          placeholder: "my-gcp-project-id",
          validate: (value) => {
            if (!value) return "Project ID is required";
          },
        });

    if (p.isCancel(projectId)) {
      p.cancel("Operation cancelled.");
      process.exit(0);
    }

    const credentials = await promptForR2Credentials();
    if (!credentials) {
      process.exit(0);
    }

    await setupR2Secrets(projectId as string, credentials);
    p.outro("R2 setup complete!");
  });

cli
  .command("r2-list", "List Cloudflare R2 secrets status")
  .option("--project-id <projectId>", "Google Cloud Project ID")
  .action(async (options) => {
    p.intro("Cloudflare R2 Secrets Status");

    const projectId = options.projectId
      ? options.projectId
      : await p.text({
          message: "Enter your Google Cloud Project ID:",
          placeholder: "my-gcp-project-id",
          validate: (value) => {
            if (!value) return "Project ID is required";
          },
        });

    if (p.isCancel(projectId)) {
      p.cancel("Operation cancelled.");
      process.exit(0);
    }

    await listR2Secrets(projectId as string);
  });
cli
  .command("ci-feedback", "Post CI failure feedback to PR (for GitHub Actions)")
  .action(() => {
    runCIFeedbackCLI();
  });

cli
  .command(
    "ci-feedback-post-job",
    "Post CI failure feedback from within a job (post-job approach)",
  )
  .action(() => {
    runPostJobFeedbackCLI();
  });

cli
  .command(
    "scheduled-job-issue",
    "Create GitHub issue when scheduled job fails (for GitHub Actions)",
  )
  .action(() => {
    runScheduledJobIssueCLI();
  });

cli
  .command(
    "artifact-cleanup",
    "Cleanup old Docker images from Artifact Registry (for GitHub Actions)",
  )
  .action(async () => {
    await runArtifactCleanupCLI();
  });

cli
  .command(
    "artifact-check",
    "Check Docker images in Artifact Registry and verify cleanup status",
  )
  .action(async () => {
    await runArtifactCheckCLI();
  });

cli.command("", "Show help").action(() => {
  const helpText = `
# Services Helper Script

This script helps manage Google Cloud setup for the Collects services and GitHub Actions utilities.

## Usage

\`\`\`bash
bun run main.ts <command>
\`\`\`

## Commands

### \`actions-setup\`

Sets up Workload Identity Federation for GitHub Actions to deploy to Google Cloud.

**What it does:**
1. Enables necessary Google Cloud APIs.
2. Creates a Workload Identity Pool and Provider.
3. Creates a dedicated Service Account.
4. Links the GitHub Repository to the Service Account.
5. Grants necessary permissions (Artifact Registry, Cloud Run, Secrets) to the Service Account.
6. Outputs the YAML configuration for your GitHub Actions workflow.

**Example:**
\`\`\`bash
bun run main.ts actions-setup
# Or with options:
bun run main.ts actions-setup --project-id my-gcp-project-id --repo username/repository
\`\`\`

### \`actions-migrate-repo\`

Migrates workload identity bindings when a GitHub repository is moved to a new org or renamed.

**What it does:**
1. Updates the Workload Identity Provider's attribute condition to use the new repository.
2. Adds a new IAM binding for the Service Account to allow the new repository.
3. Removes the old IAM binding to revoke access from the old repository path.

**When to use:**
- When you move a repository to a different organization (e.g., \`old-org/repo\` → \`new-org/repo\`)
- When you rename a repository
- When you fork and want to use the same GCP setup

**Example:**
\`\`\`bash
bun run main.ts actions-migrate-repo
# Or with options:
bun run main.ts actions-migrate-repo --project-id my-gcp-project-id --old-repo old-org/old-repo --new-repo new-org/new-repo
# For example, migrating to lqxc-org:
just scripts::actions-migrate-repo --project-id braided-case-416903 --old-repo old-owner/collects --new-repo lqxc-org/collects
\`\`\`

### \`init-db-secret\`

Initializes Neon Database branches and updates Google Cloud Secrets with connection URLs.

**What it does:**
1. Fetches Neon project branches (expects 'main'/'production' and 'development'/'dev').
2. Creates a restricted 'app_user' role on production (for least-privilege in prod).
3. Resets passwords for all roles to generate fresh credentials.
4. Creates/updates Google Cloud secrets for all environments:
   - \`database-url\` (prod, restricted role)
   - \`database-url-internal\` (internal, admin role on production, deploys with prod)
   - \`database-url-test\` (test, admin role on development)
   - \`database-url-test-internal\` (test-internal, admin role on development, deploys with main)
   - \`database-url-pr\` (pr, admin role on development)
   - \`database-url-local\` (local dev, admin role on development)

**Example:**
\`\`\`bash
bun run main.ts init-db-secret --token <NEON_API_TOKEN> --project-id <NEON_PROJECT_ID>
\`\`\`

### \`version-check\`

Checks if the version in a Cargo.toml file has changed (for GitHub Actions).

**What it does:**
1. Reads the current version from the specified Cargo.toml file.
2. Compares it with the version from the previous commit.
3. Outputs the result to console and GITHUB_OUTPUT if running in CI.

**Example:**
\`\`\`bash
bun run main.ts version-check ui/Cargo.toml
bun run main.ts version-check services/Cargo.toml
\`\`\`

### \`env-feature\`

Gets cargo feature flags for an environment. Used by justfiles to centralize environment configuration.

**Example:**
\`\`\`bash
bun run main.ts env-feature pr        # Output: --features env_pr
bun run main.ts env-feature test      # Output: --features env_test
bun run main.ts env-feature prod      # Output: (empty - no feature flag)
\`\`\`

### \`env-secret\`

Gets database secret name for an environment. Used by justfiles to centralize environment configuration.

**Example:**
\`\`\`bash
bun run main.ts env-secret pr         # Output: database-url-pr
bun run main.ts env-secret prod       # Output: database-url
bun run main.ts env-secret local      # Output: database-url-local
\`\`\`

### \`jwt-secret\`

Gets JWT secret name for an environment. Used by justfiles to centralize environment configuration.
Returns an empty string for environments that use the default local secret (local, test, test-internal).

**Example:**
\`\`\`bash
bun run main.ts jwt-secret pr         # Output: jwt-secret-pr
bun run main.ts jwt-secret prod       # Output: jwt-secret
bun run main.ts jwt-secret local      # Output: (empty - uses default local secret)
\`\`\`

### \`env-list\`

Lists all available environment names.

**Example:**
\`\`\`bash
bun run main.ts env-list              # Lists: prod, internal, nightly, test, test-internal, pr, local
\`\`\`

### \`check-pr-title\`

Validates PR title format against conventional commits specification.

**What it does:**
1. Validates the PR title against conventional commit format: \`<type>[optional scope]: <description>\`
2. Exits with code 0 if valid, 1 if invalid.

**Valid types:** feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert

**Example:**
\`\`\`bash
just scripts::check-pr-title "feat: add user authentication"
\`\`\`

### \`r2-setup\`

Sets up Cloudflare R2 storage secrets in Google Cloud Secret Manager.

**What it does:**
1. Prompts for R2 credentials (Account ID, Access Key ID, Secret Access Key, Bucket).
2. Creates secrets in Google Cloud Secret Manager if they don't exist.
3. Updates secret values with the provided credentials.

**Example:**
\`\`\`bash
bun run main.ts r2-setup --project-id my-gcp-project-id
\`\`\`

### \`r2-list\`

Lists the status of Cloudflare R2 secrets in Google Cloud Secret Manager.

**Example:**
\`\`\`bash
bun run main.ts r2-list --project-id my-gcp-project-id
\`\`\`

### \`ci-feedback\`

Automatically posts CI failure details to your PR and asks Copilot to help fix the issues.

**How it works:**
When your CI fails, this command collects the error logs and posts a helpful comment on the PR
that mentions @copilot, so Copilot can analyze the failures and suggest fixes.

**🔑 One-time Setup: Create a Personal Access Token**

Since Copilot only responds to comments from real users (not bots), you need to create a
Personal Access Token (PAT) so the comment appears to come from your account.

**Quick Setup (Fine-grained Token - Recommended):**

1. Open GitHub: Settings → Developer settings → Personal access tokens → Fine-grained tokens
2. Click "Generate new token"
3. Name it something like "CI Copilot Helper"
4. Choose an expiration (or no expiration)
5. Under "Repository access", select this repository
6. Set these permissions:
   - Pull requests: Read and write ✏️
   - Actions: Read 👁️
   - Contents: Read 👁️
7. Click "Generate token" and copy it
8. In your repo: Settings → Secrets → Actions → New secret
9. Name: \`COPILOT_INVOKER_TOKEN\`, Value: paste your token

**Alternative: Classic Token**

If you prefer a classic token, create one with the \`repo\` scope and save it as \`COPILOT_INVOKER_TOKEN\`.

That's it! Now when CI fails on a PR, Copilot will automatically be asked to help.

### \`scheduled-job-issue\`

Creates GitHub issues when scheduled background jobs fail. This tool monitors scheduled workflow runs 
and automatically creates detailed issues with diagnosis plans and possible root causes.

**How it works:**
When a scheduled job (like \`Artifact Cleanup\`) fails, this command:
1. Collects error logs from the failed jobs
2. Analyzes the errors to generate diagnosis plans
3. Creates (or updates) a GitHub issue with:
   - Error summaries
   - Possible root causes
   - Step-by-step diagnosis instructions
   - Suggested actions

**Features:**
- **Deduplication**: If an issue already exists for the same workflow, adds a comment instead of creating a new issue
- **Smart Diagnosis**: Automatically categorizes errors (authentication, network, Docker, etc.)
- **Actionable**: Provides specific diagnosis steps based on error patterns

**Environment Variables:**
- \`GITHUB_TOKEN\` - GitHub token with issues:write permission
- \`WORKFLOW_RUN_ID\` - ID of the failed workflow run
- \`WORKFLOW_NAME\` - Name of the workflow that failed
- \`WORKFLOW_RUN_URL\` - URL to the failed workflow run
- \`HEAD_SHA\` - Git SHA of the commit that triggered the workflow

**Labels Applied:**
- \`scheduled-job-failure\`
- \`automated\`

**Example:**
\`\`\`bash
# Usually called from the scheduled-job-monitor.yml workflow
bun run main.ts scheduled-job-issue
\`\`\`

### \`artifact-cleanup\`

Cleans up old Docker images from Google Cloud Artifact Registry based on retention policies.

**What it does:**
1. Lists all Docker images in the specified Artifact Registry repository.
2. Applies retention policies based on image tags:
   - Nightly builds (\`nightly-YYYYMMDD\`): Deleted after 7 days
   - Main branch builds (\`main-<sha>\`): Deleted after 1 day
   - Production releases (\`v<version>\`): Deleted after 30 days
   - PR builds (\`pr-<number>\`): Handled separately by cleanup-pr.yml
3. Deletes images that exceed their retention period.

**Environment Variables:**
- \`GCP_PROJECT_ID\` - Google Cloud Project ID (optional, uses gcloud config if not set)
- \`GCP_REGION\` - Artifact Registry region (default: us-east1)
- \`GCP_REPOSITORY\` - Repository name (default: collects-services)
- \`GCP_IMAGE_NAME\` - Image name (default: collects-services)
- \`DRY_RUN\` - Set to "true" to preview deletions without executing them

**Example:**
\`\`\`bash
# Preview what would be deleted
DRY_RUN=true bun run main.ts artifact-cleanup

# Actually delete old images
bun run main.ts artifact-cleanup
\`\`\`

### \`artifact-check\`

Checks the current state of Docker images in Artifact Registry and verifies cleanup compliance.

**What it does:**
1. Lists all Docker images in the Artifact Registry repository.
2. Categorizes images by type (PR, nightly, main, production).
3. Checks if images are within their retention policies.
4. Reports violations (images that should have been cleaned up).

**Environment Variables:**
- \`GCP_PROJECT_ID\` - Google Cloud Project ID (optional, uses gcloud config if not set)
- \`GCP_REGION\` - Artifact Registry region (default: us-east1)
- \`GCP_REPOSITORY\` - Repository name (default: collects-services)
- \`GCP_IMAGE_NAME\` - Image name (default: collects-services)

**Example:**
\`\`\`bash
# Check current artifact registry status
just scripts::artifact-check

# Or with bun directly
bun run main.ts artifact-check
\`\`\`

---
Run \`bun run main.ts --help\` for CLI details.
`;
  console.log(helpText);
});

cli.help();
cli.parse();
