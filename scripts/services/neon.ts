import { createApiClient, EndpointType } from "@neondatabase/api-client";
import * as p from "@clack/prompts";
import { $ } from "bun";

/**
 * Environment types for database connections
 */
export type DbEnv = "prod" | "test" | "pr" | "nightly" | "internal";

/**
 * Maps environment names to their corresponding Google Cloud Secret names
 */
export function getSecretNameForEnv(env: DbEnv): string {
  switch (env) {
    case "prod":
      return "database-url";
    case "internal":
      return "database-url-internal";
    default:
      return `database-url-${env}`;
  }
}

/**
 * Gets the DATABASE_URL from Google Cloud Secrets for a given environment
 */
export async function getDatabaseUrl(env: DbEnv): Promise<string> {
  const secretName = getSecretNameForEnv(env);
  const url = await $`gcloud secrets versions access latest --secret=${secretName}`.text();
  return url.trim();
}

/**
 * Lists all Neon branches for a project
 */
export async function listNeonBranches(token: string, projectId: string) {
  const neon = createApiClient({ apiKey: token });
  const response = await neon.listProjectBranches({ projectId });
  return response.data.branches;
}

/**
 * Creates a new Neon branch from an existing branch
 */
export async function createNeonBranch(
  token: string,
  projectId: string,
  branchName: string,
  parentBranchId: string,
) {
  const neon = createApiClient({ apiKey: token });
  const response = await neon.createProjectBranch(projectId, {
    branch: {
      name: branchName,
      parent_id: parentBranchId,
    },
  });
  return response.data.branch;
}

/**
 * Deletes a Neon branch
 */
export async function deleteNeonBranch(
  token: string,
  projectId: string,
  branchId: string,
) {
  const neon = createApiClient({ apiKey: token });
  await neon.deleteProjectBranch(projectId, branchId);
}

/**
 * Gets connection string for a Neon branch
 */
export async function getBranchConnectionString(
  token: string,
  projectId: string,
  branchId: string,
  roleName: string,
  dbName: string,
): Promise<string | null> {
  const neon = createApiClient({ apiKey: token });

  // Get endpoint for the branch
  const endpointsResp = await neon.listProjectEndpoints(projectId);
  const endpoint = endpointsResp.data.endpoints.find(
    (ep) => ep.branch_id === branchId,
  );

  if (!endpoint) {
    return null;
  }

  // Get role password
  const rolesResp = await neon.listProjectBranchRoles(projectId, branchId);
  const role = rolesResp.data.roles.find((r) => r.name === roleName);

  if (!role || !role.password) {
    // Try to reset password to get it
    const resetResp = await neon.resetProjectBranchRolePassword(
      projectId,
      branchId,
      roleName,
    );
    const password = resetResp.data.role.password;
    if (!password) return null;
    return `postgres://${roleName}:${password}@${endpoint.host}/${dbName}?sslmode=require`;
  }

  return `postgres://${roleName}:${role.password}@${endpoint.host}/${dbName}?sslmode=require`;
}

/**
 * Updates a Google Cloud Secret with a new value.
 * If the secret doesn't exist, it creates it.
 */
export async function updateSecret(secretName: string, value: string) {
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
      // Securely pass value via stdin using Bun.spawn to avoid shell interpolation and logging
      const proc = Bun.spawn(["gcloud", "secrets", "versions", "add", secretName, "--data-file=-"], {
          stdin: "pipe",
          stdout: "ignore",
          stderr: "pipe",
      });

      // Write value to stdin
      if (proc.stdin) {
          proc.stdin.write(value);
          proc.stdin.flush();
          proc.stdin.end();
      }

      const exitCode = await proc.exited;

      if (exitCode !== 0) {
          const stderr = await new Response(proc.stderr).text();
          throw new Error(stderr);
      }

      s.stop("Secret updated.");
  } catch (err: any) {
      s.stop("Failed to update secret.");
      p.log.error(`Failed to update secret ${secretName}: ${err.message}`);
      process.exit(1);
  }
}

