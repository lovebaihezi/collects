import * as p from "@clack/prompts";
import {
  checkResource,
  confirmAndRun,
  getProjectNumber,
  validateRepo,
} from "./utils.ts";

/**
 * Context for GitHub Actions setup
 */
export interface SetupContext {
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
    "roles/artifactregistry.repoAdmin", // Includes write + delete permissions for cleanup jobs
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
 * Grants Secret Accessor role to the default compute service account for all database secrets
 * This includes: database-url, database-url-internal, database-url-test, database-url-pr, database-url-local
 */
export async function grantSecretAccessToAllDatabaseSecrets(
  ctx: SetupContext,
): Promise<void> {
  const computeSaEmail = `${ctx.projectNumber}-compute@developer.gserviceaccount.com`;
  const databaseSecrets = [
    "database-url",
    "database-url-internal",
    "database-url-test",
    "database-url-test-internal",
    "database-url-pr",
    "database-url-local",
  ];

  for (const secretName of databaseSecrets) {
    await confirmAndRun(
      `gcloud secrets add-iam-policy-binding ${secretName} --member="serviceAccount:${computeSaEmail}" --role="roles/secretmanager.secretAccessor" --project=${ctx.projectId}`,
      `Grant access to secret '${secretName}' for Compute Service Account`,
    );
  }
}

/**
 * Grants Secret Accessor role to the default compute service account for all JWT secrets
 * This includes: jwt-secret, jwt-secret-pr
 */
export async function grantSecretAccessToAllJwtSecrets(
  ctx: SetupContext,
): Promise<void> {
  const computeSaEmail = `${ctx.projectNumber}-compute@developer.gserviceaccount.com`;
  const jwtSecrets = [
    "jwt-secret", // Used by prod, internal, nightly
    "jwt-secret-pr", // Used by PR environment
  ];

  for (const secretName of jwtSecrets) {
    await confirmAndRun(
      `gcloud secrets add-iam-policy-binding ${secretName} --member="serviceAccount:${computeSaEmail}" --role="roles/secretmanager.secretAccessor" --project=${ctx.projectId}`,
      `Grant access to secret '${secretName}' for Compute Service Account`,
    );
  }
}

/**
 * Grants access to all Zero Trust secrets for the Compute Service Account.
 * These secrets are required for the internal environment.
 */
export async function grantSecretAccessToAllZeroTrustSecrets(
  ctx: SetupContext,
): Promise<void> {
  const computeSaEmail = `${ctx.projectNumber}-compute@developer.gserviceaccount.com`;
  const zeroTrustSecrets = [
    "cf-access-team-domain", // Cloudflare Access team domain
    "cf-access-aud", // Cloudflare Access application audience
  ];

  for (const secretName of zeroTrustSecrets) {
    await confirmAndRun(
      `gcloud secrets add-iam-policy-binding ${secretName} --member="serviceAccount:${computeSaEmail}" --role="roles/secretmanager.secretAccessor" --project=${ctx.projectId}`,
      `Grant access to secret '${secretName}' for Compute Service Account`,
    );
  }
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
export interface BuildSetupOptions {
  projectId?: string;
  repo?: string;
}

/**
 * Builds setup context from command-line options or prompts
 * Uses early return pattern to avoid let variables
 */
export async function buildSetupContext(
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
 * Context for repository migration
 */
export interface MigrateRepoContext {
  projectId: string;
  oldRepo: string;
  newRepo: string;
  oldOwner: string;
  newOwner: string;
  projectNumber: string;
  poolName: string;
  providerName: string;
  saName: string;
  saEmail: string;
}

/**
 * Options for building migrate repo context
 */
export interface BuildMigrateRepoOptions {
  projectId?: string;
  oldRepo?: string;
  newRepo?: string;
}

/**
 * Creates migrate context from inputs
 */
async function createMigrateContext(
  projectId: string,
  oldRepo: string,
  newRepo: string,
): Promise<MigrateRepoContext> {
  const { owner: oldOwner } = validateRepo(oldRepo);
  const { owner: newOwner } = validateRepo(newRepo);
  const projectNumber = await getProjectNumber(projectId);

  const poolName = "github-actions-pool";
  const providerName = "github-provider";
  const saName = "github-actions-sa";
  const saEmail = `${saName}@${projectId}.iam.gserviceaccount.com`;

  return {
    projectId,
    oldRepo,
    newRepo,
    oldOwner,
    newOwner,
    projectNumber,
    poolName,
    providerName,
    saName,
    saEmail,
  };
}

/**
 * Builds migrate repo context from command-line options or prompts
 */
export async function buildMigrateRepoContext(
  options: BuildMigrateRepoOptions,
): Promise<MigrateRepoContext> {
  // Early return if all options are provided
  if (options.projectId && options.oldRepo && options.newRepo) {
    return createMigrateContext(
      options.projectId,
      options.oldRepo,
      options.newRepo,
    );
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
      oldRepo: () =>
        options.oldRepo
          ? Promise.resolve(options.oldRepo)
          : p.text({
              message: "Enter the OLD GitHub Repository (owner/repo):",
              placeholder: "old-owner/old-repo",
              validate: (value) => {
                if (!value) return "Old repository is required";
                if (!value.includes("/")) return "Format must be owner/repo";
              },
            }),
      newRepo: () =>
        options.newRepo
          ? Promise.resolve(options.newRepo)
          : p.text({
              message: "Enter the NEW GitHub Repository (owner/repo):",
              placeholder: "new-owner/new-repo",
              validate: (value) => {
                if (!value) return "New repository is required";
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

  return createMigrateContext(
    projectGroup.projectId,
    projectGroup.oldRepo,
    projectGroup.newRepo,
  );
}

/**
 * Updates workload identity provider attribute condition for new repo
 */
async function updateWorkloadIdentityProvider(
  ctx: MigrateRepoContext,
): Promise<void> {
  await confirmAndRun(
    `gcloud iam workload-identity-pools providers update-oidc ${ctx.providerName} --project=${ctx.projectId} --location=global --workload-identity-pool=${ctx.poolName} --attribute-condition="attribute.repository_owner=='${ctx.newOwner}' && attribute.repository=='${ctx.newRepo}'"`,
    `Update Workload Identity Provider to use new repo '${ctx.newRepo}'`,
  );
}

/**
 * Removes old service account binding from workload identity pool
 */
async function removeOldServiceAccountBinding(
  ctx: MigrateRepoContext,
): Promise<void> {
  const oldPrincipalSet = `principalSet://iam.googleapis.com/projects/${ctx.projectNumber}/locations/global/workloadIdentityPools/${ctx.poolName}/attribute.repository/${ctx.oldRepo}`;
  await confirmAndRun(
    `gcloud iam service-accounts remove-iam-policy-binding ${ctx.saEmail} --project=${ctx.projectId} --role="roles/iam.workloadIdentityUser" --member="${oldPrincipalSet}"`,
    `Remove old binding for '${ctx.oldRepo}' from Service Account`,
  );
}

/**
 * Adds new service account binding to workload identity pool
 */
async function addNewServiceAccountBinding(
  ctx: MigrateRepoContext,
): Promise<void> {
  const newPrincipalSet = `principalSet://iam.googleapis.com/projects/${ctx.projectNumber}/locations/global/workloadIdentityPools/${ctx.poolName}/attribute.repository/${ctx.newRepo}`;
  await confirmAndRun(
    `gcloud iam service-accounts add-iam-policy-binding ${ctx.saEmail} --project=${ctx.projectId} --role="roles/iam.workloadIdentityUser" --member="${newPrincipalSet}"`,
    `Add new binding for '${ctx.newRepo}' to Service Account`,
  );
}

/**
 * Migrates workload identity bindings when repository is moved.
 *
 * Steps are executed in order to minimize risk:
 * 1. Update provider - This changes which repo can authenticate
 * 2. Add new binding - New repo can impersonate service account
 * 3. Remove old binding - Clean up old repo access
 *
 * If a step fails, the process exits with an error. The user can
 * manually resume from the failed step or run the command again.
 */
export async function migrateRepoBindings(
  ctx: MigrateRepoContext,
): Promise<void> {
  p.log.info(`Using Project Number: ${ctx.projectNumber}`);
  p.log.info(`Migrating from: ${ctx.oldRepo}`);
  p.log.info(`Migrating to: ${ctx.newRepo}`);

  p.log.warn(`
If any step fails, the system may be in an inconsistent state.
You can run individual gcloud commands manually to complete the migration.
Each step will ask for confirmation before executing.
`);

  // 1. Update the workload identity provider attribute condition
  p.log.step("Step 1: Update Workload Identity Provider");
  await updateWorkloadIdentityProvider(ctx);

  // 2. Add new service account binding (add first before removing old)
  p.log.step("Step 2: Add new Service Account binding");
  await addNewServiceAccountBinding(ctx);

  // 3. Remove old service account binding
  p.log.step("Step 3: Remove old Service Account binding");
  await removeOldServiceAccountBinding(ctx);

  p.outro("Repository migration complete!");
  p.log.info(`
GitHub Actions workflows in '${ctx.newRepo}' can now authenticate to Google Cloud.
No changes needed to your workflow files - they use the same provider and service account.
`);
}

/**
 * Main setup orchestration
 */
export async function setupGitHubActions(ctx: SetupContext): Promise<void> {
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

  // 8. Grant access to all database secrets
  await grantSecretAccessToAllDatabaseSecrets(ctx);

  // 9. Grant access to all JWT secrets
  await grantSecretAccessToAllJwtSecrets(ctx);

  // 10. Grant access to all Zero Trust secrets (for internal environment)
  await grantSecretAccessToAllZeroTrustSecrets(ctx);

  p.outro("Setup Complete!");
  displayWorkflowYAML(ctx);
}
