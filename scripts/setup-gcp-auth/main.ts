#!/usr/bin/env bun
import { cac } from 'cac';
import prompts from 'prompts';
import { $ } from 'bun';

const cli = cac('setup-gcp-auth');

// --- Types ---
interface GlobalOptions {
  projectId?: string;
  repo?: string;
  serviceAccount: string;
}

interface AuthOptions extends GlobalOptions {
  pool: string;
  provider: string;
}

interface SecretOptions extends GlobalOptions {
  databaseUrl?: string;
  secretName: string;
}

// --- Helpers ---

/**
 * Runs a command with user confirmation and error handling.
 * @param command The shell command string to run.
 * @param description A brief description of what this command does.
 * @param ignoreFailure If true, will not throw on failure (useful for "check if exists" commands).
 */
async function runCommand(command: string, description: string, ignoreFailure = false) {
  console.log(`\n🔹 [Step] ${description}`);
  console.log(`   Command: ${command}`);

  const response = await prompts({
    type: 'confirm',
    name: 'execute',
    message: 'Execute this command?',
    initial: true,
  });

  if (!response.execute) {
    console.log('Skipping...');
    return;
  }

  try {
    // We use sh -c to allow complex shell features like pipes or ||
    await $`sh -c ${command}`;
    console.log('✅ Success');
  } catch (err: any) {
    if (ignoreFailure) {
      console.log('⚠️  Command failed, but proceeding as requested (ignoreFailure=true).');
      return;
    }

    console.error('\n❌ Command Failed!');
    console.error(`Error Output: ${err.stderr?.toString() || err.message}`);
    console.error(`\n--- LLM Debug Helper ---`);
    console.error(`The command '${command}' failed.`);
    console.error(`Error details: ${err.stderr?.toString() || err.stdout?.toString() || err.message}`);
    console.error(`Please analyze why this Google Cloud SDK command failed. Common issues include:`);
    console.error(`- Insufficient permissions for the active user.`);
    console.error(`- The resource already exists (if not handled).`);
    console.error(`- Incorrect Project ID or arguments.`);
    console.error(`------------------------\n`);
    process.exit(1);
  }
}

/**
 * Ensures projectId and repo are available, prompting if necessary.
 */
async function ensureContext(options: GlobalOptions): Promise<GlobalOptions> {
  let { projectId, repo } = options;

  if (!projectId || !repo) {
    const response = await prompts([
      {
        type: projectId ? null : 'text',
        name: 'projectId',
        message: 'Google Cloud Project ID:',
        initial: projectId,
      },
      {
        type: repo ? null : 'text',
        name: 'repo',
        message: 'GitHub Repository (owner/repo):',
        initial: repo,
      },
    ]);

    if (!projectId) projectId = response.projectId;
    if (!repo) repo = response.repo;
  }

  if (!projectId || !repo) {
    console.error('Error: --project-id and --repo are required.');
    process.exit(1);
  }

  return { ...options, projectId, repo };
}

// --- Logic ---

async function setupAuth(options: AuthOptions) {
  const { projectId, repo, serviceAccount, pool, provider } = options as Required<AuthOptions>;
  const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

  console.log(`\n🚀 Starting Auth Setup for ${projectId} (Repo: ${repo})`);

  // 1. Create Service Account
  await runCommand(
    `gcloud iam service-accounts create ${serviceAccount} --project "${projectId}" --display-name="GitHub Actions Deployer" || echo "Service account likely exists"`,
    'Create Service Account (idempotent)',
    true // It might fail if exists, but the || echo handles it usually. strict fail if syntax is wrong though.
  );

  // 2. Create Workload Identity Pool
  await runCommand(
    `gcloud iam workload-identity-pools create ${pool} --project "${projectId}" --location="global" --display-name="GitHub Actions Pool" || echo "Pool likely exists"`,
    'Create Workload Identity Pool (idempotent)',
    true
  );

  // 3. Create Workload Identity Provider
  await runCommand(
    `gcloud iam workload-identity-pools providers create-oidc ${provider} --project "${projectId}" --location="global" --workload-identity-pool="${pool}" --display-name="GitHub Actions Provider" --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" --issuer-uri="https://token.actions.githubusercontent.com" || echo "Provider likely exists"`,
    'Create Workload Identity Provider (idempotent)',
    true
  );

  // 4. Get Pool ID (Need this for binding)
  // We need to capture output here, runCommand doesn't return it. We'll do a custom run for this.
  console.log(`\n🔹 [Step] Fetching Pool ID...`);
  let poolId = '';
  try {
    const poolIdCmd = `gcloud iam workload-identity-pools describe ${pool} --project "${projectId}" --location="global" --format="value(name)"`;
    const out = await $`sh -c ${poolIdCmd}`.text();
    poolId = out.trim();
    console.log(`   Pool ID: ${poolId}`);
  } catch (err: any) {
    console.error(`❌ Failed to get Pool ID. Is the pool created?`);
    console.error(err.stderr?.toString());
    process.exit(1);
  }

  // 5. Allow GitHub Actions to Impersonate Service Account
  // Note: We need to extract the raw pool ID part if poolId is the full name, but the binding usually expects the full resource name or specific format.
  // The original script used: principalSet://iam.googleapis.com/${POOL_ID}/attribute.repository/${repo}
  // 'gcloud describe ... --format="value(name)"' returns: projects/NUMBER/locations/global/workloadIdentityPools/POOL
  // The IAM binding requires that exact format in the principalSet.

  await runCommand(
    `gcloud iam service-accounts add-iam-policy-binding "${serviceAccountEmail}" --project "${projectId}" --role="roles/iam.workloadIdentityUser" --member="principalSet://iam.googleapis.com/${poolId}/attribute.repository/${repo}"`,
    'Bind Workload Identity to Service Account'
  );

  // 6. Grant Permissions
  await runCommand(
    `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/artifactregistry.writer"`,
    'Grant Artifact Registry Writer'
  );
  await runCommand(
    `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/run.admin"`,
    'Grant Cloud Run Admin'
  );
  await runCommand(
    `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/iam.serviceAccountUser"`,
    'Grant Service Account User'
  );

  // 7. Output Secrets
  console.log('\n✅ Auth Setup Actions Completed.');
  console.log('Calculating secret values...');

  // Get Project Number
  const projectNum = (await $`gcloud projects describe ${projectId} --format="value(projectNumber)"`.text()).trim();
  const providerResourceName = `projects/${projectNum}/locations/global/workloadIdentityPools/${pool}/providers/${provider}`;

  console.log('\n👇 Update these GitHub Secrets:');
  console.log(`GCP_WORKLOAD_IDENTITY_PROVIDER: ${providerResourceName}`);
  console.log(`GCP_SERVICE_ACCOUNT:            ${serviceAccountEmail}`);
}

