/**
 * Artifact Registry Cleanup Script
 *
 * Automatically cleans up Docker images from Google Cloud Artifact Registry
 * based on retention policies:
 * - PR images: Deleted when the associated PR is closed
 * - Nightly images: Remove after 7 days
 * - Main images: Remove after 1 day
 * - Prod images: Remove after 30 days
 */

import { $ } from "bun";
import { Octokit } from "@octokit/rest";
import { parseTags } from "./shared.ts";

export interface DockerImage {
  digest: string;
  tags: string[];
  createTime: Date;
}

interface RetentionPolicy {
  pattern: RegExp;
  maxAgeDays: number;
  description: string;
}

// Milliseconds in a day for age calculations
const MS_PER_DAY = 1000 * 60 * 60 * 24;

// Retention policies for different image types
const RETENTION_POLICIES: RetentionPolicy[] = [
  {
    pattern: /^nightly-\d{8}$/,
    maxAgeDays: 7,
    description: "Nightly builds",
  },
  {
    pattern: /^main-[a-f0-9]+$/,
    maxAgeDays: 1,
    description: "Main branch builds",
  },
  {
    pattern: /^v\d+\.\d+\.\d+$/,
    maxAgeDays: 30,
    description: "Production releases",
  },
];

interface CleanupOptions {
  projectId: string;
  region: string;
  repository: string;
  imageName: string;
  dryRun: boolean;
}

/**
 * GitHub context for checking PR state
 */
interface GitHubContext {
  token: string;
  owner: string;
  repo: string;
}

/**
 * Counts of images by category
 */
export interface ImageCounts {
  total: number;
  pr: number;
  nightly: number;
  main: number;
  production: number;
  unknown: number;
}

interface CleanupResult {
  deleted: string[];
  skipped: string[];
  errors: string[];
  beforeCounts: ImageCounts;
  afterCounts: ImageCounts;
}

// Allowed characters for GCP resource names (alphanumeric, hyphens, underscores)
const SAFE_RESOURCE_NAME_PATTERN = /^[a-zA-Z0-9][-a-zA-Z0-9_]*$/;

/**
 * Validate a GCP resource name to prevent command injection
 */
function validateResourceName(name: string, fieldName: string): void {
  if (!name || !SAFE_RESOURCE_NAME_PATTERN.test(name)) {
    throw new Error(
      `Invalid ${fieldName}: "${name}". Must start with alphanumeric and contain only alphanumeric, hyphens, or underscores.`,
    );
  }
}

/**
 * Validate all cleanup options
 */
function validateOptions(options: CleanupOptions): void {
  validateResourceName(options.projectId, "project ID");
  validateResourceName(options.region, "region");
  validateResourceName(options.repository, "repository");
  validateResourceName(options.imageName, "image name");
}

/**
 * Get project ID from gcloud config or environment
 */
async function getProjectId(): Promise<string> {
  const envProjectId = process.env.GCP_PROJECT_ID;
  if (envProjectId) {
    return envProjectId;
  }

  try {
    const result = await $`gcloud config get-value project`.text();
    return result.trim();
  } catch {
    throw new Error(
      "Failed to get project ID. Set GCP_PROJECT_ID or configure gcloud.",
    );
  }
}

/**
 * List all Docker images in the Artifact Registry repository
 */
