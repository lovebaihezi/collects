import {
  createApiClient,
  type Branch,
  type Role,
  type Endpoint,
} from "@neondatabase/api-client";
import * as p from "@clack/prompts";
import { $ } from "bun";
import { confirmAndRun } from "./utils.ts";

/**
 * Environment configuration mapping.
 * - prod: production branch, restricted role (best practice: minimal permissions)
 * - internal: production branch, admin role (for admin tasks/migrations)
 * - test: development branch, default role
 * - pr: development branch, default role
 * - local: development branch, default role (for local development)
 */
interface EnvConfig {
  env: string;
  branchName: "production" | "development";
  secretName: string;
  useAdminRole: boolean;
  description: string;
}

const ENV_CONFIGS: EnvConfig[] = [
  {
    env: "prod",
    branchName: "production",
    secretName: "database-url",
    useAdminRole: false,
    description: "Production environment (restricted role)",
  },
  {
    env: "internal",
    branchName: "production",
    secretName: "database-url-internal",
    useAdminRole: true,
    description: "Internal environment (admin role for migrations)",
  },
  {
    env: "test",
    branchName: "development",
    secretName: "database-url-test",
    useAdminRole: true,
    description: "Test environment",
  },
  {
    env: "pr",
    branchName: "development",
    secretName: "database-url-pr",
    useAdminRole: true,
    description: "PR environment",
  },
  {
    env: "local",
    branchName: "development",
    secretName: "database-url-local",
    useAdminRole: true,
    description: "Local development environment",
  },
];

const RESTRICTED_ROLE_NAME = "app_user";
const ADMIN_ROLE_SUFFIX = "_owner"; // Neon's default admin role pattern: neondb_owner

interface BranchInfo {
  branch: Branch;
  endpoint: Endpoint;
  adminRole: Role;
  restrictedRole?: Role;
}

/**
 * Finds a branch by name from a list of branches
 */
function findBranchByName(
  branches: Branch[],
  name: string,
): Branch | undefined {
  return branches.find((b) => b.name === name);
}

/**
 * Gets the endpoint for a branch
 */
async function getEndpointForBranch(
  client: ReturnType<typeof createApiClient>,
  projectId: string,
  branchId: string,
): Promise<Endpoint> {
  const response = await client.listProjectBranchEndpoints(projectId, branchId);
  const endpoints = response.data.endpoints;

  if (endpoints.length === 0) {
    throw new Error(`No endpoint found for branch ${branchId}`);
  }

  // Return the read-write endpoint
  const rwEndpoint = endpoints.find((e) => e.type === "read_write");
  return rwEndpoint || endpoints[0];
}

/**
 * Gets roles for a branch
 */
async function getRolesForBranch(
  client: ReturnType<typeof createApiClient>,
  projectId: string,
  branchId: string,
): Promise<Role[]> {
  const response = await client.listProjectBranchRoles(projectId, branchId);
  return response.data.roles;
}

/**
 * Creates a restricted role for production use (best practice)
 * Checks if role exists first, if yes - reset password, if no - create it
 */
async function ensureRestrictedRole(
  client: ReturnType<typeof createApiClient>,
  projectId: string,
  branchId: string,
): Promise<Role> {
  // Get all roles to check if restricted role exists
  const roles = await getRolesForBranch(client, projectId, branchId);
  const existing = roles.find((r) => r.name === RESTRICTED_ROLE_NAME);

  if (existing) {
    p.log.info(
      `Restricted role '${RESTRICTED_ROLE_NAME}' already exists, resetting password...`,
    );
    // Reset password to get a fresh one
    const resetResponse = await client.resetProjectBranchRolePassword(
      projectId,
      branchId,
      RESTRICTED_ROLE_NAME,
    );
    return resetResponse.data.role;
  }

  p.log.info(`Creating restricted role '${RESTRICTED_ROLE_NAME}'...`);
  const response = await client.createProjectBranchRole(projectId, branchId, {
    role: {
      name: RESTRICTED_ROLE_NAME,
    },
  });
  return response.data.role;
}

/**
 * Ensures admin role exists and returns it with a fresh password
 * Admin role should always exist (Neon creates it by default), but we check just in case
 */
async function ensureAdminRole(
  client: ReturnType<typeof createApiClient>,
  projectId: string,
  branchId: string,
  branchName: string,
): Promise<Role> {
  // Get all roles
  const roles = await getRolesForBranch(client, projectId, branchId);

  // Find admin role (ends with _owner)
  const adminRole = roles.find((r) => r.name.endsWith(ADMIN_ROLE_SUFFIX));

  if (!adminRole) {
    p.log.error(
      `No admin role (*${ADMIN_ROLE_SUFFIX}) found on branch '${branchName}'`,
    );
    p.log.info(`Available roles: ${roles.map((r) => r.name).join(", ")}`);
    throw new Error(`Admin role not found on branch '${branchName}'`);
  }

  p.log.info(`Found admin role '${adminRole.name}', resetting password...`);

  // Reset password to get a fresh one
  const response = await client.resetProjectBranchRolePassword(
    projectId,
    branchId,
    adminRole.name,
  );
  return response.data.role;
}

/**
 * Builds a PostgreSQL connection URL
 */
function buildConnectionUrl(
  host: string,
  role: Role,
  database: string = "neondb",
): string {
  if (!role.password) {
    throw new Error(`Role '${role.name}' does not have a password`);
  }
  const encodedPassword = encodeURIComponent(role.password);
  return `postgresql://${role.name}:${encodedPassword}@${host}/${database}?sslmode=require`;
}

/**
 * Updates a Google Cloud Secret with a new database URL
 */
