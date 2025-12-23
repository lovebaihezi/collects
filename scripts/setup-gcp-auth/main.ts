#!/usr/bin/env bun
import { cac } from 'cac';
import prompts from 'prompts';
import { $ } from 'bun';
import { z } from 'zod';

const cli = cac('setup-gcp-auth');

// --- Schemas & Types ---

const GlobalOptionsSchema = z.object({
  projectId: z.string().optional(),
  repo: z.string().optional(),
  serviceAccount: z.string().default('github-actions-deployer'),
});

const AuthOptionsSchema = GlobalOptionsSchema.extend({
  pool: z.string().default('github-actions-pool'),
  provider: z.string().default('github-actions-provider'),
});

const SecretOptionsSchema = GlobalOptionsSchema.extend({
  secretKey: z.string().optional(),
  secretValue: z.string().optional(),
});

type GlobalOptions = z.infer<typeof GlobalOptionsSchema>;
type AuthOptions = z.infer<typeof AuthOptionsSchema>;
type SecretOptions = z.infer<typeof SecretOptionsSchema>;

// --- Helpers ---

/**
 * Runs a command with user confirmation and error handling.
 * @param command The shell command string to run.
 * @param description A brief description of what this command does.
 * @param ignoreFailure If true, will not throw on failure (useful for "check if exists" commands).
 */
