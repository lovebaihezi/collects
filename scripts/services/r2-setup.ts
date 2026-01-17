import * as p from "@clack/prompts";
import { createHash, createHmac } from "crypto";
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

// Legacy/typo secret names that may exist in older projects.
const LEGACY_R2_SECRETS = {
  // Typo: "acess" (missing 'c')
  secretAccessKey: "cf-secret-acess-key",
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
  const shouldUpdate = await p.confirm({
    message: `Update secret '${secretName}' value? (value hidden)`,
  });

  if (p.isCancel(shouldUpdate) || !shouldUpdate) {
    p.log.warn(`Skipped updating secret '${secretName}'`);
    return;
  }

  const s = p.spinner();
  s.start(`Updating secret '${secretName}'...`);

  const proc = Bun.spawn({
    cmd: [
      "gcloud",
      "secrets",
      "versions",
      "add",
      secretName,
      `--project=${projectId}`,
      "--data-file=-",
    ],
    stdin: new Blob([value]),
    stdout: "pipe",
    stderr: "pipe",
  });

  const exitCode = await proc.exited;
  if (exitCode !== 0) {
    const stderr = proc.stderr ? await new Response(proc.stderr).text() : "";
    s.stop(`Failed to update secret '${secretName}'`);
    p.log.error(stderr.trim() || `gcloud exited with code ${exitCode}`);
    process.exit(1);
  }

  s.stop(`Secret '${secretName}' updated`);
}

function sha256Hex(data: string): string {
  return createHash("sha256").update(data).digest("hex");
}

function hmacSha256(key: Buffer | string, data: string): Buffer {
  return createHmac("sha256", key).update(data).digest();
}

function toAmzDate(date: Date): string {
  return date.toISOString().replace(/[:-]|\.\d{3}/g, "");
}

function getSignatureKey(
  secretAccessKey: string,
  dateStamp: string,
  region: string,
  service: string,
): Buffer {
  const kDate = hmacSha256(`AWS4${secretAccessKey}`, dateStamp);
  const kRegion = hmacSha256(kDate, region);
  const kService = hmacSha256(kRegion, service);
  return hmacSha256(kService, "aws4_request");
}

async function readSecretValue(
  projectId: string,
  secretName: string,
): Promise<string> {
  const proc = Bun.spawn({
    cmd: [
      "gcloud",
      "secrets",
      "versions",
      "access",
      "latest",
      `--secret=${secretName}`,
      `--project=${projectId}`,
    ],
    stdout: "pipe",
    stderr: "pipe",
  });

  const exitCode = await proc.exited;
  if (exitCode !== 0) {
    const stderr = proc.stderr ? await new Response(proc.stderr).text() : "";
    p.log.error(stderr.trim() || `gcloud exited with code ${exitCode}`);
    process.exit(1);
  }

  const value = proc.stdout ? await new Response(proc.stdout).text() : "";
  return value.trim();
}