async function updateGCloudSecret(
  secretName: string,
  databaseUrl: string,
): Promise<void> {
  // Check if secret exists
  let secretExists = false;
  try {
    await $`gcloud secrets describe ${secretName}`.quiet();
    secretExists = true;
  } catch {
    secretExists = false;
  }

  if (!secretExists) {
    // Create the secret first
    await confirmAndRun(
      `gcloud secrets create ${secretName} --replication-policy="automatic"`,
      `Create secret '${secretName}'`,
    );
  }

  // Show what we're about to do (masked for security)
  const maskedUrl = databaseUrl.replace(/:([^@]+)@/, ":****@");
  p.log.info(`Will update secret '${secretName}' with: ${maskedUrl}`);

  const shouldRun = await p.confirm({
    message: `Update secret '${secretName}'?`,
  });

  if (p.isCancel(shouldRun) || !shouldRun) {
    p.log.warn(`Skipped updating secret '${secretName}'`);
    return;
  }

  // Update the secret with the real value
  try {
    await $`echo -n ${databaseUrl} | gcloud secrets versions add ${secretName} --data-file=-`.quiet();
    p.log.success(`Secret '${secretName}' updated successfully`);
  } catch (err) {
    p.log.error(`Failed to update secret '${secretName}'`);
    throw err;
  }
}

/**
 * Sets up a branch with all necessary roles and returns connection info
 * Ensures all roles exist and have fresh passwords
 */
async function setupBranch(
  client: ReturnType<typeof createApiClient>,
  projectId: string,
  branch: Branch,
  needsRestrictedRole: boolean,
): Promise<BranchInfo> {
  p.log.info(`Setting up branch '${branch.name}' (${branch.id})...`);

  // Get endpoint
  const endpoint = await getEndpointForBranch(client, projectId, branch.id);
  p.log.info(`Found endpoint: ${endpoint.host}`);

  // Ensure admin role exists and get it with fresh password
  const adminRoleWithPassword = await ensureAdminRole(
    client,
    projectId,
    branch.id,
    branch.name,
  );

  const result: BranchInfo = {
    branch,
    endpoint,
    adminRole: adminRoleWithPassword,
  };

  // Create/get restricted role if needed (for production)
  if (needsRestrictedRole) {
    result.restrictedRole = await ensureRestrictedRole(
      client,
      projectId,
      branch.id,
    );
  }

  return result;
}

/**
 * Main function to initialize database secrets
 */
export async function initDbSecret(
  neonApiToken: string,
  neonProjectId: string,
): Promise<void> {
  // Create API client
  const client = createApiClient({
    apiKey: neonApiToken,
  });

  // Step 1: List branches
  p.log.step("Fetching project branches...");
  const branchesResponse = await client.listProjectBranches({
    projectId: neonProjectId,
  });
  const branches = branchesResponse.data.branches;

  p.log.info(
    `Found ${branches.length} branches: ${branches.map((b) => b.name).join(", ")}`,
  );

  // Find production and development branches
  // Neon creates default branches, but names may vary
  const productionBranch =
    findBranchByName(branches, "main") ||
    findBranchByName(branches, "production");
  const developmentBranch =
    findBranchByName(branches, "development") ||
    findBranchByName(branches, "dev");

  if (!productionBranch) {
    p.log.error(
      "No production branch found (looked for 'main' or 'production')",
    );
    p.log.info(`Available branches: ${branches.map((b) => b.name).join(", ")}`);
    p.log.error(
      "Please ensure your Neon project has a 'main' or 'production' branch",
    );
    throw new Error("Production branch not found");
  }

  if (!developmentBranch) {
    p.log.error(
      "No development branch found (looked for 'development' or 'dev')",
    );
    p.log.info(`Available branches: ${branches.map((b) => b.name).join(", ")}`);
    p.log.error(
      "Please ensure your Neon project has a 'development' or 'dev' branch",
    );
    throw new Error("Development branch not found");
  }

  p.log.success(
    `Found production branch: ${productionBranch.name} (${productionBranch.id})`,
  );
  p.log.success(
    `Found development branch: ${developmentBranch.name} (${developmentBranch.id})`,
  );

  // Step 2: Setup branches
  p.log.step("Setting up branches...");

  // Production needs restricted role for best practices
  const productionInfo = await setupBranch(
    client,
    neonProjectId,
    productionBranch,
    true,
  );
  const developmentInfo = await setupBranch(
    client,
    neonProjectId,
    developmentBranch,
    false,
  );

  // Step 3: Generate connection URLs and update secrets
  p.log.step("Updating Google Cloud secrets...");

  for (const config of ENV_CONFIGS) {
    p.log.info(`\nProcessing ${config.env} (${config.description})...`);

    const branchInfo =
      config.branchName === "production" ? productionInfo : developmentInfo;
    const role = config.useAdminRole
      ? branchInfo.adminRole
      : branchInfo.restrictedRole;

    if (!role) {
      p.log.error(
        `No ${config.useAdminRole ? "admin" : "restricted"} role available for ${config.env}`,
      );
      process.exit(1);
    }

    const connectionUrl = buildConnectionUrl(branchInfo.endpoint.host, role);

    // Show masked URL for verification
    const maskedUrl = connectionUrl.replace(/:([^@]+)@/, ":****@");
    p.log.info(`Connection URL: ${maskedUrl}`);

    await updateGCloudSecret(config.secretName, connectionUrl);
  }

  // Step 4: Summary
  p.log.step("Summary");

  for (const config of ENV_CONFIGS) {
    const branchInfo =
      config.branchName === "production" ? productionInfo : developmentInfo;
    const role = config.useAdminRole
      ? branchInfo.adminRole
      : branchInfo.restrictedRole;
    p.log.success(
      `${config.env}: ${config.secretName} -> ${role?.name || "N/A"}`,
    );
  }

  p.outro("Database secrets initialized successfully!");
}
