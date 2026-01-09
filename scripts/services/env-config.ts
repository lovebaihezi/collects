/**
 * Centralized environment configuration for the Collects project.
 * This file is the single source of truth for environment-related mappings.
 *
 * Usage in justfiles:
 *   - `bun run main.ts env-feature <env>` - Get cargo feature flags
 *   - `bun run main.ts env-secret <env>` - Get database secret name
 *
 * Available environments: prod, internal, nightly, test, test-internal, pr, local
 */

import { type } from "arktype";

/**
 * R2 storage secrets configuration
 */
export interface R2SecretsConfig {
  /** CF Account ID secret name */
  accountId: string;
  /** CF Access Key ID secret name */
  accessKeyId: string;
  /** CF Secret Access Key secret name */
  secretAccessKey: string;
  /** CF Bucket secret name */
  bucket: string;
}

/**
 * Zero Trust secrets configuration
 */
export interface ZeroTrustSecretsConfig {
  /** CF Access Team Domain secret name */
  teamDomain: string;
  /** CF Access Audience secret name */
  aud: string;
}

/**
 * Default Zero Trust secrets (for internal environments)
 */
const DEFAULT_ZERO_TRUST_SECRETS: ZeroTrustSecretsConfig = {
  teamDomain: "cf-access-team-domain",
  aud: "cf-access-aud",
};

/**
 * Default R2 secrets (shared across most environments)
 */
const DEFAULT_R2_SECRETS: R2SecretsConfig = {
  accountId: "cf-account-id",
  accessKeyId: "cf-access-key-id",
  secretAccessKey: "cf-secret-access-key",
  bucket: "cf-bucket",
};

/**
 * Environment configuration type
 */
export interface EnvConfig {
  /** Environment name (e.g., "prod", "test", "pr") */
  env: string;
  /** Cargo feature flag for this environment (e.g., "env_pr") */
  cargoFeature: string | null;
  /** Database secret name in Google Cloud Secret Manager */
  databaseSecret: string;
  /** JWT secret name in Google Cloud Secret Manager (null = uses default local secret) */
  jwtSecret: string | null;
  /** R2 storage secrets (null = not required, e.g., local/test) */
  r2Secrets: R2SecretsConfig | null;
  /** Zero Trust secrets (null = not required, only for internal environments) */
  zeroTrustSecrets: ZeroTrustSecretsConfig | null;
  /** Description of the environment */
  description: string;
}

/**
 * All environment configurations.
 * This is the single source of truth for environment mappings.
 */
export const ENV_CONFIGS: EnvConfig[] = [
  {
    env: "prod",
    cargoFeature: null, // Production uses default (no feature)
    databaseSecret: "database-url",
    jwtSecret: "jwt-secret",
    r2Secrets: DEFAULT_R2_SECRETS,
    zeroTrustSecrets: null, // Not an internal environment
    description: "Production environment",
  },
  {
    env: "internal",
    cargoFeature: "env_internal",
    databaseSecret: "database-url-internal",
    jwtSecret: "jwt-secret", // Same secret name as production
    r2Secrets: DEFAULT_R2_SECRETS,
    zeroTrustSecrets: DEFAULT_ZERO_TRUST_SECRETS, // Required for internal
    description: "Internal environment (admin role, deploys with prod)",
  },
  {
    env: "nightly",
    cargoFeature: "env_nightly",
    databaseSecret: "database-url", // Uses production database
    jwtSecret: "jwt-secret", // Same secret name as production
    r2Secrets: DEFAULT_R2_SECRETS,
    zeroTrustSecrets: null, // Not an internal environment
    description: "Nightly build environment",
  },
  {
    env: "test",
    cargoFeature: "env_test",
    databaseSecret: "database-url-test",
    jwtSecret: null, // Uses default local secret
    r2Secrets: null, // R2 not required for test
    zeroTrustSecrets: null, // Not an internal environment
    description: "Test environment",
  },
  {
    env: "test-internal",
    cargoFeature: "env_test_internal",
    databaseSecret: "database-url-test-internal",
    jwtSecret: null, // Uses default local secret
    r2Secrets: null, // R2 not required for test-internal
    zeroTrustSecrets: DEFAULT_ZERO_TRUST_SECRETS, // Required for internal
    description: "Test-internal environment (admin role, deploys with main)",
  },
  {
    env: "pr",
    cargoFeature: "env_pr",
    databaseSecret: "database-url-pr",
    jwtSecret: "jwt-secret-pr",
    r2Secrets: DEFAULT_R2_SECRETS,
    zeroTrustSecrets: null, // Not an internal environment
    description: "Pull request environment",
  },
  {
    env: "local",
    cargoFeature: null, // Local uses default (no feature)
    databaseSecret: "database-url-local",
    jwtSecret: null, // Uses default local secret
    r2Secrets: null, // R2 not required for local
    zeroTrustSecrets: null, // Not an internal environment
    description: "Local development environment",
  },
];

/**
 * Get environment configuration by name
 */
export function getEnvConfig(env: string): EnvConfig | undefined {
  return ENV_CONFIGS.find((c) => c.env === env);
}

/**
 * Get cargo feature flag for an environment
 * Returns the feature flag (e.g., "--features env_pr") or empty string
 */
export function getCargoFeature(env: string): string {
  const config = getEnvConfig(env);
  if (!config) {
    console.error(`Unknown environment: ${env}`);
    console.error(
      `Available environments: ${ENV_CONFIGS.map((c) => c.env).join(", ")}`,
    );
    process.exit(1);
  }

  if (config.cargoFeature) {
    return `--features ${config.cargoFeature}`;
  }
  return "";
}

/**
 * Get database secret name for an environment
 */
export function getDatabaseSecret(env: string): string {
  const config = getEnvConfig(env);
  if (!config) {
    console.error(`Unknown environment: ${env}`);
    console.error(
      `Available environments: ${ENV_CONFIGS.map((c) => c.env).join(", ")}`,
    );
    process.exit(1);
  }
  return config.databaseSecret;
}

/**
 * Get JWT secret name for an environment
 * Returns the secret name or empty string if the environment uses default local secret
 */
export function getJwtSecret(env: string): string {
  const config = getEnvConfig(env);
  if (!config) {
    console.error(`Unknown environment: ${env}`);
    console.error(
      `Available environments: ${ENV_CONFIGS.map((c) => c.env).join(", ")}`,
    );
    process.exit(1);
  }
  return config.jwtSecret ?? "";
}

/**
 * Get R2 secrets configuration for an environment
 * Returns the R2 secrets config or null if R2 is not required
 */
export function getR2Secrets(env: string): R2SecretsConfig | null {
  const config = getEnvConfig(env);
  if (!config) {
    console.error(`Unknown environment: ${env}`);
    console.error(
      `Available environments: ${ENV_CONFIGS.map((c) => c.env).join(", ")}`,
    );
    process.exit(1);
  }
  return config.r2Secrets;
}

/**
 * Check if an environment requires R2 secrets
 */
export function requiresR2Secrets(env: string): boolean {
  return getR2Secrets(env) !== null;
}

/**
 * Validate environment name
 */
export const envNameType = type(
  "'prod' | 'internal' | 'nightly' | 'test' | 'test-internal' | 'pr' | 'local' | ''",
);

export type EnvName =
  | "prod"
  | "internal"
  | "nightly"
  | "test"
  | "test-internal"
  | "pr"
  | "local"
  | "";

/**
 * List all available environment names
 */
export function listEnvironments(): string[] {
  return ENV_CONFIGS.map((c) => c.env);
}
