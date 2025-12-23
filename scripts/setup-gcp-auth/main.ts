#!/usr/bin/env bun
import { cac } from 'cac';
import prompts from 'prompts';
import { $ } from 'bun';
import { type } from 'arktype';

const cli = cac('setup-gcp-auth');

// --- Schemas & Types ---

// Base definition for Global Options
const GlobalOptionsSchema = type({
  "projectId?": "string",
  "repo?": "string",
  // serviceAccount has a default in CAC, so it will be present
  "serviceAccount": "string",
});

// Since spreading .json might be unstable or untyped, let's use intersection or re-definition.
// Re-definition is safest for simple schemas to avoid TS complexity in this script.

const AuthOptionsSchema = type({
  "projectId?": "string",
  "repo?": "string",
  "serviceAccount": "string",
  "pool": "string",
  "provider": "string",
});

const SecretOptionsSchema = type({
  "projectId?": "string",
  "repo?": "string",
  "serviceAccount": "string",
  "secretKey?": "string",
  "secretValue?": "string",
});

const SetupOptionsSchema = type({
    "projectId?": "string",
    "repo?": "string",
    "serviceAccount": "string",
    "pool?": "string",
    "provider?": "string",
    "databaseUrl?": "string",
    "secretKey?": "string",
    "secretValue?": "string",
});

type GlobalOptions = typeof GlobalOptionsSchema.infer;
type AuthOptions = typeof AuthOptionsSchema.infer;
type SecretOptions = typeof SecretOptionsSchema.infer;
type SetupOptions = typeof SetupOptionsSchema.infer;

// --- Helpers ---

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
    await $`sh -c ${command}`;
    console.log('✅ Success');
  } catch (err: any) {
    if (ignoreFailure) {
      console.log('⚠️  Command failed, but proceeding as requested (ignoreFailure=true).');
      return;
    }

    console.error('\n❌ Command Failed!');
    console.error(`Error Output: ${err.stderr?.toString() || err.message}`);
    process.exit(1);
  }
}

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
  const parsed = AuthOptionsSchema.assert(options);
  const { projectId, repo, serviceAccount, pool, provider } = parsed as Required<AuthOptions>;
  const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

  console.log(`\n🚀 Starting Auth Setup for ${projectId} (Repo: ${repo})`);

  await askToRun(
    `gcloud iam service-accounts create ${serviceAccount} --project "${projectId}" --display-name="GitHub Actions Deployer" || echo "Service account likely exists"`,
    'Create Service Account (idempotent)',
    true
  );

  await askToRun(
    `gcloud iam workload-identity-pools create ${pool} --project "${projectId}" --location="global" --display-name="GitHub Actions Pool" || echo "Pool likely exists"`,
    'Create Workload Identity Pool (idempotent)',
    true
  );

  await askToRun(
    `gcloud iam workload-identity-pools providers create-oidc ${provider} --project "${projectId}" --location="global" --workload-identity-pool="${pool}" --display-name="GitHub Actions Provider" --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" --issuer-uri="https://token.actions.githubusercontent.com" || echo "Provider likely exists"`,
    'Create Workload Identity Provider (idempotent)',
    true
  );

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

  await askToRun(
    `gcloud iam service-accounts add-iam-policy-binding "${serviceAccountEmail}" --project "${projectId}" --role="roles/iam.workloadIdentityUser" --member="principalSet://iam.googleapis.com/${poolId}/attribute.repository/${repo}"`,
    'Bind Workload Identity to Service Account'
  );

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

  console.log('\n✅ Auth Setup Actions Completed.');
  console.log('Calculating secret values...');

  const projectNum = (await $`gcloud projects describe ${projectId} --format="value(projectNumber)"`.text()).trim();
  const providerResourceName = `projects/${projectNum}/locations/global/workloadIdentityPools/${pool}/providers/${provider}`;

  console.log('\n👇 Update these GitHub Secrets:');
  console.log(`GCP_WORKLOAD_IDENTITY_PROVIDER: ${providerResourceName}`);
  console.log(`GCP_SERVICE_ACCOUNT:            ${serviceAccountEmail}`);
}

