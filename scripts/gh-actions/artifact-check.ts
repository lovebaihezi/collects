/**
 * Artifact Registry Check Script
 *
 * Checks the current state of Docker images in Google Cloud Artifact Registry
 * and verifies if cleanup policies are being applied correctly.
 */

import { $ } from "bun";

interface DockerImage {
  digest: string;
  tags: string[];
  createTime: Date;
}

interface ImageCategory {
  name: string;
  images: DockerImage[];
  retentionDays: number | null; // null means cleaned on PR close
}

interface ImageToRemove {
  tags: string[];
  digest: string;
  age: string;
  reason: string;
  fullPath: string;
}

interface CheckResult {
  totalImages: number;
  categories: {
    pr: ImageCategory;
    nightly: ImageCategory;
    main: ImageCategory;
    production: ImageCategory;
    unknown: ImageCategory;
  };
  violations: string[];
  imagesToRemove: ImageToRemove[];
  summary: string[];
}

const MS_PER_DAY = 1000 * 60 * 60 * 24;

// Allowed characters for GCP resource names
const SAFE_RESOURCE_NAME_PATTERN = /^[a-zA-Z0-9][-a-zA-Z0-9_]*$/;

function validateResourceName(name: string, fieldName: string): void {
  if (!name || !SAFE_RESOURCE_NAME_PATTERN.test(name)) {
    throw new Error(
      `Invalid ${fieldName}: "${name}". Must start with alphanumeric and contain only alphanumeric, hyphens, or underscores.`,
    );
  }
}

interface CheckOptions {
  projectId: string;
  region: string;
  repository: string;
  imageName: string;
}

function validateOptions(options: CheckOptions): void {
  validateResourceName(options.projectId, "project ID");
  validateResourceName(options.region, "region");
  validateResourceName(options.repository, "repository");
  validateResourceName(options.imageName, "image name");
}

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