async function listImages(options: CleanupOptions): Promise<DockerImage[]> {
  const { region, projectId, repository, imageName } = options;
  const fullPath = `${region}-docker.pkg.dev/${projectId}/${repository}/${imageName}`;

  console.log(`Listing images in ${fullPath}...`);

  try {
    // List all images with their digests and tags
    const result =
      await $`gcloud artifacts docker images list ${fullPath} --include-tags --format=json`.text();

    const rawImages = JSON.parse(result) as Array<{
      package: string;
      version: string;
      tags: string | string[];
      createTime: string;
    }>;

    return rawImages.map((img) => ({
      digest: img.version,
      tags: parseTags(img.tags),
      createTime: new Date(img.createTime),
    }));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to list images: ${message}`);
  }
}

/**
 * Find the matching retention policy for a tag
 */
function findRetentionPolicy(tag: string): RetentionPolicy | undefined {
  return RETENTION_POLICIES.find((policy) => policy.pattern.test(tag));
}

// Pre-compiled patterns for image categorization
const PR_PATTERN = /^pr-\d+$/;
const NIGHTLY_PATTERN = /^nightly-\d{8}$/;
const MAIN_PATTERN = /^main-[a-f0-9]+$/;
const PRODUCTION_PATTERN = /^v\d+\.\d+\.\d+$/;

/**
 * Categorize an image based on its tags
 */
export function categorizeImage(
  image: DockerImage,
): "pr" | "nightly" | "main" | "production" | "unknown" {
  for (const tag of image.tags) {
    if (PR_PATTERN.test(tag)) return "pr";
    if (NIGHTLY_PATTERN.test(tag)) return "nightly";
    if (MAIN_PATTERN.test(tag)) return "main";
    if (PRODUCTION_PATTERN.test(tag)) return "production";
  }
  return "unknown";
}

/**
 * Count images by category
 */
export function countImagesByCategory(images: DockerImage[]): ImageCounts {
  const counts: ImageCounts = {
    total: images.length,
    pr: 0,
    nightly: 0,
    main: 0,
    production: 0,
    unknown: 0,
  };

  for (const image of images) {
    const category = categorizeImage(image);
    counts[category]++;
  }

  return counts;
}

/**
 * Format image counts for display
 */
export function formatImageCounts(counts: ImageCounts): string {
  const lines = [
    `Total: ${counts.total}`,
    `  PR builds: ${counts.pr}`,
    `  Nightly builds: ${counts.nightly}`,
    `  Main branch builds: ${counts.main}`,
    `  Production releases: ${counts.production}`,
    `  Unknown/Untagged: ${counts.unknown}`,
  ];
  return lines.join("\n");
}

/**
 * Extract PR number from a pr-{number} tag
 */
export function extractPrNumber(tag: string): number | null {
  const match = tag.match(/^pr-(\d+)$/);
  return match ? parseInt(match[1], 10) : null;
}

/**
 * Cache for PR states to avoid repeated API calls
 */
const prStateCache = new Map<number, "open" | "closed">();

/**
 * Check if a PR is closed using the GitHub API
 */
export async function isPrClosed(
  octokit: Octokit,
  owner: string,
  repo: string,
  prNumber: number,
): Promise<boolean> {
  // Check cache first
  const cached = prStateCache.get(prNumber);
  if (cached !== undefined) {
    return cached === "closed";
  }

  try {
    const { data: pr } = await octokit.rest.pulls.get({
      owner,
      repo,
      pull_number: prNumber,
    });
    const state = pr.state as "open" | "closed";
    prStateCache.set(prNumber, state);
    return state === "closed";
  } catch (error) {
    // If we can't fetch PR state (e.g., PR doesn't exist or API error),
    // skip deletion to be safe
    console.warn(
      `Failed to check PR #${prNumber} state: ${error instanceof Error ? error.message : String(error)}`,
    );
    return false;
  }
}

/**
 * Check if an image should be deleted based on retention policies
 * Note: For PR images, use shouldDeletePrImage which checks PR state
 */
function shouldDeleteImage(
  image: DockerImage,
  now: Date,
): {
  shouldDelete: boolean;
  reason: string;
  isPrImage?: boolean;
  prNumber?: number;
} {
  // Skip images with no tags (untagged manifests will be cleaned up by GCR lifecycle)
  if (image.tags.length === 0) {
    return { shouldDelete: false, reason: "No tags (untagged manifest)" };
  }

  // Check each tag against retention policies
  for (const tag of image.tags) {
    const policy = findRetentionPolicy(tag);
    if (policy) {
      const ageInDays =
        (now.getTime() - image.createTime.getTime()) / MS_PER_DAY;
      if (ageInDays > policy.maxAgeDays) {
        return {
          shouldDelete: true,
          reason: `${policy.description}: ${ageInDays.toFixed(1)} days old (max: ${policy.maxAgeDays})`,
        };
      }
      return {
        shouldDelete: false,
        reason: `${policy.description}: ${ageInDays.toFixed(1)} days old (max: ${policy.maxAgeDays})`,
      };
    }
  }

  // Check for PR images - mark for async PR state check
  const prTag = image.tags.find((tag) => tag.startsWith("pr-"));
  if (prTag) {
    const prNumber = extractPrNumber(prTag);
    if (prNumber !== null) {
      return {
        shouldDelete: false,
        reason: "PR image (checking PR state...)",
        isPrImage: true,
        prNumber,
      };
    }
  }

  return { shouldDelete: false, reason: "No matching retention policy" };
}

/**
 * Delete a Docker image from Artifact Registry
 */
