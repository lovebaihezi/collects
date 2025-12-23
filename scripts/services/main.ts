#!/usr/bin/env bun
import { cac } from 'cac';
import * as p from '@clack/prompts';
import { $ } from 'bun';
import { type } from 'arktype';

const cli = cac('services');

/**
 * Runs a shell command with error handling and LLM prompt generation.
 */
async function runCommand(command: string, context: string) {
  try {
    // We use Bun.spawn to have better control or just use $ if simple
    // Using $ from bun as imported. We capture stdout to keep the UI clean.
    const { stdout } = await $`${{ raw: command }}`;
    return stdout.toString();
  } catch (err: any) {
    p.log.error(`COMMAND FAILED: ${command}`);

    let errorOutput = '';

    // ShellError is not exported from 'bun' in the current version, so we check the name/properties
    if (err.name === 'ShellError' || (err.stdout && err.stderr)) {
      errorOutput = err.stdout.toString() + err.stderr.toString();
    } else {
      errorOutput = err.message || String(err);
    }

    p.log.error(`ERROR: ${errorOutput.trim()}`);

    const llmPrompt = `
I ran the command \`${command}\` to ${context} and got this error:
\`\`\`
${errorOutput.trim()}
\`\`\`
How do I fix this in Google Cloud?
`;

    p.note(llmPrompt, 'PROMPT FOR LLM');
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
    message: 'Do you want to run this command?',
  });

  if (p.isCancel(shouldRun) || !shouldRun) {
    p.log.warn('Operation cancelled by user.');
    process.exit(0);
  }

  await runCommand(command, context);
  p.log.success('Command executed successfully.');
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

cli.command('actions-setup', 'Setup GitHub Actions with Google Cloud Workload Identity Federation')
  .action(async () => {
    p.intro('GitHub Actions + Google Cloud Workload Identity Federation Setup');

    const projectGroup = await p.group({
      projectId: () => p.text({
        message: 'Enter your Google Cloud Project ID:',
        placeholder: 'my-gcp-project-id',
        validate: (value) => {
          if (!value) return 'Project ID is required';
        }
      }),
      repo: () => p.text({
        message: 'Enter your GitHub Repository (owner/repo):',
        placeholder: 'username/repository',
        validate: (value) => {
          if (!value) return 'Repository is required';
          if (!value.includes('/')) return 'Format must be owner/repo';
        }
      })
    }, {
      onCancel: () => {
        p.cancel('Operation cancelled.');
        process.exit(0);
      }
    });

    const projectId = projectGroup.projectId;
    const repo = projectGroup.repo;

    // Validate repo format using ArkType
    const repoType = type(/^[^/]+\/[^/]+$/);
    const result = repoType(repo);

    if (result instanceof type.errors) {
      p.log.error(`Invalid repository format: ${result.summary}`);
      process.exit(1);
    }

    const owner = result.split('/')[0];

    const poolName = 'github-actions-pool';
    const providerName = 'github-provider';
    const saName = 'github-actions-sa';
    const saEmail = `${saName}@${projectId}.iam.gserviceaccount.com`;
    const poolId = `projects/${projectId}/locations/global/workloadIdentityPools/${poolName}`;
    const providerId = `${poolId}/providers/${providerName}`;

    // 1. Enable IAM Credentials API
    // We check if it's enabled by trying to describe it or just ensuring it's enabled.
    // Simpler to just attempt enabling or check.
    // Let's assume we confirm enabling.
    await confirmAndRun(
      `gcloud services enable iamcredentials.googleapis.com --project ${projectId}`,
      'Enable IAM Credentials API'
    );

    // 2. Create Workload Identity Pool
    const poolExists = await checkResource(`gcloud iam workload-identity-pools describe ${poolName} --project=${projectId} --location=global`);
    if (!poolExists) {
      await confirmAndRun(
        `gcloud iam workload-identity-pools create ${poolName} --project=${projectId} --location=global --display-name="GitHub Actions Pool"`,
        'Create Workload Identity Pool'
      );
    } else {
      p.log.info(`Workload Identity Pool '${poolName}' already exists.`);
    }

    // 3. Create Workload Identity Provider
    const providerExists = await checkResource(`gcloud iam workload-identity-pools providers describe ${providerName} --workload-identity-pool=${poolName} --project=${projectId} --location=global`);
    if (!providerExists) {
      await confirmAndRun(
        `gcloud iam workload-identity-pools providers create-oidc ${providerName} --project=${projectId} --location=global --workload-identity-pool=${poolName} --display-name="GitHub Provider" --attribute-mapping="google.subject=assertion.sub,attribute.actor=assertion.actor,attribute.repository=assertion.repository,attribute.repository_owner=assertion.repository_owner" --issuer-uri="https://token.actions.githubusercontent.com" --attribute-condition="attribute.repository_owner=='${owner}' && attribute.repository=='${repo}'"`,
        'Create Workload Identity Provider'
      );
    } else {
      p.log.info(`Workload Identity Provider '${providerName}' already exists.`);
    }

    // 4. Create Service Account
    const saExists = await checkResource(`gcloud iam service-accounts describe ${saEmail} --project=${projectId}`);
    if (!saExists) {
      await confirmAndRun(
        `gcloud iam service-accounts create ${saName} --project=${projectId} --display-name="GitHub Actions Service Account"`,
        'Create Service Account'
      );
    } else {
      p.log.info(`Service Account '${saName}' already exists.`);
    }

    // 5. Bind Service Account to Pool (Allow GitHub Actions to impersonate this SA)
    // We specifically allow the specific repository
    const principalSet = `principalSet://iam.googleapis.com/${poolId}/attribute.repository/${repo}`;
    // It's hard to check exact binding existence easily without parsing JSON.
    // Running this multiple times is generally safe/idempotent (it just updates policy).
    await confirmAndRun(
      `gcloud iam service-accounts add-iam-policy-binding ${saEmail} --project=${projectId} --role="roles/iam.workloadIdentityUser" --member="${principalSet}"`,
      `Allow GitHub Repo '${repo}' to impersonate Service Account`
    );

    // 6. Grant Roles to Service Account
    const roles = [
      'roles/artifactregistry.writer',
      'roles/secretmanager.secretAccessor',
      'roles/run.admin',
      'roles/iam.serviceAccountUser'
    ];

    for (const role of roles) {
      await confirmAndRun(
        `gcloud projects add-iam-policy-binding ${projectId} --member="serviceAccount:${saEmail}" --role="${role}"`,
        `Grant '${role}' to Service Account`
      );
    }

    p.outro('Setup Complete!');

    const yamlOutput = `
# Add this to your GitHub Actions workflow:

- id: 'auth'
  name: 'Authenticate to Google Cloud'
  uses: 'google-github-actions/auth@v2'
  with:
    workload_identity_provider: '${providerId}'
    service_account: '${saEmail}'
`;

    // Using console.log specifically for the copy-paste block
    console.log(yamlOutput);
  });

cli.command('', 'Show help')
  .action(() => {
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
\`\`\`

---
Run \`bun run main.ts --help\` for CLI details.
`;
    console.log(helpText);
  });

cli.help();
cli.parse();