async function setupSecrets(options: SecretOptions) {
  const { projectId, serviceAccount, secretName } = options as Required<SecretOptions>;
  let { databaseUrl } = options;

  console.log(`\n🚀 Starting Secret Setup for ${projectId}`);

  const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

  if (!databaseUrl) {
    const response = await prompts({
      type: 'password', // Mask input
      name: 'databaseUrl',
      message: 'Enter the DATABASE_URL:',
    });
    databaseUrl = response.databaseUrl;
  }

  if (!databaseUrl) {
    console.error('Error: DATABASE_URL is required.');
    process.exit(1);
  }

  // 1. Create Secret (idempotent-ish)
  await runCommand(
    `gcloud secrets create ${secretName} --project "${projectId}" --replication-policy="automatic" || echo "Secret likely exists"`,
    `Create Secret '${secretName}'`,
    true
  );

  // 2. Add Secret Version
  // We handle this carefully to pipe the value
  console.log(`\n🔹 [Step] Add new version to secret '${secretName}'`);
  const confirm = await prompts({
    type: 'confirm',
    name: 'execute',
    message: 'Execute this command? (Values will be piped securely)',
    initial: true,
  });

  if (confirm.execute) {
    try {
      // Using printf to avoid newline issues, pipe to gcloud
      const proc = Bun.spawn(['sh', '-c', `gcloud secrets versions add ${secretName} --project "${projectId}" --data-file=-`], {
        stdin: 'pipe',
      });
      if (proc.stdin) {
        proc.stdin.write(databaseUrl);
        proc.stdin.end();
      }
      const exitCode = await proc.exited;
      if (exitCode !== 0) {
        throw new Error(`Process exited with code ${exitCode}`);
      }
      console.log('✅ Success');
    } catch (err: any) {
      console.error('❌ Failed to add secret version.');
      console.error(err);
      process.exit(1);
    }
  } else {
    console.log('Skipping...');
  }

  // 3. Grant Access to Service Account
  await runCommand(
    `gcloud secrets add-iam-policy-binding ${secretName} --project "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/secretmanager.secretAccessor"`,
    `Grant Service Account access to '${secretName}'`
  );

  console.log('\n✅ Secret Setup Completed.');
}

// --- CLI Definitions ---

// Shared options
const sharedOptions = (c: any) => {
  c.option('--project-id <id>', 'Google Cloud Project ID')
   .option('--repo <repo>', 'GitHub Repository (owner/repo)')
   .option('--service-account <name>', 'Service Account Name', { default: 'github-actions-deployer' });
};

// 1. Auth Command
const authCmd = cli.command('auth', 'Setup only Workload Identity')
  .option('--pool <name>', 'Workload Identity Pool Name', { default: 'github-actions-pool' })
  .option('--provider <name>', 'Workload Identity Provider Name', { default: 'github-actions-provider' });

sharedOptions(authCmd);

authCmd.action(async (options: AuthOptions) => {
  const ctx = await ensureContext(options);
  await setupAuth(ctx as Required<AuthOptions>);
});

// 2. Secrets Command
const secretCmd = cli.command('secrets', 'Setup only Secrets (DATABASE_URL)')
  .option('--database-url <url>', 'Database URL (prompts if missing)')
  .option('--secret-name <name>', 'GCP Secret Name', { default: 'DATABASE_URL' });

sharedOptions(secretCmd);

secretCmd.action(async (options: SecretOptions) => {
  const ctx = await ensureContext(options);
  await setupSecrets(ctx as Required<SecretOptions>);
});

// 3. Setup Command (Default/All)
const setupCmd = cli.command('setup', 'Setup Everything (Auth + Secrets)')
  .option('--pool <name>', 'Workload Identity Pool Name', { default: 'github-actions-pool' })
  .option('--provider <name>', 'Workload Identity Provider Name', { default: 'github-actions-provider' })
  .option('--database-url <url>', 'Database URL')
  .option('--secret-name <name>', 'GCP Secret Name', { default: 'DATABASE_URL' });

sharedOptions(setupCmd);

setupCmd.action(async (options: AuthOptions & SecretOptions) => {
  const ctx = await ensureContext(options);
  await setupAuth(ctx as Required<AuthOptions>);
  await setupSecrets(ctx as Required<SecretOptions>);
});

cli.help();
cli.parse();
