import { createApiClient, EndpointType } from "@neondatabase/api-client";
import * as p from "@clack/prompts";
import { $ } from "bun";
import { runCommand } from "./utils.ts";

/**
 * Updates a Google Cloud Secret with a new value.
 * If the secret doesn't exist, it creates it.
 */
async function updateSecret(secretName: string, value: string) {
  p.log.info(`Updating secret: ${secretName}`);

  // Check if secret exists
  try {
    // Use quiet() to suppress output, throws if fails (non-zero exit)
    await $`gcloud secrets describe ${secretName} --quiet`.quiet();
  } catch {
    // Create secret if it doesn't exist
    p.log.info(`Secret ${secretName} does not exist. Creating it...`);
    await $`gcloud secrets create ${secretName} --replication-policy="automatic"`.quiet();
  }

  const s = p.spinner();
  s.start("Adding secret version...");
  try {
    // Securely pass value via stdin using pipe to avoid shell interpolation
    await $`echo ${value} | gcloud secrets versions add ${secretName} --data-file=-`.quiet();
    s.stop("Secret updated.");
  } catch (err: any) {
    s.stop("Failed to update secret.");
    p.log.error(`Failed to update secret ${secretName}: ${err.message}`);
    process.exit(1);
  }
}

export async function initDbSecret(token: string, projectId: string) {
  const neon = createApiClient({
    apiKey: token,
  });

  const dbName = "collects";

  p.intro(`Initializing Neon Database branches for project: ${projectId}`);

  // 1. Get project info and production branch (Neon's default branch is "production", not "main")
  p.log.info("Fetching project information...");
  let productionBranchId: string;
  let productionBranch: any;

  try {
    const branchesResp = await neon.listProjectBranches({ projectId });
    // Neon creates a default branch named "production" for new projects
    productionBranch = branchesResp.data.branches.find(
      (b: any) => b.name === "production",
    );

    // Fallback to finding the default branch if "production" is not found
    if (!productionBranch) {
      productionBranch = branchesResp.data.branches.find(
        (b: any) => b.default === true,
      );
    }

    if (!productionBranch) {
      throw new Error(
        "Production branch not found in project. Neon projects should have a 'production' branch by default.",
      );
    }
    productionBranchId = productionBranch.id;
    p.log.success(`Found production branch: ${productionBranchId}`);
  } catch (e: any) {
    p.log.error(`Failed to get project branches: ${e.message || e}`);
    process.exit(1);
  }

  // 2. Get or create roles on production branch
  p.log.info("Checking roles on production branch...");
  let adminRole;
  let webUserRole;

  try {
    const rolesResp = await neon.listProjectBranchRoles(
      projectId,
      productionBranchId,
    );
    const roles = rolesResp.data.roles;

    adminRole = roles.find((r: any) => r.name === "admin");
    webUserRole = roles.find((r: any) => r.name === "web_user");

    if (!adminRole) {
      p.log.info("Creating admin role...");
      const adminResp = await neon.createProjectBranchRole(
        projectId,
        productionBranchId,
        {
          role: { name: "admin" },
        },
      );
      adminRole = adminResp.data.role;
    }

    if (!webUserRole) {
      p.log.info("Creating web_user role...");
      const webUserResp = await neon.createProjectBranchRole(
        projectId,
        productionBranchId,
        {
          role: { name: "web_user" },
        },
      );
      webUserRole = webUserResp.data.role;
    }

    p.log.success("Roles configured on production: admin, web_user");
  } catch (e: any) {
    p.log.error(`Failed to configure roles: ${e.message || e}`);
    process.exit(1);
  }

  // 3. Get or create development branch (for local, main, and PR environments)
  // Note: Neon may auto-create a "development" branch, we use it for non-production environments
  p.log.info("Checking development branch...");
  let devBranch;
  let webUserRoleDevPass: string | undefined;

  try {
    const branchesResp = await neon.listProjectBranches({ projectId });
    devBranch = branchesResp.data.branches.find(
      (b: any) => b.name === "development",
    );

    if (!devBranch) {
      p.log.info("Creating development branch from production...");
      const devBranchResp = await neon.createProjectBranch(projectId, {
        branch: {
          name: "development",
          parent_id: productionBranchId,
        },
      });
      devBranch = devBranchResp.data.branch;
      p.log.success("Development branch created");
    } else {
      p.log.success(`Development branch already exists: ${devBranch.id}`);
    }

    // Reset password for web_user on development branch to get a valid password for it
    p.log.info("Resetting web_user password on development branch...");
    const resetResp = await neon.resetProjectBranchRolePassword(
      projectId,
      devBranch.id,
      "web_user",
    );
    webUserRoleDevPass = resetResp.data.role.password;
  } catch (e: any) {
    p.log.error(`Failed to setup development branch: ${e.message || e}`);
    process.exit(1);
  }

  // 4. Get endpoints
  p.log.info("Fetching endpoints...");
  let productionEndpoint;
  let devEndpoint;

  try {
    const endpointsResp = await neon.listProjectEndpoints(projectId);
    const endpoints = endpointsResp.data.endpoints;

    productionEndpoint = endpoints.find(
      (ep: any) => ep.branch_id === productionBranchId,
    );
    devEndpoint = endpoints.find((ep: any) => ep.branch_id === devBranch.id);

    if (!productionEndpoint) throw new Error("Production endpoint not found");

    if (!devEndpoint) {
      p.log.info("Creating endpoint for development branch...");
      const epResp = await neon.createProjectEndpoint(projectId, {
        endpoint: {
          branch_id: devBranch.id,
          type: EndpointType.ReadWrite,
        },
      });
      devEndpoint = epResp.data.endpoint;
    }
  } catch (e: any) {
    p.log.error(`Failed to fetch/create endpoints: ${e.message || e}`);
    process.exit(1);
  }

  // 5. Build connection strings and update secrets
  if (!adminRole.password || !webUserRole.password) {
    p.log.error(
      "Passwords not returned for roles. Cannot create connection strings.",
    );
    process.exit(1);
  }
  if (!webUserRoleDevPass) {
    p.log.error("Password for development branch web_user not obtained.");
    process.exit(1);
  }

  const getConnString = (
    user: string,
    pass: string,
    host: string,
    db: string,
  ) => {
    return `postgres://${user}:${pass}@${host}/${db}?sslmode=require`;
  };

  // === Production branch connection strings ===
  // Production environment (web_user role)
  const databaseUrl = getConnString(
    webUserRole.name,
    webUserRole.password,
    productionEndpoint.host,
    dbName,
  );
  // Internal/admin environment (admin role for migrations)
  const databaseUrlInternal = getConnString(
    adminRole.name,
    adminRole.password,
    productionEndpoint.host,
    dbName,
  );
  // Nightly environment (uses production branch with web_user)
  const databaseUrlNightly = getConnString(
    webUserRole.name,
    webUserRole.password,
    productionEndpoint.host,
    dbName,
  );

  // === Development branch connection strings ===
  // Used for: local development, main branch deployments, and PR environments
  const databaseUrlDev = getConnString(
    webUserRole.name,
    webUserRoleDevPass,
    devEndpoint.host,
    dbName,
  );

  p.log.info("Updating Google Cloud Secrets...");

  // Production branch secrets
  await updateSecret("database-url", databaseUrl); // Production environment
  await updateSecret("database-url-internal", databaseUrlInternal); // Admin/migrations
  await updateSecret("database-url-nightly", databaseUrlNightly); // Nightly environment

  // Development branch secrets (for local, main, and PR environments)
  await updateSecret("database-url-dev", databaseUrlDev); // Local development
  await updateSecret("database-url-main", databaseUrlDev); // Main branch deployments
  await updateSecret("database-url-pr", databaseUrlDev); // PR environments

  p.outro("Neon Database branches setup complete!");
  p.log.info(`Project ID: ${projectId}`);
  p.log.info("Branch structure:");
  p.log.info("  - Production branch → prod, internal (admin), nightly");
  p.log.info("  - Development branch → local, main, PR environments");
  p.log.info(
    "Next steps: Run SQL migrations using the admin credentials (database-url-internal).",
  );
}
