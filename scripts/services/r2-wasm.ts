/**
 * R2 WASM Management for PR builds
 *
 * This module provides functionality to upload and delete WASM files
 * stored in Cloudflare R2 for PR-specific builds.
 *
 * R2 Path format: pr-{pr_number}/{filename}
 * Example: pr-123/collects-ui-abc123.wasm
 */

import { $ } from "bun";
import { readdirSync, statSync } from "fs";
import { join, basename } from "path";

/**
 * Validate that a string contains only safe characters for use in R2 paths.
 * Allows alphanumeric characters, hyphens, underscores, dots, and forward slashes.
 */
function isValidR2Key(key: string): boolean {
  return /^[a-zA-Z0-9\-_./]+$/.test(key);
}

/**
 * Validate that a PR number is a valid positive integer string.
 */
function isValidPrNumber(prNumber: string): boolean {
  return /^\d+$/.test(prNumber);
}

/**
 * Validate that a bucket name contains only safe characters.
 */
function isValidBucketName(name: string): boolean {
  return /^[a-zA-Z0-9\-]+$/.test(name);
}

/**
 * Upload WASM files from dist directory to R2
 *
 * @param prNumber - The PR number to use as prefix
 * @param distPath - Path to the dist directory containing WASM files
 * @param bucketName - R2 bucket name (default: collects-wasm)
 */
export async function uploadWasmToR2(
  prNumber: string,
  distPath: string,
  bucketName: string = "collects-wasm",
): Promise<void> {
  // Validate inputs to prevent command injection
  if (!isValidPrNumber(prNumber)) {
    throw new Error(`Invalid PR number: ${prNumber}. Must be a positive integer.`);
  }
  if (!isValidBucketName(bucketName)) {
    throw new Error(`Invalid bucket name: ${bucketName}. Must contain only alphanumeric characters and hyphens.`);
  }

  console.log(`Uploading WASM files for PR #${prNumber} to R2...`);

  // Find all .wasm files in the dist directory
  const files = readdirSync(distPath);
  const wasmFiles = files.filter((f) => f.endsWith(".wasm"));

  if (wasmFiles.length === 0) {
    console.log("No WASM files found in dist directory");
    return;
  }

  for (const wasmFile of wasmFiles) {
    // Use basename to ensure we only get the filename, not a path traversal
    const safeFilename = basename(wasmFile);
    if (!isValidR2Key(safeFilename)) {
      console.warn(`Skipping file with invalid name: ${wasmFile}`);
      continue;
    }

    const localPath = join(distPath, safeFilename);
    const r2Key = `pr-${prNumber}/${safeFilename}`;

    const size = statSync(localPath).size;
    console.log(`Uploading ${safeFilename} (${(size / 1024).toFixed(2)} KB) to ${r2Key}...`);

    // Use wrangler r2 to upload - inputs are validated above
    await $`pnpm wrangler r2 object put ${bucketName}/${r2Key} --file ${localPath} --content-type application/wasm`;

    console.log(`✓ Uploaded ${safeFilename}`);
  }

  console.log(`Successfully uploaded ${wasmFiles.length} WASM file(s) for PR #${prNumber}`);
}

/**
 * Delete all WASM files for a PR from R2
 *
 * @param prNumber - The PR number to delete files for
 * @param bucketName - R2 bucket name (default: collects-wasm)
 */
export async function deleteWasmFromR2(
  prNumber: string,
  bucketName: string = "collects-wasm",
): Promise<void> {
  // Validate inputs to prevent command injection
  if (!isValidPrNumber(prNumber)) {
    throw new Error(`Invalid PR number: ${prNumber}. Must be a positive integer.`);
  }
  if (!isValidBucketName(bucketName)) {
    throw new Error(`Invalid bucket name: ${bucketName}. Must contain only alphanumeric characters and hyphens.`);
  }

  console.log(`Deleting WASM files for PR #${prNumber} from R2...`);

  const prefix = `pr-${prNumber}/`;

  try {
    // List objects with the PR prefix
    const result =
      await $`pnpm wrangler r2 object list ${bucketName} --prefix ${prefix}`.text();

    // Parse the output to get object keys with proper error handling
    let objects: Array<{ key?: string; Key?: string }>;
    try {
      objects = JSON.parse(result);
    } catch {
      console.log(`Failed to parse R2 list response, no objects found for PR #${prNumber}`);
      return;
    }

    if (!Array.isArray(objects) || objects.length === 0) {
      console.log(`No WASM files found for PR #${prNumber}`);
      return;
    }

    let deletedCount = 0;
    for (const obj of objects) {
      const key = obj.key || obj.Key;
      if (key && isValidR2Key(key)) {
        console.log(`Deleting ${key}...`);
        // Key is validated above to contain only safe characters
        await $`pnpm wrangler r2 object delete ${bucketName}/${key}`;
        deletedCount++;
      } else if (key) {
        console.warn(`Skipping object with invalid key: ${key}`);
      }
    }

    console.log(`Successfully deleted ${deletedCount} WASM file(s) for PR #${prNumber}`);
  } catch (error) {
    // If listing fails, it might be because there are no objects
    console.log(`No WASM files found for PR #${prNumber} or error listing: ${error}`);
  }
}

/**
 * Run the R2 WASM management CLI
 */
export function runR2WasmCli(
  action: "upload" | "delete",
  prNumber: string,
  distPath?: string,
): void {
  if (!prNumber) {
    console.error("PR number is required");
    process.exit(1);
  }

  switch (action) {
    case "upload":
      if (!distPath) {
        console.error("Dist path is required for upload");
        process.exit(1);
      }
      uploadWasmToR2(prNumber, distPath)
        .then(() => process.exit(0))
        .catch((err) => {
          console.error("Upload failed:", err);
          process.exit(1);
        });
      break;
    case "delete":
      deleteWasmFromR2(prNumber)
        .then(() => process.exit(0))
        .catch((err) => {
          console.error("Delete failed:", err);
          process.exit(1);
        });
      break;
    default:
      console.error(`Unknown action: ${action}`);
      process.exit(1);
  }
}