async function deleteImage(
  options: CleanupOptions,
  image: DockerImage,
): Promise<void> {
  const { region, projectId, repository, imageName } = options;
  const fullPath = `${region}-docker.pkg.dev/${projectId}/${repository}/${imageName}@${image.digest}`;

  console.log(`Deleting: ${fullPath}`);
  console.log(`  Tags: ${image.tags.join(", ") || "(none)"}`);

  if (options.dryRun) {
    console.log("  [DRY RUN] Would delete this image");
    return;
  }

  await $`gcloud artifacts docker images delete ${fullPath} --quiet --delete-tags`;
}

/**
 * Main cleanup function
 */
export async function cleanupArtifacts(
  options: Partial<CleanupOptions> = {},
  github?: GitHubContext,
): Promise<CleanupResult> {
  const fullOptions: CleanupOptions = {
    projectId: options.projectId || (await getProjectId()),
    region: options.region || "us-east1",
    repository: options.repository || "collects-services",
    imageName: options.imageName || "collects-services",
    dryRun: options.dryRun ?? false,
  };

  // Validate inputs to prevent command injection
  validateOptions(fullOptions);

  // Create Octokit instance if GitHub context is provided
  const octokit = github ? new Octokit({ auth: github.token }) : null;

  console.log("=== Artifact Registry Cleanup ===");
  console.log(`Project: ${fullOptions.projectId}`);
  console.log(`Region: ${fullOptions.region}`);
  console.log(`Repository: ${fullOptions.repository}`);
  console.log(`Image: ${fullOptions.imageName}`);
  console.log(`Dry Run: ${fullOptions.dryRun}`);
  console.log(
    `PR Cleanup: ${octokit ? "enabled (checking closed PRs)" : "disabled (no GitHub token)"}`,
  );
  console.log("");

  // Initialize result with empty counts
  const emptyCounts: ImageCounts = {
    total: 0,
    pr: 0,
    nightly: 0,
    main: 0,
    production: 0,
    unknown: 0,
  };

  const result: CleanupResult = {
    deleted: [],
    skipped: [],
    errors: [],
    beforeCounts: { ...emptyCounts },
    afterCounts: { ...emptyCounts },
  };

  try {
    const images = await listImages(fullOptions);

    // Count images BEFORE cleanup
    result.beforeCounts = countImagesByCategory(images);

    console.log("=== Before Cleanup ===");
    console.log(formatImageCounts(result.beforeCounts));
    console.log("");

    const now = new Date();

    for (const image of images) {
      const { shouldDelete, reason, isPrImage, prNumber } = shouldDeleteImage(
        image,
        now,
      );
      const tagInfo = image.tags.join(", ") || "(untagged)";

      // Handle PR images - check if PR is closed
      if (isPrImage && prNumber !== undefined) {
        if (octokit && github) {
          const prClosed = await isPrClosed(
            octokit,
            github.owner,
            github.repo,
            prNumber,
          );
          if (prClosed) {
            try {
              await deleteImage(fullOptions, image);
              result.deleted.push(`${tagInfo} - PR #${prNumber} is closed`);
              console.log(`  ✅ Deleted: PR #${prNumber} is closed\n`);
            } catch (error) {
              const message =
                error instanceof Error ? error.message : String(error);
              result.errors.push(`${tagInfo}: ${message}`);
              console.log(`  ❌ Error: ${message}\n`);
            }
          } else {
            result.skipped.push(`${tagInfo} - PR #${prNumber} is still open`);
            console.log(
              `  ⏭️  Skipped [${tagInfo}]: PR #${prNumber} is still open`,
            );
          }
        } else {
          result.skipped.push(
            `${tagInfo} - PR image (no GitHub token to check state)`,
          );
          console.log(
            `  ⏭️  Skipped [${tagInfo}]: PR image (no GitHub token to check state)`,
          );
        }
        continue;
      }

      if (shouldDelete) {
        try {
          await deleteImage(fullOptions, image);
          result.deleted.push(`${tagInfo} - ${reason}`);
          console.log(`  ✅ Deleted: ${reason}\n`);
        } catch (error) {
          const message =
            error instanceof Error ? error.message : String(error);
          result.errors.push(`${tagInfo}: ${message}`);
          console.log(`  ❌ Error: ${message}\n`);
        }
      } else {
        result.skipped.push(`${tagInfo} - ${reason}`);
        console.log(`  ⏭️  Skipped [${tagInfo}]: ${reason}`);
      }
    }

    // Count images AFTER cleanup (re-fetch the list)
    const imagesAfter = await listImages(fullOptions);
    result.afterCounts = countImagesByCategory(imagesAfter);

    // Print summary
    console.log("\n=== After Cleanup ===");
    console.log(formatImageCounts(result.afterCounts));
    console.log("");

    console.log("=== Cleanup Summary ===");
    console.log(`Deleted: ${result.deleted.length}`);
    console.log(`Skipped: ${result.skipped.length}`);
    console.log(`Errors: ${result.errors.length}`);

    if (result.deleted.length > 0) {
      console.log("\nDeleted images:");
      result.deleted.forEach((d) => console.log(`  - ${d}`));
    }

    if (result.errors.length > 0) {
      console.log("\nErrors:");
      result.errors.forEach((e) => console.log(`  - ${e}`));
    }

    return result;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Cleanup failed: ${message}`);
    result.errors.push(message);
    return result;
  }
}

/**
 * Write content to GitHub Step Summary (if available)
 */
async function appendToGitHubSummary(content: string): Promise<void> {
  const summaryFile = process.env.GITHUB_STEP_SUMMARY;
  if (summaryFile) {
    try {
      await Bun.write(summaryFile, content + "\n", { append: true });
    } catch (error) {
      // Log error for debugging but don't fail the cleanup
      console.warn("Failed to write to GITHUB_STEP_SUMMARY:", error);
    }
  }
}

/**
 * Format image counts as markdown table
 */
export function formatImageCountsMarkdown(
  label: string,
  counts: ImageCounts,
): string {
  const lines = [
    `### ${label}`,
    "",
    "| Image Type | Count |",
    "|------------|-------|",
    `| **Total** | ${counts.total} |`,
    `| PR builds | ${counts.pr} |`,
    `| Nightly builds | ${counts.nightly} |`,
    `| Main branch builds | ${counts.main} |`,
    `| Production releases | ${counts.production} |`,
    `| Unknown/Untagged | ${counts.unknown} |`,
    "",
  ];
  return lines.join("\n");
}