async function askToRun(command: string, description: string, ignoreFailure = false) {
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
async function ensureContext<T extends GlobalOptions>(options: T): Promise<T> {
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
  // Validate options
  const parsed = AuthOptionsSchema.parse(options);
  const { projectId, repo, serviceAccount, pool, provider } = parsed as Required<AuthOptions>;
  const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

  console.log(`\n🚀 Starting Auth Setup for ${projectId} (Repo: ${repo})`);

  // 1. Create Service Account
  await askToRun(
    `gcloud iam service-accounts create ${serviceAccount} --project "${projectId}" --display-name="GitHub Actions Deployer" || echo "Service account likely exists"`,
    'Create Service Account (idempotent)',
    true
  );

  // 2. Create Workload Identity Pool
  await askToRun(
    `gcloud iam workload-identity-pools create ${pool} --project "${projectId}" --location="global" --display-name="GitHub Actions Pool" || echo "Pool likely exists"`,
    'Create Workload Identity Pool (idempotent)',
    true
  );

  // 3. Create Workload Identity Provider
  await askToRun(
    `gcloud iam workload-identity-pools providers create-oidc ${provider} --project "${projectId}" --location="global" --workload-identity-pool="${pool}" --display-name="GitHub Actions Provider" --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" --issuer-uri="https://token.actions.githubusercontent.com" || echo "Provider likely exists"`,
    'Create Workload Identity Provider (idempotent)',
    true
  );

  // 4. Get Pool ID
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
  await askToRun(
    `gcloud iam service-accounts add-iam-policy-binding "${serviceAccountEmail}" --project "${projectId}" --role="roles/iam.workloadIdentityUser" --member="principalSet://iam.googleapis.com/${poolId}/attribute.repository/${repo}"`,
    'Bind Workload Identity to Service Account'
  );

  // 6. Grant Permissions
  await askToRun(
    `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/artifactregistry.writer"`,
    'Grant Artifact Registry Writer'
  );
  await askToRun(
    `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/run.admin"`,
    'Grant Cloud Run Admin'
  );
  await askToRun(
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
  // Validate options
  const parsed = SecretOptionsSchema.parse(options);
  const { projectId, serviceAccount } = parsed as Required<SecretOptions>; // global options are ensured by ensureContext but schema check helps
  let { secretKey, secretValue } = parsed;

  console.log(`\n🚀 Starting Secret Setup for ${projectId}`);

  const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

  // Prompt for Secret Key if missing
  if (!secretKey) {
    const response = await prompts({
      type: 'text',
      name: 'secretKey',
      message: 'Enter the Secret Key (e.g. DATABASE_URL):',
      initial: 'DATABASE_URL'
    });
    secretKey = response.secretKey;
  }

  if (!secretKey) {
    console.error('Error: Secret Key is required.');
    process.exit(1);
  }

  // Prompt for Secret Value if missing
  if (!secretValue) {
    const response = await prompts({
      type: 'password', // Mask input
      name: 'secretValue',
      message: `Enter the value for ${secretKey}:`,
    });
    secretValue = response.secretValue;
  }

  if (!secretValue) {
    console.error(`Error: Value for ${secretKey} is required.`);
    process.exit(1);
  }

  // Derive GCP Secret Name (lowercase)
  const gcpSecretName = secretKey.toLowerCase().replace(/_/g, '-'); // Google secrets usually prefer hyphens or valid chars

  // 1. Create Secret (idempotent-ish)
  await askToRun(
    `gcloud secrets create ${gcpSecretName} --project "${projectId}" --replication-policy="automatic" || echo "Secret likely exists"`,
    `Create Secret '${gcpSecretName}'`,
    true
  );

  // 2. Add Secret Version
  console.log(`\n🔹 [Step] Add new version to secret '${gcpSecretName}'`);
  const confirm = await prompts({
    type: 'confirm',
    name: 'execute',
    message: 'Execute this command? (Values will be piped securely)',
    initial: true,
  });

  if (confirm.execute) {
    try {
      const proc = Bun.spawn(['sh', '-c', `gcloud secrets versions add ${gcpSecretName} --project "${projectId}" --data-file=-`], {
        stdin: 'pipe',
      });
      if (proc.stdin) {
        proc.stdin.write(secretValue);
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
  await askToRun(
    `gcloud secrets add-iam-policy-binding ${gcpSecretName} --project "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/secretmanager.secretAccessor"`,
    `Grant Service Account access to '${gcpSecretName}'`
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
  await setupAuth(ctx);
});

// 2. Secrets Command
const secretCmd = cli.command('secrets', 'Setup a Secret (e.g. DATABASE_URL)')
  .option('--secret-key <key>', 'Secret Key (e.g. DATABASE_URL)')
  .option('--secret-value <val>', 'Secret Value')
  // Aliases for backward compat / ease of use
  .option('--database-url <url>', 'Alias for --secret-value when key is DATABASE_URL');

sharedOptions(secretCmd);

secretCmd.action(async (options: any) => {
  const ctx = await ensureContext(options);
  // normalization
  if (options.databaseUrl && !options.secretValue) {
    options.secretKey = 'DATABASE_URL';
    options.secretValue = options.databaseUrl;
  }
  await setupSecrets(ctx);
});

// 3. Setup Command (Default/All)
const setupCmd = cli.command('setup', 'Setup Everything (Auth + DATABASE_URL)')
  .option('--pool <name>', 'Workload Identity Pool Name', { default: 'github-actions-pool' })
  .option('--provider <name>', 'Workload Identity Provider Name', { default: 'github-actions-provider' })
  .option('--database-url <url>', 'Database URL');

sharedOptions(setupCmd);

setupCmd.action(async (options: any) => {
  const ctx = await ensureContext(options);
  await setupAuth(ctx);

  // For 'setup', we default to DATABASE_URL if provided or prompt for it
  if (options.databaseUrl) {
      await setupSecrets({
          ...ctx,
          secretKey: 'DATABASE_URL',
          secretValue: options.databaseUrl,
          secretName: 'DATABASE_URL' // Unused in new logic but types might match
      });
  } else {
      console.log('\n❓ Do you want to configure DATABASE_URL now?');
      const res = await prompts({ type: 'confirm', name: 'yes', message: 'Configure DATABASE_URL?', initial: true });
      if (res.yes) {
           await setupSecrets({
               ...ctx,
               secretKey: 'DATABASE_URL',
               secretValue: undefined,
               secretName: 'DATABASE_URL'
           });
      }
  }
});

cli.help();
cli.parse();
