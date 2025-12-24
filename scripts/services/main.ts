#!/usr/bin/env bun
import { cac } from "cac";
import * as p from "@clack/prompts";
import { $ } from "bun";
import {
  checkResource,
  confirmAndRun,
  getProjectNumber,
  validateRepo,
} from "./utils.ts";
import {
  initDbSecret,
  getDatabaseUrl,
  getSecretNameForEnv,
  listNeonBranches,
  createNeonBranch,
  deleteNeonBranch,
  getBranchConnectionString,
  updateSecret,
  type DbEnv,
} from "./neon.ts";

const cli = cac("services");

/**
 * Context for GitHub Actions setup
 */
interface SetupContext {
  projectId: string;
  repo: string;
  owner: string;
  projectNumber: string;
  poolName: string;
  providerName: string;
  saName: string;
  saEmail: string;
  poolId: string;
  providerId: string;
}

/**
 * Creates setup context from inputs
 */
async function createSetupContext(
  projectId: string,
  repo: string,
): Promise<SetupContext> {
  const { owner } = validateRepo(repo);
  const projectNumber = await getProjectNumber(projectId);

  const poolName = "github-actions-pool";
  const providerName = "github-provider";
  const saName = "github-actions-sa";
  const saEmail = `${saName}@${projectId}.iam.gserviceaccount.com`;
  const poolId = `projects/${projectNumber}/locations/global/workloadIdentityPools/${poolName}`;
  const providerId = `${poolId}/providers/${providerName}`;

  return {
    projectId,
    repo,
    owner,
    projectNumber,
    poolName,
    providerName,
    saName,
    saEmail,
    poolId,
    providerId,
  };
}

/**
 * Checks if workload identity pool exists
 */
async function checkPoolExists(ctx: SetupContext): Promise<boolean> {
  return checkResource(
    `gcloud iam workload-identity-pools describe ${ctx.poolName} --project=${ctx.projectId} --location=global`,
  );
}

/**
 * Checks if workload identity provider exists
 */
async function checkProviderExists(ctx: SetupContext): Promise<boolean> {
  return checkResource(
    `gcloud iam workload-identity-pools providers describe ${ctx.providerName} --workload-identity-pool=${ctx.poolName} --project=${ctx.projectId} --location=global`,
  );
}

/**
 * Checks if service account exists
 */
async function checkServiceAccountExists(ctx: SetupContext): Promise<boolean> {
  return checkResource(
    `gcloud iam service-accounts describe ${ctx.saEmail} --project=${ctx.projectId}`,
  );
}

/**
 * Enables IAM Credentials API
 */
async function enableIAMCredentialsAPI(ctx: SetupContext): Promise<void> {
  await confirmAndRun(
    `gcloud services enable iamcredentials.googleapis.com --project ${ctx.projectId}`,
    "Enable IAM Credentials API",
  );
}

/**
 * Creates workload identity pool
 */
async function createWorkloadIdentityPool(ctx: SetupContext): Promise<void> {
  await confirmAndRun(
    `gcloud iam workload-identity-pools create ${ctx.poolName} --project=${ctx.projectId} --location=global --display-name="GitHub Actions Pool"`,
    "Create Workload Identity Pool",
  );
}

/**
 * Creates workload identity provider
 */
async function createWorkloadIdentityProvider(
  ctx: SetupContext,
): Promise<void> {
  await confirmAndRun(
    `gcloud iam workload-identity-pools providers create-oidc ${ctx.providerName} --project=${ctx.projectId} --location=global --workload-identity-pool=${ctx.poolName} --display-name="GitHub Provider" --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" --issuer-uri="https://token.actions.githubusercontent.com" --attribute-condition="attribute.repository_owner=='${ctx.owner}' && attribute.repository=='${ctx.repo}'"`,
    "Create Workload Identity Provider",
  );
}

/**
 * Creates service account
 */
async function createServiceAccount(ctx: SetupContext): Promise<void> {
  await confirmAndRun(
    `gcloud iam service-accounts create ${ctx.saName} --project=${ctx.projectId} --display-name="GitHub Actions Service Account"`,
    "Create Service Account",
  );
}

/**
 * Binds service account to workload identity pool
 */
async function bindServiceAccountToPool(ctx: SetupContext): Promise<void> {
  const principalSet = `principalSet://iam.googleapis.com/projects/${ctx.projectNumber}/locations/global/workloadIdentityPools/${ctx.poolName}/attribute.repository/${ctx.repo}`;
  await confirmAndRun(
    `gcloud iam service-accounts add-iam-policy-binding ${ctx.saEmail} --project=${ctx.projectId} --role="roles/iam.workloadIdentityUser" --member="${principalSet}"`,
    `Allow GitHub Repo '${ctx.repo}' to impersonate Service Account`,
  );
}

