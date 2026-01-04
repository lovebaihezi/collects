#!/usr/bin/env bun
import { cac } from "cac";
import * as p from "@clack/prompts";
import { initDbSecret } from "./services/neon.ts";
import {
  buildSetupContext,
  setupGitHubActions,
  type BuildSetupOptions,
} from "./services/gcloud.ts";
import { runVersionCheck } from "./gh-actions/version-check.ts";
import { runCIFeedbackCLI } from "./gh-actions/ci-feedback.ts";
import {
  getCargoFeature,
  getDatabaseSecret,
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
### \`ci-feedback\`

Posts CI failure feedback to the PR. This command is designed to be called from GitHub Actions.

**What it does:**
1. Reads workflow run information from environment variables.
2. Collects failed job logs and extracts relevant error lines.
3. Counts previous failures per job (stops at 3 to prevent loops).
4. Posts a structured comment on the PR mentioning @copilot for analysis.

**Required Environment Variables:**
- \`GITHUB_TOKEN\` - GitHub token with write access to PRs (see note below)
- \`GITHUB_REPOSITORY_OWNER\` - Repository owner
- \`GITHUB_REPOSITORY\` - Full repository name (owner/repo)
- \`WORKFLOW_RUN_ID\` - The workflow run ID
- \`HEAD_SHA\` - The commit SHA
- \`WORKFLOW_RUN_URL\` - URL to the workflow run

**Important: Token Requirements for Copilot Invocation**

To enable @copilot mentions to trigger Copilot responses, you must use a Personal Access Token (PAT)
from a user account instead of the default \`GITHUB_TOKEN\`. Comments from the GitHub Actions bot
(using \`GITHUB_TOKEN\`) cannot invoke Copilot - only user account comments can.

**Setup Instructions:**
1. Create a Personal Access Token (PAT) with \`repo\` scope
2. Add the PAT as a repository secret named \`COPILOT_INVOKER_TOKEN\`
3. The workflow will automatically use this token if available

If \`COPILOT_INVOKER_TOKEN\` is not configured, the workflow falls back to \`GITHUB_TOKEN\`,
which will post the comment but won't trigger Copilot responses.

**Example:**
\`\`\`bash
GITHUB_TOKEN=xxx WORKFLOW_RUN_ID=123 ... bun run main.ts ci-feedback
\`\`\`

---
Run \`bun run main.ts --help\` for CLI details.
`;
  console.log(helpText);
});

cli.help();
cli.parse();
