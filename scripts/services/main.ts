#!/usr/bin/env bun
import { cac } from "cac";
import * as p from "@clack/prompts";
import { $ } from "bun";
import { type } from "arktype";

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
 * Runs a shell command with error handling and LLM prompt generation.
 */
async function runCommand(command: string, context: string) {
  const s = p.spinner();
  try {
    // We use Bun.spawn to have better control or just use $ if simple
    // Using $ from bun as imported. We capture stdout to keep the UI clean.
    s.start(`Run Google Cloud CLI: ${command}`);
    const { stdout } = await $`${{ raw: command }}`.quiet();
    s.stop("GCLI succeeded");
    return stdout.toString();
  } catch (err: any) {
    s.stop(`Failed to run command: ${command}`);
    p.log.error(`COMMAND FAILED: ${command}`);

    let errorOutput = "";

    // ShellError is not exported from 'bun' in the current version, so we check the name/properties
    if (err.name === "ShellError" || (err.stdout && err.stderr)) {
      errorOutput = err.stdout.toString() + err.stderr.toString();
    } else {
      errorOutput = err.message || String(err);
    }

    p.log.error(`ERROR: ${errorOutput.trim()}`);

    const llmPrompt = `I ran the command \`${command}\` to ${context} and got this error:

${errorOutput.trim()}

How do I fix this in Google Cloud?`;

    p.log.info("To get help from an AI assistant, use the following prompt:");
    p.log.message(llmPrompt);

    process.exit(1);
  }
}

/**
 * Asks for confirmation before running a command.
 */
async function confirmAndRun(command: string, context: string) {
  p.log.info(`Next step: ${context}`);
  p.log.message(`Command: ${command}`);

  const shouldRun = await p.confirm({
    message: "Do you want to run this command?",
  });

  if (p.isCancel(shouldRun) || !shouldRun) {
    p.log.warn("Operation cancelled by user.");
    process.exit(0);
  }

  await runCommand(command, context);
  p.log.success("Command executed successfully.");
}

/**
 * Checks if a resource exists using gcloud describe/list.
 * Returns true if exists, false otherwise.
 * Mutes output to keep the flow clean.
 */
async function checkResource(command: string): Promise<boolean> {
  try {
    // Run quietly, we only care about exit code
    await $`${{ raw: command }} --quiet`.quiet();
    return true;
  } catch {
    return false;
  }
}

/**
 * Validates and parses repository format
 */
function validateRepo(repo: string): { owner: string; repo: string } {
  const repoType = type(/^[^/]+\/[^/]+$/);
  const result = repoType(repo);

  if (result instanceof type.errors) {
    p.log.error(`Invalid repository format: ${result.summary}`);
    process.exit(1);
  }

  const [owner, repoName] = result.split("/");
  return { owner, repo: repoName };
}

/**
 * Gets project number from project ID
 */
async function getProjectNumber(projectId: string): Promise<string> {
  const projectNumber =
    await $`gcloud projects describe ${projectId} --format="value(projectNumber)"`.text();
  return projectNumber.trim();
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

cli.command("", "Show help").action(() => {
  const helpText = `
# Services Helper Script

This script helps manage Google Cloud setup for the Collects services.

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

---
Run \`bun run main.ts --help\` for CLI details.
`;
  console.log(helpText);
});

cli.help();
cli.parse();