/**
 * Grants a role to service account
 */
async function grantRoleToServiceAccount(
  ctx: SetupContext,
  role: string,
): Promise<void> {
  await confirmAndRun(
    `gcloud projects add-iam-policy-binding ${ctx.projectId} --member="serviceAccount:${ctx.saEmail}" --role="${role}" --condition=None`,
    `Grant '${role}' to Service Account`,
  );
}

/**
 * Grants all necessary roles to service account
 */
async function grantRolesToServiceAccount(ctx: SetupContext): Promise<void> {
  const roles = [
    "roles/artifactregistry.writer",
    "roles/secretmanager.secretAccessor",
    "roles/run.admin",
    "roles/iam.serviceAccountUser",
  ];

  for (const role of roles) {
    await grantRoleToServiceAccount(ctx, role);
  }
}

/**
 * Grants Secret Accessor role to the default compute service account
 */
async function grantSecretAccessorToComputeServiceAccount(
  ctx: SetupContext,
): Promise<void> {
  const computeSaEmail = `${ctx.projectNumber}-compute@developer.gserviceaccount.com`;
  await confirmAndRun(
    `gcloud projects add-iam-policy-binding ${ctx.projectId} --member="serviceAccount:${computeSaEmail}" --role="roles/secretmanager.secretAccessor" --condition=None`,
    `Grant 'roles/secretmanager.secretAccessor' to Default Compute Service Account (${computeSaEmail})`,
  );
}

/**
 * Displays the final workflow YAML
 */
function displayWorkflowYAML(ctx: SetupContext): void {
  const yamlOutput = `
# Add this to your GitHub Actions workflow:

- id: 'auth'
  name: 'Authenticate to Google Cloud'
  uses: 'google-github-actions/auth@v2'
  with:
    workload_identity_provider: '${ctx.providerId}'
    service_account: '${ctx.saEmail}'
`;

  console.log(yamlOutput);
}

/**
 * Options for building setup context
 */
interface BuildSetupOptions {
  projectId?: string;
  repo?: string;
}

/**
 * Builds setup context from command-line options or prompts
 * Uses early return pattern to avoid let variables
 */
async function buildSetupContext(
  options: BuildSetupOptions,
): Promise<SetupContext> {
  // Early return if both options are provided
  if (options.projectId && options.repo) {
    return createSetupContext(options.projectId, options.repo);
  }

  // Prompt for missing options
  const projectGroup = await p.group(
    {
      projectId: () =>
        options.projectId
          ? Promise.resolve(options.projectId)
          : p.text({
              message: "Enter your Google Cloud Project ID:",
              placeholder: "my-gcp-project-id",
              validate: (value) => {
                if (!value) return "Project ID is required";
              },
            }),
      repo: () =>
        options.repo
          ? Promise.resolve(options.repo)
          : p.text({
              message: "Enter your GitHub Repository (owner/repo):",
              placeholder: "username/repository",
              validate: (value) => {
                if (!value) return "Repository is required";
                if (!value.includes("/")) return "Format must be owner/repo";
              },
            }),
    },
    {
      onCancel: () => {
        p.cancel("Operation cancelled.");
        process.exit(0);
      },
    },
  );

  return createSetupContext(projectGroup.projectId, projectGroup.repo);
}

/**
 * Main setup orchestration
 */
async function setupGitHubActions(ctx: SetupContext): Promise<void> {
  p.log.info(`Using Project Number: ${ctx.projectNumber}`);
  p.log.info(`Using repo: ${ctx.repo}`);

  // 1. Enable IAM Credentials API
  await enableIAMCredentialsAPI(ctx);

  // 2. Create Workload Identity Pool
  const poolExists = await checkPoolExists(ctx);
  if (!poolExists) {
    await createWorkloadIdentityPool(ctx);
  } else {
    p.log.info(`Workload Identity Pool '${ctx.poolName}' already exists.`);
  }

  // 3. Create Workload Identity Provider
  const providerExists = await checkProviderExists(ctx);
  if (!providerExists) {
    await createWorkloadIdentityProvider(ctx);
  } else {
    p.log.info(
      `Workload Identity Provider '${ctx.providerName}' already exists.`,
    );
  }

  // 4. Create Service Account
  const saExists = await checkServiceAccountExists(ctx);
  if (!saExists) {
    await createServiceAccount(ctx);
  } else {
    p.log.info(`Service Account '${ctx.saName}' already exists.`);
  }

  // 5. Bind Service Account to Pool
  await bindServiceAccountToPool(ctx);

  // 6. Grant Roles to Service Account
  await grantRolesToServiceAccount(ctx);

  // 7. Grant Secret Accessor to Compute Service Account
  await grantSecretAccessorToComputeServiceAccount(ctx);

  p.outro("Setup Complete!");
  displayWorkflowYAML(ctx);
}

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
    });
    await setupGitHubActions(ctx);
  });

