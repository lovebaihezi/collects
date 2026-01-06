import * as p from "@clack/prompts";
import { $ } from "bun";
import { randomBytes } from "crypto";
import { confirmAndRun, checkResource } from "./utils.ts";

/**
 * JWT secret names for each environment
 */
export const JWT_SECRETS = {
  production: "jwt-secret", // Used by prod, internal, nightly
  pr: "jwt-secret-pr", // Used by PR environment
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
 * Generates a cryptographically secure random string for JWT secret
 * Uses Node.js crypto module which is available in Bun
 */
function generateJwtSecret(): string {
  return randomBytes(32).toString("base64");
}

/**
 * Updates a GCP secret value with a new version
 */
async function updateSecretValue(
  projectId: string,
  secretName: string,
  value: string,
): Promise<void> {
  try {
    await $`echo -n ${value} | gcloud secrets versions add ${secretName} --project=${projectId} --data-file=-`.quiet();
    p.log.success(`Secret '${secretName}' updated successfully`);
  } catch (err) {
    p.log.error(`Failed to update secret '${secretName}'`);
    throw err;
  }
}

/**
 * Sets up JWT secrets in Google Cloud Secret Manager
 * Generates random secrets automatically
 */
export async function setupJwtSecrets(projectId: string): Promise<void> {
  p.log.info(`Setting up JWT secrets in project: ${projectId}`);

  // Create secrets if they don't exist
  for (const [, secretName] of Object.entries(JWT_SECRETS)) {
    await createSecretIfNotExists(projectId, secretName);
  }

  // Generate and update secret values
  for (const [envName, secretName] of Object.entries(JWT_SECRETS)) {
    const secret = generateJwtSecret();
    p.log.info(`Generated JWT secret for ${envName} environment`);

    const shouldUpdate = await p.confirm({
      message: `Update secret '${secretName}' with the generated value?`,
    });

    if (p.isCancel(shouldUpdate) || !shouldUpdate) {
      p.log.warn(`Skipped updating secret '${secretName}'`);
      continue;
    }

    await updateSecretValue(projectId, secretName, secret);
  }

  p.log.success("JWT secrets have been set up successfully!");
  displayJwtUsageInstructions();
}

/**
 * Displays instructions for using JWT secrets
 */
function displayJwtUsageInstructions(): void {
  const instructions = `
# JWT Secret Configuration Complete!

The following secrets have been created/updated in Google Cloud Secret Manager:
- ${JWT_SECRETS.production} (for prod, internal, nightly environments)
- ${JWT_SECRETS.pr} (for PR environment)

## Environment Mapping

| Environment   | Secret Name        |
|---------------|-------------------|
| prod          | ${JWT_SECRETS.production} |
| internal      | ${JWT_SECRETS.production} |
| nightly       | ${JWT_SECRETS.production} |
| pr            | ${JWT_SECRETS.pr} |
| test          | (uses default)    |
| test-internal | (uses default)    |
| local         | (uses default)    |

## Accessing Secrets Locally

\`\`\`bash
export JWT_SECRET=$(gcloud secrets versions access latest --secret=${JWT_SECRETS.production})
\`\`\`

## Note

JWT secrets are automatically included in Cloud Run deployments via the
\`just services::gcloud-deploy\` command for environments that require them.
`;

  console.log(instructions);
}

/**
 * Lists all JWT secrets and their status
 */
export async function listJwtSecrets(projectId: string): Promise<void> {
  p.log.info(`Checking JWT secrets in project: ${projectId}`);

  for (const [key, secretName] of Object.entries(JWT_SECRETS)) {
    const exists = await checkSecretExists(projectId, secretName);
    const status = exists ? "✓ exists" : "✗ not found";
    p.log.info(`${key} (${secretName}): ${status}`);
  }
}
