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
    description: "Production environment",
  },
  {
    env: "internal",
    cargoFeature: "env_internal",
    databaseSecret: "database-url-internal",
    jwtSecret: "jwt-secret", // Same secret name as production
    description: "Internal environment (admin role, deploys with prod)",
  },
  {
    env: "nightly",
    cargoFeature: "env_nightly",
    databaseSecret: "database-url", // Uses production database
    jwtSecret: "jwt-secret", // Same secret name as production
    description: "Nightly build environment",
  },
  {
    env: "test",
    cargoFeature: "env_test",
    databaseSecret: "database-url-test",
    jwtSecret: null, // Uses default local secret
    description: "Test environment",
  },
  {
    env: "test-internal",
    cargoFeature: "env_test_internal",
    databaseSecret: "database-url-test-internal",
    jwtSecret: null, // Uses default local secret
    description: "Test-internal environment (admin role, deploys with main)",
  },
  {
    env: "pr",
    cargoFeature: "env_pr",
    databaseSecret: "database-url-pr",
    jwtSecret: "jwt-secret-pr",
    description: "Pull request environment",
  },
  {
    env: "local",
    cargoFeature: null, // Local uses default (no feature)
    databaseSecret: "database-url-local",
    jwtSecret: null, // Uses default local secret
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