async function listImages(options: CheckOptions): Promise<DockerImage[]> {
  const { region, projectId, repository, imageName } = options;
  const fullPath = `${region}-docker.pkg.dev/${projectId}/${repository}/${imageName}`;

  try {
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

function categorizeImage(image: DockerImage): string {
  for (const tag of image.tags) {
    if (tag.match(/^pr-\d+$/)) return "pr";
    if (tag.match(/^nightly-\d{8}$/)) return "nightly";
    if (tag.match(/^main-[a-f0-9]+$/)) return "main";
    if (tag.match(/^v\d+\.\d+\.\d+$/)) return "production";
  }
  return "unknown";
}

function calculateAgeDays(createTime: Date, now: Date): number {
  return (now.getTime() - createTime.getTime()) / MS_PER_DAY;
}

function formatAge(createTime: Date, now: Date): string {
  const ageMs = now.getTime() - createTime.getTime();
  const days = Math.floor(ageMs / MS_PER_DAY);
  const hours = Math.floor((ageMs % MS_PER_DAY) / (1000 * 60 * 60));

  if (days > 0) {
    return `${days}d ${hours}h`;
  }
  return `${hours}h`;
}

export async function checkArtifacts(
  options: Partial<CheckOptions> = {},
): Promise<CheckResult> {
  const fullOptions: CheckOptions = {
    projectId: options.projectId || (await getProjectId()),
    region: options.region || "us-east1",
    repository: options.repository || "collects-services",
    imageName: options.imageName || "collects-services",
  };

  validateOptions(fullOptions);

  const baseImagePath = `${fullOptions.region}-docker.pkg.dev/${fullOptions.projectId}/${fullOptions.repository}/${fullOptions.imageName}`;

  const result: CheckResult = {
    totalImages: 0,
    categories: {
      pr: { name: "PR builds", images: [], retentionDays: null },
      nightly: { name: "Nightly builds", images: [], retentionDays: 7 },
      main: { name: "Main branch builds", images: [], retentionDays: 1 },
      production: { name: "Production releases", images: [], retentionDays: 30 },
      unknown: { name: "Unknown/Untagged", images: [], retentionDays: null },
    },
    violations: [],
    imagesToRemove: [],
    summary: [],
  };

  console.log("=== Artifact Registry Check ===");
  console.log(`Project: ${fullOptions.projectId}`);
  console.log(`Region: ${fullOptions.region}`);
  console.log(`Repository: ${fullOptions.repository}`);
  console.log(`Image: ${fullOptions.imageName}`);
  console.log("");

  const images = await listImages(fullOptions);
  result.totalImages = images.length;

  const now = new Date();

  // Categorize images
  for (const image of images) {
    const category = categorizeImage(image);
    result.categories[category as keyof typeof result.categories].images.push(
      image,
    );
  }

  // Check for violations and build summary
  console.log("=== Image Summary ===\n");

  // PR images - should be empty if all PRs are closed
  const prCat = result.categories.pr;
  console.log(`📦 ${prCat.name}: ${prCat.images.length} images`);
  if (prCat.images.length > 0) {
    console.log("   ⚠️  PR images should be cleaned when PRs close");
    for (const img of prCat.images) {
      const age = formatAge(img.createTime, now);
      const tags = img.tags.join(", ");
      console.log(`   - ${tags} (age: ${age})`);
      result.violations.push(
        `PR image "${tags}" still exists (age: ${age}) - should be cleaned on PR close`,
      );
      result.imagesToRemove.push({
        tags: img.tags,
        digest: img.digest,
        age,
        reason: "PR closed - should be cleaned immediately",
        fullPath: `${baseImagePath}@${img.digest}`,
      });
    }
  }
  console.log("");

  // Nightly images
  const nightlyCat = result.categories.nightly;
  console.log(`🌙 ${nightlyCat.name}: ${nightlyCat.images.length} images`);
  for (const img of nightlyCat.images) {
    const ageDays = calculateAgeDays(img.createTime, now);
    const age = formatAge(img.createTime, now);
    const tags = img.tags.join(", ");

    if (ageDays > 7) {
      console.log(`   ❌ ${tags} (age: ${age}) - exceeds 7 day retention`);
      result.violations.push(
        `Nightly image "${tags}" (age: ${age}) exceeds 7 day retention`,
      );
      result.imagesToRemove.push({
        tags: img.tags,
        digest: img.digest,
        age,
        reason: "Exceeds 7 day retention policy",
        fullPath: `${baseImagePath}@${img.digest}`,
      });
    } else {
      console.log(`   ✅ ${tags} (age: ${age})`);
    }
  }
  console.log("");

  // Main branch images
  const mainCat = result.categories.main;
  console.log(`🔀 ${mainCat.name}: ${mainCat.images.length} images`);
  for (const img of mainCat.images) {
    const ageDays = calculateAgeDays(img.createTime, now);
    const age = formatAge(img.createTime, now);
    const tags = img.tags.join(", ");

    if (ageDays > 1) {
      console.log(`   ❌ ${tags} (age: ${age}) - exceeds 1 day retention`);
      result.violations.push(
        `Main branch image "${tags}" (age: ${age}) exceeds 1 day retention`,
      );
      result.imagesToRemove.push({
        tags: img.tags,
        digest: img.digest,
        age,
        reason: "Exceeds 1 day retention policy",
        fullPath: `${baseImagePath}@${img.digest}`,
      });
    } else {
      console.log(`   ✅ ${tags} (age: ${age})`);
    }
  }
  console.log("");

  // Production releases
  const prodCat = result.categories.production;
  console.log(`🚀 ${prodCat.name}: ${prodCat.images.length} images`);
  for (const img of prodCat.images) {
    const ageDays = calculateAgeDays(img.createTime, now);
    const age = formatAge(img.createTime, now);
    const tags = img.tags.join(", ");

    if (ageDays > 30) {
      console.log(`   ❌ ${tags} (age: ${age}) - exceeds 30 day retention`);
      result.violations.push(
        `Production image "${tags}" (age: ${age}) exceeds 30 day retention`,
      );
      result.imagesToRemove.push({
        tags: img.tags,
        digest: img.digest,
        age,
        reason: "Exceeds 30 day retention policy",
        fullPath: `${baseImagePath}@${img.digest}`,
      });
    } else {
      console.log(`   ✅ ${tags} (age: ${age})`);
    }
  }
  console.log("");

  // Unknown/Untagged
  const unknownCat = result.categories.unknown;
  if (unknownCat.images.length > 0) {
    console.log(`❓ ${unknownCat.name}: ${unknownCat.images.length} images`);
    for (const img of unknownCat.images) {
      const age = formatAge(img.createTime, now);
      const tags = img.tags.length > 0 ? img.tags.join(", ") : "(untagged)";
      console.log(`   - ${tags} (age: ${age})`);
    }
    console.log("");
  }

  // Final summary
  console.log("=== Cleanup Status ===\n");
  console.log(`Total images: ${result.totalImages}`);
  console.log(`  - PR builds: ${prCat.images.length}`);
  console.log(`  - Nightly builds: ${nightlyCat.images.length}`);
  console.log(`  - Main branch: ${mainCat.images.length}`);
  console.log(`  - Production: ${prodCat.images.length}`);
  console.log(`  - Unknown/Untagged: ${unknownCat.images.length}`);
  console.log("");

  if (result.violations.length === 0) {
    console.log("✅ All images are within retention policies");
    result.summary.push("All images are within retention policies");
  } else {
    console.log(`⚠️  Found ${result.violations.length} violation(s):`);
    for (const violation of result.violations) {
      console.log(`   - ${violation}`);
    }
    console.log("");
    console.log("Run `just scripts::artifact-cleanup` to clean up old images");
    result.summary.push(
      `Found ${result.violations.length} images that should be cleaned up`,
    );
  }

  // List images to remove
  if (result.imagesToRemove.length > 0) {
    console.log("\n=== Images to Remove ===\n");
    console.log(
      `The following ${result.imagesToRemove.length} image(s) should be removed based on retention policies:\n`,
    );

    for (const img of result.imagesToRemove) {
      const tags = img.tags.join(", ") || "(untagged)";
      console.log(`📛 ${tags}`);
      console.log(`   Age: ${img.age}`);
      console.log(`   Reason: ${img.reason}`);
      console.log(`   Digest: ${img.digest}`);
      console.log(`   Full path: ${img.fullPath}`);
      console.log("");
    }

    console.log("=== Removal Commands ===\n");
    console.log("To remove these images, run the following commands:\n");
    for (const img of result.imagesToRemove) {
      console.log(
        `gcloud artifacts docker images delete "${img.fullPath}" --delete-tags --quiet`,
      );
    }
    console.log(
      "\nOr run `just scripts::artifact-cleanup` to automatically clean up all images.",
    );
  }

  return result;
}

export async function runArtifactCheckCLI(): Promise<void> {
  const projectId = process.env.GCP_PROJECT_ID;
  const region = process.env.GCP_REGION || "us-east1";
  const repository = process.env.GCP_REPOSITORY || "collects-services";
  const imageName = process.env.GCP_IMAGE_NAME || "collects-services";

  const result = await checkArtifacts({
    projectId,
    region,
    repository,
    imageName,
  });

  // Exit with error if there are violations
  if (result.violations.length > 0) {
    process.exit(1);
  }
}
