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
import { join } from "path";

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
  console.log(`Uploading WASM files for PR #${prNumber} to R2...`);

  // Find all .wasm files in the dist directory
  const files = readdirSync(distPath);
  const wasmFiles = files.filter((f) => f.endsWith(".wasm"));

  if (wasmFiles.length === 0) {
    console.log("No WASM files found in dist directory");
    return;
  }

  for (const wasmFile of wasmFiles) {
    const localPath = join(distPath, wasmFile);
    const r2Key = `pr-${prNumber}/${wasmFile}`;

    const size = statSync(localPath).size;
    console.log(`Uploading ${wasmFile} (${(size / 1024).toFixed(2)} KB) to ${r2Key}...`);

    // Use wrangler r2 to upload
    await $`pnpm wrangler r2 object put ${bucketName}/${r2Key} --file ${localPath} --content-type application/wasm`;

    console.log(`✓ Uploaded ${wasmFile}`);
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
  console.log(`Deleting WASM files for PR #${prNumber} from R2...`);

  const prefix = `pr-${prNumber}/`;

  try {
    // List objects with the PR prefix
    const result =
      await $`pnpm wrangler r2 object list ${bucketName} --prefix ${prefix}`.text();

    // Parse the output to get object keys
    // wrangler r2 object list outputs JSON
    const objects = JSON.parse(result);

    if (!objects || objects.length === 0) {
      console.log(`No WASM files found for PR #${prNumber}`);
      return;
    }

    let deletedCount = 0;
    for (const obj of objects) {
      const key = obj.key || obj.Key;
      if (key) {
        console.log(`Deleting ${key}...`);
        await $`pnpm wrangler r2 object delete ${bucketName}/${key}`;
        deletedCount++;
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
