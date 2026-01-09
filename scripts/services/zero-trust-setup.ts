import * as p from "@clack/prompts";
import { confirmAndRun, checkResource } from "./utils.ts";

/**
 * Zero Trust secret names for Cloudflare Access configuration
 */
export const ZERO_TRUST_SECRETS = {
  teamDomain: "cf-access-team-domain",
  audience: "cf-access-aud",
} as const;

/**
 * Cloudflare Zero Trust credentials
 */
export interface ZeroTrustCredentials {
  teamDomain: string;
  audience: string;
}

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
 * Prompts for Zero Trust credentials interactively
 */
export async function promptForZeroTrustCredentials(): Promise<ZeroTrustCredentials | null> {
  const result = await p.group(
    {
      teamDomain: () =>
        p.text({
          message: "Enter your Cloudflare Access Team Domain:",
          placeholder: "myteam.cloudflareaccess.com",
          validate: (value) => {
            if (!value) return "Team domain is required";
            if (!value.includes("."))
              return "Team domain should be a valid domain (e.g., myteam.cloudflareaccess.com)";
          },
        }),
      audience: () =>
        p.text({
          message:
            "Enter your Cloudflare Access Application Audience (AUD) tag:",
          placeholder: "your-application-audience-tag",
          validate: (value) => {
            if (!value) return "Audience tag is required";
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

  return result as ZeroTrustCredentials;
}

/**
 * Sets up Zero Trust secrets in Google Cloud Secret Manager
 */
export async function setupZeroTrustSecrets(
  projectId: string,
  credentials: ZeroTrustCredentials,
): Promise<void> {
  p.log.info(`Setting up Zero Trust secrets in project: ${projectId}`);

  // Create secrets if they don't exist
  for (const [, secretName] of Object.entries(ZERO_TRUST_SECRETS)) {
    await createSecretIfNotExists(projectId, secretName);
  }

  // Update secret values
  await updateSecretValue(
    projectId,
    ZERO_TRUST_SECRETS.teamDomain,
    credentials.teamDomain,
  );
  await updateSecretValue(
    projectId,
    ZERO_TRUST_SECRETS.audience,
    credentials.audience,
  );

  p.log.success("Zero Trust secrets have been set up successfully!");

  displayZeroTrustUsageInstructions();
}

/**
 * Displays instructions for using Zero Trust secrets
 */
function displayZeroTrustUsageInstructions(): void {
  const instructions = `
# Zero Trust Configuration Complete!

The following secrets have been created/updated in Google Cloud Secret Manager:
- ${ZERO_TRUST_SECRETS.teamDomain}
- ${ZERO_TRUST_SECRETS.audience}

## Environment Usage

| Environment   | Zero Trust Required |
|---------------|---------------------|
| internal      | ✓ Required          |
| test-internal | ✓ Required          |
| prod          | Optional            |
| nightly       | Optional            |
| test          | Not used            |
| pr            | Not used            |
| local         | Not used            |

## Using Zero Trust Secrets in Cloud Run

These secrets are automatically included in Cloud Run deployments for the
\`internal\` and \`test-internal\` environments via the \`just services::gcloud-deploy\` command.

For manual deployment:

\`\`\`bash
gcloud run deploy collects-services \\
  --set-secrets "CF_ACCESS_TEAM_DOMAIN=${ZERO_TRUST_SECRETS.teamDomain}:latest" \\
  --set-secrets "CF_ACCESS_AUD=${ZERO_TRUST_SECRETS.audience}:latest"
\`\`\`

## Accessing Secrets Locally

\`\`\`bash
export CF_ACCESS_TEAM_DOMAIN=$(gcloud secrets versions access latest --secret=${ZERO_TRUST_SECRETS.teamDomain})
export CF_ACCESS_AUD=$(gcloud secrets versions access latest --secret=${ZERO_TRUST_SECRETS.audience})
\`\`\`

## Cloudflare Access Setup

1. Go to Cloudflare Zero Trust dashboard (https://one.dash.cloudflare.com)
2. Navigate to Access > Applications
3. Create a new Application (Self-hosted)
4. Configure Access policies (e.g., allow specific emails/groups)
5. Note the Application Audience (AUD) tag from the application settings
6. Your team domain is shown at the top (e.g., myteam.cloudflareaccess.com)
`;

  console.log(instructions);
}

/**
 * Lists all Zero Trust secrets and their status
 */
export async function listZeroTrustSecrets(projectId: string): Promise<void> {
  p.log.info(`Checking Zero Trust secrets in project: ${projectId}`);

  for (const [key, secretName] of Object.entries(ZERO_TRUST_SECRETS)) {
    const exists = await checkSecretExists(projectId, secretName);
    const status = exists ? "✓ exists" : "✗ not found";
    p.log.info(`${key} (${secretName}): ${status}`);
  }
}
