/**
 * Artifact Registry Cleanup Script
 *
 * Automatically cleans up Docker images from Google Cloud Artifact Registry
 * based on retention policies:
 * - PR images: Handled separately by cleanup-pr.yml when PR is closed
 * - Nightly images: Remove after 7 days
 * - Main images: Remove after 1 day
 * - Prod images: Remove after 30 days
 */

import { $ } from "bun";

interface DockerImage {
  digest: string;
  tags: string[];
  createTime: Date;
}

interface RetentionPolicy {
  pattern: RegExp;
  maxAgeDays: number;
  description: string;
}

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

interface CleanupResult {
  deleted: string[];
  skipped: string[];
  errors: string[];
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
      tags: string;
      createTime: string;
    }>;

    return rawImages.map((img) => ({
      digest: img.version,
      tags: img.tags ? img.tags.split(",").map((t) => t.trim()) : [],
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

/**
 * Check if an image should be deleted based on retention policies
 */
function shouldDeleteImage(
  image: DockerImage,
  now: Date,
): { shouldDelete: boolean; reason: string } {
  // Skip images with no tags (untagged manifests will be cleaned up by GCR lifecycle)
  if (image.tags.length === 0) {
    return { shouldDelete: false, reason: "No tags (untagged manifest)" };
  }

  // Check each tag against retention policies
  for (const tag of image.tags) {
    const policy = findRetentionPolicy(tag);
    if (policy) {
      const ageInDays =
        (now.getTime() - image.createTime.getTime()) / (1000 * 60 * 60 * 24);
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

  // Skip PR images (they are handled by cleanup-pr.yml when PR closes)
  const hasPrTag = image.tags.some((tag) => tag.startsWith("pr-"));
  if (hasPrTag) {
    return {
      shouldDelete: false,
      reason: "PR image (handled by cleanup-pr.yml)",
    };
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
): Promise<CleanupResult> {
  const fullOptions: CleanupOptions = {
    projectId: options.projectId || (await getProjectId()),
    region: options.region || "us-east1",
    repository: options.repository || "collects-services",
    imageName: options.imageName || "collects-services",
    dryRun: options.dryRun ?? false,
  };

  console.log("=== Artifact Registry Cleanup ===");
  console.log(`Project: ${fullOptions.projectId}`);
  console.log(`Region: ${fullOptions.region}`);
  console.log(`Repository: ${fullOptions.repository}`);
  console.log(`Image: ${fullOptions.imageName}`);
  console.log(`Dry Run: ${fullOptions.dryRun}`);
  console.log("");

  const result: CleanupResult = {
    deleted: [],
    skipped: [],
    errors: [],
  };

  try {
    const images = await listImages(fullOptions);
    console.log(`Found ${images.length} images\n`);

    const now = new Date();

    for (const image of images) {
      const { shouldDelete, reason } = shouldDeleteImage(image, now);
      const tagInfo = image.tags.join(", ") || "(untagged)";

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

    // Print summary
    console.log("\n=== Cleanup Summary ===");
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
 * CLI entry point
 */
export async function runArtifactCleanupCLI(): Promise<void> {
  const dryRun = process.env.DRY_RUN === "true";
  const projectId = process.env.GCP_PROJECT_ID;
  const region = process.env.GCP_REGION || "us-east1";
  const repository = process.env.GCP_REPOSITORY || "collects-services";
  const imageName = process.env.GCP_IMAGE_NAME || "collects-services";

  const result = await cleanupArtifacts({
    projectId,
    region,
    repository,
    imageName,
    dryRun,
  });

  // Exit with error if there were any errors
  if (result.errors.length > 0) {
    process.exit(1);
  }
}
