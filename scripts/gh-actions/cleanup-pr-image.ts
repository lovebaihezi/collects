import { appendFileSync } from "fs";
import { execSync } from "child_process";

/**
 * Cleanup PR Image - Deletes Docker images for closed PRs from GCloud Artifact Registry
 *
 * This script is designed to be called from GitHub Actions when a PR is closed (merged or not).
 * It removes the Docker image tagged with the PR number from GCloud Artifact Registry.
 */

export interface CleanupOptions {
  prNumber: number;
  gcpRegion?: string;
  repositoryName?: string;
  imageName?: string;
}

export interface CleanupResult {
  imageDeleted: boolean;
  imageTag: string;
  error?: string;
}

/**
 * Executes gcloud command and returns output
 */
function runGcloudCommand(args: string[]): { success: boolean; output: string } {
  try {
    const output = execSync(`gcloud ${args.join(" ")}`, {
      encoding: "utf-8",
      stdio: ["pipe", "pipe", "pipe"],
    });
    return { success: true, output: output.trim() };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { success: false, output: message };
  }
}

/**
 * Gets the current GCP project ID from gcloud config
 */
function getProjectId(): string {
  const result = runGcloudCommand(["config", "get-value", "project"]);
  if (!result.success) {
    throw new Error(`Failed to get GCP project ID: ${result.output}`);
  }
  return result.output;
}

/**
 * Deletes a Docker image from GCloud Artifact Registry
 */
export function deleteDockerImage(options: CleanupOptions): CleanupResult {
  const {
    prNumber,
    gcpRegion = "us-east1",
    repositoryName = "collects-services",
    imageName = "collects-services",
  } = options;

  const imageTag = `pr-${prNumber}`;
  const projectId = getProjectId();
  const fullImagePath = `${gcpRegion}-docker.pkg.dev/${projectId}/${repositoryName}/${imageName}:${imageTag}`;

  console.log(`Attempting to delete Docker image: ${fullImagePath}`);

  const result = runGcloudCommand([
    "artifacts",
    "docker",
    "images",
    "delete",
    fullImagePath,
    "--quiet",
    "--delete-tags",
  ]);

  if (result.success) {
    console.log(`Successfully deleted Docker image with tag: ${imageTag}`);
    return { imageDeleted: true, imageTag };
  } else {
    console.log(
      `Docker image with tag ${imageTag} not found or already deleted`,
    );
    return { imageDeleted: false, imageTag, error: result.output };
  }
}

/**
 * Set output for GitHub Actions
 */
function setOutput(name: string, value: string): void {
  console.log(`${name}=${value}`);
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`);
  }
}

/**
 * Writes cleanup summary to GitHub Actions step summary
 */
function writeStepSummary(prNumber: number, result: CleanupResult): void {
  if (!process.env.GITHUB_STEP_SUMMARY) {
    return;
  }

  let summary = `## Cleanup Summary for PR #${prNumber}\n\n`;

  if (result.imageDeleted) {
    summary += `- ✅ Docker image \`${result.imageTag}\` deleted\n`;
  } else {
    summary += `- ℹ️ Docker image \`${result.imageTag}\` not found or already deleted\n`;
  }

  summary +=
    "- ℹ️ Cloud Run service `collects-services-pr` is shared and remains active\n";
  summary +=
    "- ℹ️ Cloudflare Worker `pr` environment is shared and remains active\n";

  appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
}

/**
 * CLI entry point for GitHub Actions
 */
export function runCleanupPRImageCLI(): void {
  const prNumberStr = process.env.PR_NUMBER;

  if (!prNumberStr) {
    console.error("PR_NUMBER environment variable is required");
    process.exit(1);
  }

  const prNumber = parseInt(prNumberStr, 10);
  if (isNaN(prNumber)) {
    console.error("PR_NUMBER must be a valid number");
    process.exit(1);
  }

  // Optional environment variables for customization
  const gcpRegion = process.env.GCP_REGION || "us-east1";
  const repositoryName = process.env.REPOSITORY_NAME || "collects-services";
  const imageName = process.env.IMAGE_NAME || "collects-services";

  const result = deleteDockerImage({
    prNumber,
    gcpRegion,
    repositoryName,
    imageName,
  });

  // Set outputs for GitHub Actions
  setOutput("image_deleted", String(result.imageDeleted));
  setOutput("image_tag", result.imageTag);

  // Write step summary
  writeStepSummary(prNumber, result);

  console.log("\nCleanup completed!");
}
