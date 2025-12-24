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

  // 1. Get project info and main branch
  p.log.info("Fetching project information...");
  let mainBranchId: string;
  let mainBranch: any;

  try {
    const branchesResp = await neon.listProjectBranches({ projectId });
    mainBranch = branchesResp.data.branches.find((b: any) => b.name === "main");

    if (!mainBranch) {
      throw new Error("Main branch not found in project");
    }
    mainBranchId = mainBranch.id;
    p.log.success(`Found main branch: ${mainBranchId}`);
  } catch (e: any) {
    p.log.error(`Failed to get project branches: ${e.message || e}`);
    process.exit(1);
  }

  // 2. Get or create roles on main branch
  p.log.info("Checking roles on main branch...");
  let adminRole;
  let webUserRole;

  try {
    const rolesResp = await neon.listProjectBranchRoles(
      projectId,
      mainBranchId,
    );
    const roles = rolesResp.data.roles;

    adminRole = roles.find((r: any) => r.name === "admin");
    webUserRole = roles.find((r: any) => r.name === "web_user");

    if (!adminRole) {
      p.log.info("Creating admin role...");
      const adminResp = await neon.createProjectBranchRole(
        projectId,
        mainBranchId,
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
        mainBranchId,
        {
          role: { name: "web_user" },
        },
      );
      webUserRole = webUserResp.data.role;
    }

    p.log.success("Roles configured on main: admin, web_user");
  } catch (e: any) {
    p.log.error(`Failed to configure roles: ${e.message || e}`);
    process.exit(1);
  }

  // 3. Get or create test branch
  p.log.info("Checking test branch...");
  let testBranch;
  let webUserRoleTestPass: string | undefined;

  try {
    const branchesResp = await neon.listProjectBranches({ projectId });
    testBranch = branchesResp.data.branches.find((b: any) => b.name === "test");

    if (!testBranch) {
      p.log.info("Creating test branch from main...");
      const testBranchResp = await neon.createProjectBranch(projectId, {
        branch: {
          name: "test",
          parent_id: mainBranchId,
        },
      });
      testBranch = testBranchResp.data.branch;
      p.log.success("Test branch created");
    } else {
      p.log.success(`Test branch already exists: ${testBranch.id}`);
    }

    // Reset password for web_user on test branch to get a valid password for it
    p.log.info("Resetting web_user password on test branch...");
    const resetResp = await neon.resetProjectBranchRolePassword(
      projectId,
      testBranch.id,
      "web_user",
    );
    webUserRoleTestPass = resetResp.data.role.password;
  } catch (e: any) {
    p.log.error(`Failed to setup test branch: ${e.message || e}`);
    process.exit(1);
  }

  // 4. Get endpoints
  p.log.info("Fetching endpoints...");
  let mainEndpoint;
  let testEndpoint;

  try {
    const endpointsResp = await neon.listProjectEndpoints(projectId);
    const endpoints = endpointsResp.data.endpoints;

    mainEndpoint = endpoints.find((ep: any) => ep.branch_id === mainBranchId);
    testEndpoint = endpoints.find((ep: any) => ep.branch_id === testBranch.id);

    if (!mainEndpoint) throw new Error("Main endpoint not found");

    if (!testEndpoint) {
      p.log.info("Creating endpoint for test branch...");
      const epResp = await neon.createProjectEndpoint(projectId, {
        endpoint: {
          branch_id: testBranch.id,
          type: EndpointType.ReadWrite,
        },
      });
      testEndpoint = epResp.data.endpoint;
    }
  } catch (e: any) {
    p.log.error(`Failed to fetch/create endpoints: ${e.message || e}`);
    process.exit(1);
  }

  // 5. Build connection strings and update secrets
  const getConnString = (
    user: string,
    pass: string,
    host: string,
    db: string,
  ) => {
    return `postgres://${user}:${pass}@${host}/${db}?sslmode=require`;
  };

  const databaseUrl = getConnString(
    webUserRole.name,
    webUserRole.password!,
    mainEndpoint.host,
    dbName,
  );
  const databaseUrlInternal = getConnString(
    adminRole.name,
    adminRole.password!,
    mainEndpoint.host,
    dbName,
  );
  const databaseUrlTest = getConnString(
    "web_user",
    webUserRoleTestPass!,
    testEndpoint.host,
    dbName,
  );

  p.log.info("Updating Google Cloud Secrets...");

  await updateSecret("database-url", databaseUrl);
  await updateSecret("database-url-internal", databaseUrlInternal);
  await updateSecret("database-url-test", databaseUrlTest);
  // PR environment uses database-url-pr which also points to the test branch/user
  await updateSecret("database-url-pr", databaseUrlTest);

  p.outro("Neon Database branches setup complete!");
  p.log.info(`Project ID: ${projectId}`);
  p.log.info(
    "Next steps: Run SQL migrations using the admin credentials (database-url-internal).",
  );
}
