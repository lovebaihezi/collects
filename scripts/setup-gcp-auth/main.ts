#!/usr/bin/env bun
import { cac } from 'cac';

const cli = cac('setup-gcp-auth');

cli
  .command('', 'Setup Google Cloud Authentication for GitHub Actions')
  .option('--project-id <id>', 'Google Cloud Project ID')
  .option('--repo <repo>', 'GitHub Repository (owner/repo)')
  .option('--service-account <name>', 'Service Account Name', { default: 'github-actions-deployer' })
  .option('--pool <name>', 'Workload Identity Pool Name', { default: 'github-actions-pool' })
  .option('--provider <name>', 'Workload Identity Provider Name', { default: 'github-actions-provider' })
  .action(async (options) => {
    let { projectId, repo, serviceAccount, pool, provider } = options;

    if (!projectId) {
      console.error('Error: --project-id is required');
      process.exit(1);
    }
    if (!repo) {
      console.error('Error: --repo is required (e.g., owner/repo)');
      process.exit(1);
    }

    console.log(`\n🔹 Setting up GCP Auth for project: ${projectId}, repo: ${repo}\n`);

    const commands = [
      `# 1. Create Service Account`,
      `gcloud iam service-accounts create ${serviceAccount} --project "${projectId}" --display-name="GitHub Actions Deployer" || true`,
      ``,
      `# 2. Create Workload Identity Pool`,
      `gcloud iam workload-identity-pools create ${pool} --project "${projectId}" --location="global" --display-name="GitHub Actions Pool" || true`,
      ``,
      `# 3. Create Workload Identity Provider`,
      `gcloud iam workload-identity-pools providers create-oidc ${provider} --project "${projectId}" --location="global" --workload-identity-pool="${pool}" --display-name="GitHub Actions Provider" --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository" --issuer-uri="https://token.actions.githubusercontent.com" || true`,
      ``,
      `# 4. Get Pool ID`,
      `POOL_ID=$(gcloud iam workload-identity-pools describe ${pool} --project "${projectId}" --location="global" --format="value(name)")`,
      ``,
      `# 5. Allow GitHub Actions to Impersonate Service Account`,
      `gcloud iam service-accounts add-iam-policy-binding "${serviceAccount}@${projectId}.iam.gserviceaccount.com" --project "${projectId}" --role="roles/iam.workloadIdentityUser" --member="principalSet://iam.googleapis.com/\${POOL_ID}/attribute.repository/${repo}"`,
      ``,
      `# 6. Grant Permissions`,
      `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccount}@${projectId}.iam.gserviceaccount.com" --role="roles/artifactregistry.writer"`,
      `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccount}@${projectId}.iam.gserviceaccount.com" --role="roles/run.admin"`,
      `gcloud projects add-iam-policy-binding "${projectId}" --member="serviceAccount:${serviceAccount}@${projectId}.iam.gserviceaccount.com" --role="roles/iam.serviceAccountUser"`,
    ];

    console.log('--- Run the following commands in your terminal ---');
    console.log(commands.join('\n'));
    console.log('\n---------------------------------------------------\n');

    // Calculate values for GitHub Secrets
    // We assume the pool ID format based on inputs
    // The correct full resource name for provider is: projects/{project_number}/locations/global/workloadIdentityPools/{pool}/providers/{provider}
    // We need project number.

    console.log('Fetching Project Number...');
    const proc = Bun.spawnSync(['gcloud', 'projects', 'describe', projectId, '--format=value(projectNumber)']);
    const projectNumber = proc.stdout.toString().trim();

    if (!projectNumber) {
        console.error("Failed to fetch project number. Make sure you are authenticated with gcloud.");
    } else {
        const providerResourceName = `projects/${projectNumber}/locations/global/workloadIdentityPools/${pool}/providers/${provider}`;
        const serviceAccountEmail = `${serviceAccount}@${projectId}.iam.gserviceaccount.com`;

        console.log('✅ Setup Instructions Generated.');
        console.log('\n👇 Add these secrets to your GitHub Repository:');
        console.log('---------------------------------------------------');
        console.log(`Secret Name:  GCP_WORKLOAD_IDENTITY_PROVIDER`);
        console.log(`Secret Value: ${providerResourceName}`);
        console.log('---------------------------------------------------');
        console.log(`Secret Name:  GCP_SERVICE_ACCOUNT`);
        console.log(`Secret Value: ${serviceAccountEmail}`);
        console.log('---------------------------------------------------');
    }
  });

cli.help();
cli.parse();