export async function initDbSecret(token: string) {
  const neon = createApiClient({
    apiKey: token,
  });

  const date = new Date().toISOString().split("T")[0]; // YYYY-MM-DD
  const projectName = `collects-${date}`;
  const dbName = "collects";

  p.intro(`Initializing Neon Database: ${projectName}`);

  // 1. Create Project
  p.log.info(`Creating Neon project: ${projectName}...`);

  let project;
  try {
    const response = await neon.createProject({
        project: {
            name: projectName,
            pg_version: 16
        }
    });
    project = response.data.project;
  } catch (e: any) {
    p.log.error(`Failed to create project: ${e.message || e}`);
    process.exit(1);
  }

  const projectId = project.id;
  p.log.success(`Project created: ${projectId}`);

  // 2. Create Database
  p.log.info(`Creating database: ${dbName}...`);
  try {
      const branchesResp = await neon.listProjectBranches({ projectId });
      const mainBranch = branchesResp.data.branches.find((b: any) => b.name === 'main');

      if (!mainBranch) {
          throw new Error("Main branch not found after project creation");
      }

      await neon.createProjectBranchDatabase(projectId, mainBranch.id, {
          database: {
              name: dbName,
              owner_name: "neondb_owner"
          }
      });
  } catch (e: any) {
       p.log.error(`Failed to create database: ${e.message || e}`);
       process.exit(1);
  }
  p.log.success(`Database created: ${dbName}`);

  // 3. Configure Roles
  p.log.info("Configuring roles...");
  let adminRole;
  let webUserRole;
  let mainBranchId: string;

  try {
      // Get main branch again or reuse
      const branchesResp = await neon.listProjectBranches({ projectId });
      const mainBranch = branchesResp.data.branches.find((b: any) => b.name === 'main');
      if (!mainBranch) throw new Error("Main branch missing");
      mainBranchId = mainBranch.id;

      // Create admin on Main
      const adminResp = await neon.createProjectBranchRole(projectId, mainBranchId, {
          role: { name: "admin" }
      });
      adminRole = adminResp.data.role;

      // Create web_user on Main
      const webUserResp = await neon.createProjectBranchRole(projectId, mainBranchId, {
          role: { name: "web_user" }
      });
      webUserRole = webUserResp.data.role;
  } catch (e: any) {
      p.log.error(`Failed to create roles: ${e.message || e}`);
      process.exit(1);
  }
  p.log.success("Roles created on main: admin, web_user");

  // 4. Sets up Branches
  p.log.info("Setting up branches...");
  let testBranch;
  let webUserRoleTestPass: string | undefined;

  try {
      // Create test branch from main
      const testBranchResp = await neon.createProjectBranch(projectId, {
          branch: {
              name: "test",
              parent_id: mainBranchId
          }
      });
      testBranch = testBranchResp.data.branch;

      // Reset password for web_user on test branch to get a valid password for it
      p.log.info("Resetting web_user password on test branch...");
      const resetResp = await neon.resetProjectBranchRolePassword(projectId, testBranch.id, "web_user");
      webUserRoleTestPass = resetResp.data.role.password;

  } catch (e: any) {
      p.log.error(`Failed to setup branches: ${e.message || e}`);
      process.exit(1);
  }
  p.log.success("Branches setup: main, test");

  // 5. Update Secrets
  if (!adminRole.password || !webUserRole.password) {
      p.log.warn("Passwords not returned for roles. This might cause issues.");
  }
  if (!webUserRoleTestPass) {
      p.log.warn("Password for test branch web_user not obtained.");
  }

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
                  type: EndpointType.ReadWrite
              }
          });
          testEndpoint = epResp.data.endpoint;
      }
  } catch (e: any) {
       p.log.error(`Failed to fetch/create endpoints: ${e.message || e}`);
       process.exit(1);
  }

  const getConnString = (user: string, pass: string, host: string, db: string) => {
      return `postgres://${user}:${pass}@${host}/${db}?sslmode=require`;
  }

  const databaseUrl = getConnString(webUserRole.name, webUserRole.password!, mainEndpoint.host, dbName);
  const databaseUrlInternal = getConnString(adminRole.name, adminRole.password!, mainEndpoint.host, dbName);
  const databaseUrlTest = getConnString("web_user", webUserRoleTestPass!, testEndpoint.host, dbName);

  p.log.info("Updating Google Cloud Secrets...");

  await updateSecret("database-url", databaseUrl);
  await updateSecret("database-url-internal", databaseUrlInternal);
  await updateSecret("database-url-test", databaseUrlTest);
  // PR environment uses database-url-pr which also points to the test branch/user
  await updateSecret("database-url-pr", databaseUrlTest);

  p.outro("Neon Database Setup Complete!");
  p.log.info(`Project: ${projectName}`);
  p.log.info("Next steps: Run SQL migrations using the admin credentials (database-url-internal).");
}