async function setupSecrets(options: SecretOptions) {
  const parsed = SecretOptionsSchema.assert(options);
  const { projectId, serviceAccount } = parsed as Required<SecretOptions>;
  let { secretKey, secretValue } = parsed;

  console.log(`\n🚀 Starting Secret Setup for ${projectId}`);

  const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

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

  if (!secretValue) {
    const response = await prompts({
      type: 'password',
      name: 'secretValue',
      message: `Enter the value for ${secretKey}:`,
    });
    secretValue = response.secretValue;
  }

  if (!secretValue) {
    console.error(`Error: Value for ${secretKey} is required.`);
    process.exit(1);
  }

  const gcpSecretName = secretKey.toLowerCase().replace(/_/g, '-');

  await askToRun(
    `gcloud secrets create ${gcpSecretName} --project "${projectId}" --replication-policy="automatic" || echo "Secret likely exists"`,
    `Create Secret '${gcpSecretName}'`,
    true
  );

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

  await askToRun(
    `gcloud secrets add-iam-policy-binding ${gcpSecretName} --project "${projectId}" --member="serviceAccount:${serviceAccountEmail}" --role="roles/secretmanager.secretAccessor"`,
    `Grant Service Account access to '${gcpSecretName}'`
  );

  console.log('\n✅ Secret Setup Completed.');
}

// --- CLI Definitions ---

const sharedOptions = (c: any) => {
  c.option('--project-id <id>', 'Google Cloud Project ID')
   .option('--repo <repo>', 'GitHub Repository (owner/repo)')
   .option('--service-account <name>', 'Service Account Name', { default: 'github-actions-deployer' });
};

const authCmd = cli.command('auth', 'Setup only Workload Identity')
  .option('--pool <name>', 'Workload Identity Pool Name', { default: 'github-actions-pool' })
  .option('--provider <name>', 'Workload Identity Provider Name', { default: 'github-actions-provider' });

sharedOptions(authCmd);

authCmd.action(async (options: AuthOptions) => {
  const ctx = await ensureContext(options);
  await setupAuth(ctx);
});

const secretCmd = cli.command('secrets', 'Setup a Secret (e.g. DATABASE_URL)')
  .option('--secret-key <key>', 'Secret Key (e.g. DATABASE_URL)')
  .option('--secret-value <val>', 'Secret Value')
  .option('--database-url <url>', 'Alias for --secret-value when key is DATABASE_URL');

sharedOptions(secretCmd);

secretCmd.action(async (options: SecretOptions & { databaseUrl?: string }) => {
  const ctx = await ensureContext(options);
  if (options.databaseUrl && !options.secretValue) {
    options.secretValue = options.databaseUrl;
    if (!options.secretKey) {
       options.secretKey = 'DATABASE_URL';
    }
  }
  await setupSecrets(ctx);
});

const setupCmd = cli.command('setup', 'Setup Everything (Auth + DATABASE_URL)')
  .option('--pool <name>', 'Workload Identity Pool Name', { default: 'github-actions-pool' })
  .option('--provider <name>', 'Workload Identity Provider Name', { default: 'github-actions-provider' })
  .option('--database-url <url>', 'Database URL');

sharedOptions(setupCmd);

setupCmd.action(async (options: SetupOptions) => {
  const ctx = await ensureContext(options);
  // Default values for auth (cac provided)
  const authOpts: AuthOptions = {
      ...ctx,
      pool: options.pool || 'github-actions-pool',
      provider: options.provider || 'github-actions-provider'
  };
  await setupAuth(authOpts);

  if (options.databaseUrl) {
      await setupSecrets({
          ...ctx,
          secretKey: 'DATABASE_URL',
          secretValue: options.databaseUrl,
      });
  } else {
      console.log('\n❓ Do you want to configure DATABASE_URL now?');
      const res = await prompts({ type: 'confirm', name: 'yes', message: 'Configure DATABASE_URL?', initial: true });
      if (res.yes) {
           await setupSecrets({
               ...ctx,
               secretKey: 'DATABASE_URL',
               secretValue: undefined,
           });
      }
  }
});

cli.help();
cli.parse();
