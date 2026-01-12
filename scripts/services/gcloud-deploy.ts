/**
 * Cloud Run deployment script for Collects services.
 *
 * This script handles deploying the services Docker image to Google Cloud Run
 * with the appropriate secrets and configuration for each environment.
 *
 * Usage:
 *   bun run main.ts gcloud-deploy <env> <image_tag>
 *
 * Examples:
 *   bun run main.ts gcloud-deploy prod v2026.1.3
 *   bun run main.ts gcloud-deploy pr pr-123
 *   bun run main.ts gcloud-deploy test main-abc123
 */

import * as p from "@clack/prompts";
import { $ } from "bun";
import { getEnvConfig, type EnvConfig } from "./env-config.ts";

const GCP_REGION = "us-east1";
const REPOSITORY_NAME = "collects-services";
const IMAGE_NAME = "collects-services";

export interface DeployConfig {
  env: string;
  imageTag: string;
  projectId: string;
  serviceName: string;
  fullImageName: string;
  secrets: SecretBinding[];
}

export interface SecretBinding {
  envVar: string;
  secretName: string;
  version: string;
}

/**
 * Get the current GCP project ID
 */
async function getProjectId(): Promise<string> {
  const result = await $`gcloud config get-value project`.quiet();
  return result.text().trim();
}

/**
 * Determine the Cloud Run service name based on environment
 */
function getServiceName(env: string): string {
  if (env === "prod") {
    return "collects-services";
  }
  return `collects-services-${env}`;
}

/**
 * Build the full Docker image name
 */
function getFullImageName(projectId: string, imageTag: string): string {
  return `${GCP_REGION}-docker.pkg.dev/${projectId}/${REPOSITORY_NAME}/${IMAGE_NAME}:${imageTag}`;
}

/**
 * Build secret bindings for an environment
 */
function buildSecretBindings(envConfig: EnvConfig): SecretBinding[] {
  const secrets: SecretBinding[] = [];

  // Database URL is always required
  secrets.push({
    envVar: "DATABASE_URL",
    secretName: envConfig.databaseSecret,
    version: "latest",
  });

  // JWT secret (if required for this environment)
  if (envConfig.jwtSecret) {
    secrets.push({
      envVar: "JWT_SECRET",
      secretName: envConfig.jwtSecret,
      version: "latest",
    });
  }

  // R2 storage secrets (if required for this environment)
  const r2Secrets = envConfig.r2Secrets;
  if (r2Secrets) {
    secrets.push({
      envVar: "CF_ACCOUNT_ID",
      secretName: r2Secrets.accountId,
      version: "latest",
    });
    secrets.push({
      envVar: "CF_ACCESS_KEY_ID",
      secretName: r2Secrets.accessKeyId,
      version: "latest",
    });
    secrets.push({
      envVar: "CF_SECRET_ACCESS_KEY",
      secretName: r2Secrets.secretAccessKey,
      version: "latest",
    });
    secrets.push({
      envVar: "CF_BUCKET",
      secretName: r2Secrets.bucket,
      version: "latest",
    });
  }

  // Zero Trust secrets (if required for this environment - internal environments)
  const zeroTrustSecrets = envConfig.zeroTrustSecrets;
  if (zeroTrustSecrets) {
    secrets.push({
      envVar: "CF_ACCESS_TEAM_DOMAIN",
      secretName: zeroTrustSecrets.teamDomain,
      version: "latest",
    });
    secrets.push({
      envVar: "CF_ACCESS_AUD",
      secretName: zeroTrustSecrets.aud,
      version: "latest",
    });
  }

  return secrets;
}

/**
 * Format secrets for gcloud --set-secrets flag
 * Only outputs env var names, never secret values
 */
function formatSecretsArg(secrets: SecretBinding[]): string {
  return secrets
    .map((s) => `${s.envVar}=${s.secretName}:${s.version}`)
    .join(",");
}

/**
 * Get a summary of configured secrets (for logging, no sensitive data)
 */
function getSecretsSummary(secrets: SecretBinding[]): string {
  const categories: string[] = [];

  if (secrets.some((s) => s.envVar === "DATABASE_URL")) {
    categories.push("DATABASE_URL");
  }
  if (secrets.some((s) => s.envVar === "JWT_SECRET")) {
    categories.push("JWT_SECRET");
  }
  if (
    secrets.some(
      (s) => s.envVar.startsWith("CF_") && !s.envVar.startsWith("CF_ACCESS"),
    )
  ) {
    categories.push("R2 storage");
  }
  if (secrets.some((s) => s.envVar.startsWith("CF_ACCESS"))) {
    categories.push("Zero Trust");
  }

  return categories.join(", ");
}

/**
 * Build deployment configuration
 */
export async function buildDeployConfig(
  env: string,
  imageTag: string,
): Promise<DeployConfig> {
  const envConfig = getEnvConfig(env);
  if (!envConfig) {
    throw new Error(
      `Unknown environment: ${env}. Valid environments: prod, internal, nightly, test, test-internal, pr, local`,
    );
  }

  const projectId = await getProjectId();
  const serviceName = getServiceName(env);
  const fullImageName = getFullImageName(projectId, imageTag);
  const secrets = buildSecretBindings(envConfig);

  return {
    env,
    imageTag,
    projectId,
    serviceName,
    fullImageName,
    secrets,
  };
}

/**
 * Execute the Cloud Run deployment
 */
export async function deploy(config: DeployConfig): Promise<void> {
  const { env, projectId, serviceName, fullImageName, secrets } = config;

  p.log.info(`Deploying to Cloud Run...`);
  p.log.info(`  Service: ${serviceName}`);
  p.log.info(`  Image: ${fullImageName}`);
  p.log.info(`  Region: ${GCP_REGION}`);
  p.log.info(`  Secrets: ${getSecretsSummary(secrets)}`);

  const secretsArg = formatSecretsArg(secrets);

  const s = p.spinner();
  s.start(`Deploying ${serviceName}...`);

  try {
    await $`gcloud run deploy ${serviceName} \
      --image ${fullImageName} \
      --region ${GCP_REGION} \
      --platform managed \
      --allow-unauthenticated \
      --startup-probe httpGet.path=/is-health,httpGet.port=8080,initialDelaySeconds=1,timeoutSeconds=3,periodSeconds=3,failureThreshold=3 \
      --set-env-vars ENV=${env},GCP_PROJECT_ID=${projectId} \
      --set-secrets ${secretsArg}`.quiet();

    s.stop(`Successfully deployed ${serviceName}`);
    p.log.success(`Deployment complete!`);
  } catch (err) {
    s.stop(`Failed to deploy ${serviceName}`);

    let errorMessage = "Unknown error";
    if (err instanceof $.ShellError) {
      errorMessage = err.stderr.toString();
    } else if (err instanceof Error) {
      errorMessage = err.message;
    }

    p.log.error(`Deployment failed: ${errorMessage}`);
    throw err;
  }
}

/**
 * Main entry point for gcloud-deploy command
 */
export async function runGcloudDeploy(
  env: string,
  imageTag: string,
): Promise<void> {
  p.intro(`Cloud Run Deployment: ${env}`);

  try {
    const config = await buildDeployConfig(env, imageTag);
    await deploy(config);
    p.outro("Deployment successful!");
  } catch (err) {
    if (err instanceof Error) {
      p.log.error(err.message);
    }
    process.exit(1);
  }
}