cli
  .command("init-db-secret", "Initialize Neon Database and update Secrets")
  .option("--token <token>", "Neon API Token")
  .action(async (options) => {
    if (!options.token) {
      console.error("Error: --token is required");
      process.exit(1);
    }
    await initDbSecret(options.token);
  });

// =============================================================================
// Database Migration Commands
// =============================================================================

cli
  .command("migrate <env>", "Run sqlx migrations on a specific environment")
  .option("--source <path>", "Path to migrations directory", {
    default: "../services/migrations",
  })
  .action(async (env: DbEnv, options) => {
    p.intro(`Running migrations on ${env} environment`);

    const s = p.spinner();
    s.start("Fetching database URL from Google Cloud Secrets...");

    try {
      const databaseUrl = await getDatabaseUrl(env);
      s.stop("Database URL retrieved.");

      s.start("Running migrations...");
      await $`DATABASE_URL=${databaseUrl} sqlx migrate run --source ${options.source}`;
      s.stop("Migrations complete.");

      p.outro(`Successfully applied migrations to ${env} environment.`);
    } catch (e: unknown) {
      s.stop("Migration failed.");
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to run migrations: ${errorMessage}`);
      process.exit(1);
    }
  });

cli
  .command(
    "migrate-info <env>",
    "Check migration status on a specific environment",
  )
  .option("--source <path>", "Path to migrations directory", {
    default: "../services/migrations",
  })
  .action(async (env: DbEnv, options) => {
    p.intro(`Checking migration status on ${env} environment`);

    const s = p.spinner();
    s.start("Fetching database URL from Google Cloud Secrets...");

    try {
      const databaseUrl = await getDatabaseUrl(env);
      s.stop("Database URL retrieved.");

      p.log.info("Migration status:");
      await $`DATABASE_URL=${databaseUrl} sqlx migrate info --source ${options.source}`;

      p.outro("Done.");
    } catch (e: unknown) {
      s.stop("Failed.");
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to check migration status: ${errorMessage}`);
      process.exit(1);
    }
  });

cli
  .command(
    "migrate-revert <env>",
    "Revert the last migration on a specific environment",
  )
  .option("--source <path>", "Path to migrations directory", {
    default: "../services/migrations",
  })
  .action(async (env: DbEnv, options) => {
    p.intro(`Reverting last migration on ${env} environment`);

    const confirmed = await p.confirm({
      message: `Are you sure you want to revert the last migration on ${env}?`,
    });

    if (p.isCancel(confirmed) || !confirmed) {
      p.cancel("Operation cancelled.");
      process.exit(0);
    }

    const s = p.spinner();
    s.start("Fetching database URL from Google Cloud Secrets...");

    try {
      const databaseUrl = await getDatabaseUrl(env);
      s.stop("Database URL retrieved.");

      s.start("Reverting migration...");
      await $`DATABASE_URL=${databaseUrl} sqlx migrate revert --source ${options.source}`;
      s.stop("Migration reverted.");

      p.outro(`Successfully reverted last migration on ${env} environment.`);
    } catch (e: unknown) {
      s.stop("Revert failed.");
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to revert migration: ${errorMessage}`);
      process.exit(1);
    }
  });

// =============================================================================
// Neon Branch Management Commands
// =============================================================================

cli
  .command("neon-branches", "List all Neon branches for a project")
  .option("--token <token>", "Neon API Token")
  .option("--project-id <projectId>", "Neon Project ID")
  .action(async (options) => {
    if (!options.token || !options.projectId) {
      console.error("Error: --token and --project-id are required");
      process.exit(1);
    }

    p.intro("Listing Neon branches");

    try {
      const branches = await listNeonBranches(options.token, options.projectId);
      p.log.info("Branches:");
      for (const branch of branches) {
        console.log(`  - ${branch.name} (${branch.id})`);
      }
      p.outro("Done.");
    } catch (e: unknown) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to list branches: ${errorMessage}`);
      process.exit(1);
    }
  });