/**
 * CLI entry point
 */
export async function runArtifactCleanupCLI(): Promise<void> {
  const dryRun = process.env.DRY_RUN === "true";
  const projectId = process.env.GCP_PROJECT_ID;
  const region = process.env.GCP_REGION || "us-east1";
  const repository = process.env.GCP_REPOSITORY || "collects-services";
  const imageName = process.env.GCP_IMAGE_NAME || "collects-services";

  // GitHub context for checking PR states
  const githubToken = process.env.GITHUB_TOKEN;
  const githubRepository = process.env.GITHUB_REPOSITORY;

  let github: GitHubContext | undefined;
  if (githubToken && githubRepository && githubRepository.includes("/")) {
    const [owner, repo] = githubRepository.split("/");
    if (owner && repo) {
      github = { token: githubToken, owner, repo };
    }
  }

  if (!github) {
    console.log(
      "Note: GITHUB_TOKEN or GITHUB_REPOSITORY not set - PR cleanup will be skipped",
    );
    console.log(
      "Set these environment variables to enable automatic cleanup of closed PR images.\n",
    );
  }

  const result = await cleanupArtifacts(
    {
      projectId,
      region,
      repository,
      imageName,
      dryRun,
    },
    github,
  );

  // Write counts to GitHub Step Summary
  const prCleanupNote = github
    ? "PR images for closed PRs are automatically deleted."
    : "⚠️ PR cleanup disabled (set GITHUB_TOKEN and GITHUB_REPOSITORY to enable)";

  const summaryContent = [
    "## Artifact Registry Cleanup Summary",
    "",
    dryRun
      ? "🔍 **Dry Run Mode** - No images were actually deleted"
      : "✅ Cleanup completed",
    "",
    formatImageCountsMarkdown("📊 Before Cleanup", result.beforeCounts),
    formatImageCountsMarkdown("📊 After Cleanup", result.afterCounts),
    "### 📋 Results",
    "",
    `| Action | Count |`,
    `|--------|-------|`,
    `| Deleted | ${result.deleted.length} |`,
    `| Skipped | ${result.skipped.length} |`,
    `| Errors | ${result.errors.length} |`,
    "",
    "### Retention Policies",
    "",
    "| Image Type | Retention Period |",
    "|------------|------------------|",
    "| Nightly builds | 7 days |",
    "| Main branch builds | 1 day |",
    "| Production releases | 30 days |",
    `| PR builds | When PR is closed |`,
    "",
    `> ${prCleanupNote}`,
    "",
  ].join("\n");

  await appendToGitHubSummary(summaryContent);

  // Exit with error if there were any errors
  if (result.errors.length > 0) {
    process.exit(1);
  }
}