async function verifyR2Credentials(credentials: R2Credentials): Promise<void> {
  const { accountId, accessKeyId, secretAccessKey, bucket } = credentials;
  const region = "auto";
  const service = "s3";
  const host = `${accountId}.r2.cloudflarestorage.com`;
  const url = `https://${host}/${bucket}`;

  const now = new Date();
  const amzDate = toAmzDate(now);
  const dateStamp = amzDate.slice(0, 8);
  const payloadHash = sha256Hex("");
  const canonicalHeaders =
    `host:${host}\n` +
    `x-amz-content-sha256:${payloadHash}\n` +
    `x-amz-date:${amzDate}\n`;
  const signedHeaders = "host;x-amz-content-sha256;x-amz-date";
  const canonicalRequest = [
    "HEAD",
    `/${bucket}`,
    "",
    canonicalHeaders,
    signedHeaders,
    payloadHash,
  ].join("\n");
  const stringToSign = [
    "AWS4-HMAC-SHA256",
    amzDate,
    `${dateStamp}/${region}/${service}/aws4_request`,
    sha256Hex(canonicalRequest),
  ].join("\n");
  const signingKey = getSignatureKey(
    secretAccessKey,
    dateStamp,
    region,
    service,
  );
  const signature = createHmac("sha256", signingKey)
    .update(stringToSign)
    .digest("hex");
  const authorization = [
    "AWS4-HMAC-SHA256 Credential=",
    `${accessKeyId}/${dateStamp}/${region}/${service}/aws4_request`,
    `, SignedHeaders=${signedHeaders}, Signature=${signature}`,
  ].join("");

  const response = await fetch(url, {
    method: "HEAD",
    headers: {
      "x-amz-content-sha256": payloadHash,
      "x-amz-date": amzDate,
      Authorization: authorization,
    },
  });

  if (response.ok) {
    p.log.success(`R2 verification succeeded for bucket '${bucket}'.`);
    return;
  }

  const body = await response.text().catch(() => "");
  p.log.error(
    `R2 verification failed (HTTP ${response.status}). Check bucket name and permissions.`,
  );
  if (body.trim()) {
    p.log.message(body.trim());
  }
  process.exit(1);
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

  const shouldVerify = await p.confirm({
    message: "Verify R2 credentials by sending a signed HEAD request now?",
  });

  if (!p.isCancel(shouldVerify) && shouldVerify) {
    await verifyR2Credentials(credentials);
  }
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

## Verify Access

\`\`\`bash
just scripts::r2-verify --project-id <project-id>
\`\`\`
`;

  console.log(instructions);
}

/**
 * Verifies R2 access by reading secrets from GCP and signing a HEAD request.
 */
export async function verifyR2Secrets(projectId: string): Promise<void> {
  p.log.info(`Verifying R2 credentials for project: ${projectId}`);

  const missing: string[] = [];
  for (const [, secretName] of Object.entries(R2_SECRETS)) {
    const exists = await checkSecretExists(projectId, secretName);
    if (!exists) {
      missing.push(secretName);
    }
  }

  if (missing.length > 0) {
    p.log.error("Missing required R2 secrets:");
    for (const secretName of missing) {
      p.log.message(`- ${secretName}`);
    }
    process.exit(1);
  }

  const credentials: R2Credentials = {
    accountId: await readSecretValue(projectId, R2_SECRETS.accountId),
    accessKeyId: await readSecretValue(projectId, R2_SECRETS.accessKeyId),
    secretAccessKey: await readSecretValue(
      projectId,
      R2_SECRETS.secretAccessKey,
    ),
    bucket: await readSecretValue(projectId, R2_SECRETS.bucket),
  };

  if (
    !credentials.accountId ||
    !credentials.accessKeyId ||
    !credentials.secretAccessKey ||
    !credentials.bucket
  ) {
    p.log.error("One or more R2 secrets are empty.");
    process.exit(1);
  }

  await verifyR2Credentials(credentials);
}

/**
 * Lists all R2 secrets and their status
 */
export async function listR2Secrets(projectId: string): Promise<void> {
  p.log.info(`Checking R2 secrets in project: ${projectId}`);

  for (const [key, secretName] of Object.entries(R2_SECRETS)) {
    const exists = await checkSecretExists(projectId, secretName);
    if (!exists) {
      p.log.info(`${key} (${secretName}): ✗ not found`);
      continue;
    }

    const hasValue = await checkResource(
      `gcloud secrets versions access latest --secret=${secretName} --project=${projectId}`,
    );
    const status = hasValue
      ? "✓ exists (has value)"
      : "⚠ exists (no value or no access)";
    p.log.info(`${key} (${secretName}): ${status}`);
  }

  const legacySecretExists = await checkSecretExists(
    projectId,
    LEGACY_R2_SECRETS.secretAccessKey,
  );
  const correctSecretExists = await checkSecretExists(
    projectId,
    R2_SECRETS.secretAccessKey,
  );
  if (legacySecretExists && !correctSecretExists) {
    p.log.warn(
      `Found legacy secret '${LEGACY_R2_SECRETS.secretAccessKey}'. The scripts expect '${R2_SECRETS.secretAccessKey}'.`,
    );
  }
}
