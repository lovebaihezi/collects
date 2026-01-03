import * as p from "@clack/prompts";
import { confirmAndRun, checkResource } from "./utils.ts";

/**
 * Context for R2 storage setup
 */
export interface R2SetupContext {
  projectId: string;
  bucketName: string;
  cfAccountId: string;
}

/**
 * Cloudflare R2 credentials
 */
export interface R2Credentials {
  accountId: string;
  accessKeyId: string;
  secretAccessKey: string;
  bucket: string;
}

/**
 * Secret names for R2 configuration
 */
export const R2_SECRETS = {
  accountId: "cf-account-id",
  accessKeyId: "cf-access-key-id",
  secretAccessKey: "cf-secret-access-key",
  bucket: "cf-bucket",
} as const;

/**
 * Checks if a GCP secret exists
 */
async function checkSecretExists(
  projectId: string,
  secretName: string,
): Promise<boolean> {
  return checkResource(
    `gcloud secrets describe ${secretName} --project=${projectId}`,
  );
}

/**
 * Creates a GCP secret if it doesn't exist
 */
async function createSecretIfNotExists(
  projectId: string,
  secretName: string,
): Promise<void> {
  const exists = await checkSecretExists(projectId, secretName);
  if (!exists) {
    await confirmAndRun(
      `gcloud secrets create ${secretName} --project=${projectId} --replication-policy=automatic`,
      `Create secret '${secretName}'`,
    );
  } else {
    p.log.info(`Secret '${secretName}' already exists.`);
  }
}

/**
 * Updates a GCP secret value
 */
async function updateSecretValue(
  projectId: string,
  secretName: string,
  value: string,
): Promise<void> {
  // Use printf to handle special characters properly
  await confirmAndRun(
    `printf '%s' '${value}' | gcloud secrets versions add ${secretName} --project=${projectId} --data-file=-`,
    `Update secret '${secretName}' value`,
  );
}

/**
 * Prompts for R2 credentials interactively
 */
export async function promptForR2Credentials(): Promise<R2Credentials | null> {
  const result = await p.group(
    {
      accountId: () =>
        p.text({
          message: "Enter your Cloudflare Account ID:",
          placeholder: "your-cloudflare-account-id",
          validate: (value) => {
            if (!value) return "Account ID is required";
          },
        }),
      accessKeyId: () =>
        p.text({
          message: "Enter your R2 Access Key ID:",
          placeholder: "your-access-key-id",
          validate: (value) => {
            if (!value) return "Access Key ID is required";
          },
        }),
      secretAccessKey: () =>
        p.password({
          message: "Enter your R2 Secret Access Key:",
          validate: (value) => {
            if (!value) return "Secret Access Key is required";
          },
        }),
      bucket: () =>
        p.text({
          message: "Enter your R2 Bucket Name:",
          placeholder: "collects-files",
          validate: (value) => {
            if (!value) return "Bucket name is required";
          },
        }),
    },
    {
      onCancel: () => {
        p.cancel("Operation cancelled.");
        return null;
      },
    },
  );

  return result as R2Credentials;
}

/**
 * Sets up R2 secrets in Google Cloud Secret Manager
 */
export async function setupR2Secrets(
  projectId: string,
  credentials: R2Credentials,
): Promise<void> {
  p.log.info(`Setting up R2 secrets in project: ${projectId}`);

  // Create secrets if they don't exist
  for (const [, secretName] of Object.entries(R2_SECRETS)) {
    await createSecretIfNotExists(projectId, secretName);
  }

  // Update secret values
  await updateSecretValue(
    projectId,
    R2_SECRETS.accountId,
    credentials.accountId,
  );
  await updateSecretValue(
    projectId,
    R2_SECRETS.accessKeyId,
    credentials.accessKeyId,
  );
  await updateSecretValue(
    projectId,
    R2_SECRETS.secretAccessKey,
    credentials.secretAccessKey,
  );
  await updateSecretValue(projectId, R2_SECRETS.bucket, credentials.bucket);

  p.log.success("R2 secrets have been set up successfully!");

  displayR2UsageInstructions();
}

/**
 * Displays instructions for using R2 secrets
 */
function displayR2UsageInstructions(): void {
  const instructions = `
# R2 Configuration Complete!

The following secrets have been created/updated in Google Cloud Secret Manager:
- ${R2_SECRETS.accountId}
- ${R2_SECRETS.accessKeyId}
- ${R2_SECRETS.secretAccessKey}
- ${R2_SECRETS.bucket}

## Using R2 Secrets in Cloud Run

Add these to your Cloud Run deployment:

\`\`\`bash
gcloud run deploy collects-services \\
  --set-secrets "CF_ACCOUNT_ID=${R2_SECRETS.accountId}:latest" \\
  --set-secrets "CF_ACCESS_KEY_ID=${R2_SECRETS.accessKeyId}:latest" \\
  --set-secrets "CF_SECRET_ACCESS_KEY=${R2_SECRETS.secretAccessKey}:latest" \\
  --set-secrets "CF_BUCKET=${R2_SECRETS.bucket}:latest"
\`\`\`

## Accessing Secrets Locally

\`\`\`bash
export CF_ACCOUNT_ID=$(gcloud secrets versions access latest --secret=${R2_SECRETS.accountId})
export CF_ACCESS_KEY_ID=$(gcloud secrets versions access latest --secret=${R2_SECRETS.accessKeyId})
export CF_SECRET_ACCESS_KEY=$(gcloud secrets versions access latest --secret=${R2_SECRETS.secretAccessKey})
export CF_BUCKET=$(gcloud secrets versions access latest --secret=${R2_SECRETS.bucket})
\`\`\`
`;

  console.log(instructions);
}

/**
 * Lists all R2 secrets and their status
 */
export async function listR2Secrets(projectId: string): Promise<void> {
  p.log.info(`Checking R2 secrets in project: ${projectId}`);

  for (const [key, secretName] of Object.entries(R2_SECRETS)) {
    const exists = await checkSecretExists(projectId, secretName);
    const status = exists ? "✓ exists" : "✗ not found";
    p.log.info(`${key} (${secretName}): ${status}`);
  }
}