cli
  .command("neon-create-branch <name>", "Create a new Neon branch")
  .option("--token <token>", "Neon API Token")
  .option("--project-id <projectId>", "Neon Project ID")
  .option("--parent <parentId>", "Parent branch ID")
  .action(async (name: string, options) => {
    if (!options.token || !options.projectId || !options.parent) {
      console.error(
        "Error: --token, --project-id, and --parent are required",
      );
      process.exit(1);
    }

    p.intro(`Creating Neon branch: ${name}`);

    const s = p.spinner();
    s.start("Creating branch...");

    try {
      const branch = await createNeonBranch(
        options.token,
        options.projectId,
        name,
        options.parent,
      );
      s.stop("Branch created.");
      p.log.success(`Branch created: ${branch.name} (${branch.id})`);
      p.outro("Done.");
    } catch (e: unknown) {
      s.stop("Failed.");
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to create branch: ${errorMessage}`);
      process.exit(1);
    }
  });

cli
  .command(
    "neon-update-secret <env>",
    "Update a Google Cloud Secret with a Neon branch connection string",
  )
  .option("--token <token>", "Neon API Token")
  .option("--project-id <projectId>", "Neon Project ID")
  .option("--branch-id <branchId>", "Neon Branch ID")
  .option("--role <role>", "Database role name", { default: "web_user" })
  .option("--database <database>", "Database name", { default: "collects" })
  .action(async (env: DbEnv, options) => {
    if (!options.token || !options.projectId || !options.branchId) {
      console.error(
        "Error: --token, --project-id, and --branch-id are required",
      );
      process.exit(1);
    }

    p.intro(`Updating ${env} environment secret with Neon branch connection`);

    const s = p.spinner();
    s.start("Getting connection string from Neon...");

    try {
      const connString = await getBranchConnectionString(
        options.token,
        options.projectId,
        options.branchId,
        options.role,
        options.database,
      );

      if (!connString) {
        s.stop("Failed.");
        p.log.error("Could not get connection string for the branch.");
        process.exit(1);
      }

      s.stop("Connection string retrieved.");

      const secretName = getSecretNameForEnv(env);
      await updateSecret(secretName, connString);

      p.outro(`Successfully updated ${secretName} secret.`);
    } catch (e: unknown) {
      s.stop("Failed.");
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to update secret: ${errorMessage}`);
      process.exit(1);
    }
  });

cli
  .command("show-db-url <env>", "Display the database URL for an environment (masked)")
  .action(async (env: DbEnv) => {
    p.intro(`Getting database URL for ${env} environment`);

    try {
      const url = await getDatabaseUrl(env);
      // Mask the password in the URL for display
      const maskedUrl = url.replace(/:([^:@]+)@/, ":****@");
      p.log.info(`Database URL (masked): ${maskedUrl}`);
      p.outro("Done.");
    } catch (e: unknown) {
      const errorMessage = e instanceof Error ? e.message : String(e);
      p.log.error(`Failed to get database URL: ${errorMessage}`);
      process.exit(1);
    }
  });

cli.command("", "Show help").action(() => {
  const helpText = `
# Services Helper Script

This script helps manage Google Cloud setup, database migrations, and Neon branch management for the Collects services.

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

Initializes a new Neon database project and updates Google Cloud Secrets.

**Example:**
\`\`\`bash
bun run main.ts init-db-secret --token <neon-api-token>
\`\`\`

---

## Database Migration Commands

### \`migrate <env>\`

Run sqlx migrations on a specific environment.
Environments: prod, test, pr, nightly, internal

**Example:**
\`\`\`bash
bun run main.ts migrate internal
bun run main.ts migrate test --source ../services/migrations
\`\`\`

### \`migrate-info <env>\`

Check migration status on a specific environment.

**Example:**
\`\`\`bash
bun run main.ts migrate-info prod
\`\`\`

### \`migrate-revert <env>\`

Revert the last migration on a specific environment (with confirmation).

**Example:**
\`\`\`bash
bun run main.ts migrate-revert test
\`\`\`

---

## Neon Branch Management Commands

### \`neon-branches\`

List all Neon branches for a project.

**Example:**
\`\`\`bash
bun run main.ts neon-branches --token <token> --project-id <project-id>
\`\`\`

### \`neon-create-branch <name>\`

Create a new Neon branch from a parent branch.

**Example:**
\`\`\`bash
bun run main.ts neon-create-branch feature-xyz --token <token> --project-id <project-id> --parent <parent-branch-id>
\`\`\`

### \`neon-update-secret <env>\`

Update a Google Cloud Secret with a Neon branch connection string.

**Example:**
\`\`\`bash
bun run main.ts neon-update-secret pr --token <token> --project-id <project-id> --branch-id <branch-id>
\`\`\`

### \`show-db-url <env>\`

Display the database URL for an environment (password masked).

**Example:**
\`\`\`bash
bun run main.ts show-db-url prod
\`\`\`

---
Run \`bun run main.ts --help\` for CLI details.
`;
  console.log(helpText);
});

cli.help();
cli.parse();
