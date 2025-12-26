import { readFileSync } from "fs";
import { execSync } from "child_process";

/**
 * Result of version check operation
 */
export interface VersionCheckResult {
  versionChanged: boolean;
  currentVersion: string;
  previousVersion: string;
}

/**
 * Extracts version from Cargo.toml content
 */
function extractVersion(content: string): string {
  const match = content.match(/^version\s*=\s*["']([^"']+)["']/m);
  return match ? match[1] : "";
}

/**
 * Reads the current version from a Cargo.toml file
 */
function readCurrentVersion(cargoTomlPath: string): string {
  try {
    const content = readFileSync(cargoTomlPath, "utf-8");
    return extractVersion(content);
  } catch (error) {
    console.error(`Error reading ${cargoTomlPath}:`, error);
    return "";
  }
}

/**
 * Reads the previous version from a Cargo.toml file (from HEAD^)
 */
function readPreviousVersion(cargoTomlPath: string): string {
  try {
    const content = execSync(`git show HEAD^:${cargoTomlPath}`, {
      encoding: "utf-8",
    });
    return extractVersion(content);
  } catch (error) {
    // File may not exist in previous commit or this is the first commit
    return "";
  }
}

/**
 * Checks if the version in a Cargo.toml file has changed
 */
export function checkVersionChange(cargoTomlPath: string): VersionCheckResult {
  const currentVersion = readCurrentVersion(cargoTomlPath);
  const previousVersion = readPreviousVersion(cargoTomlPath);

  const versionChanged =
    currentVersion !== previousVersion &&
    previousVersion !== "" &&
    currentVersion !== "";

  return {
    versionChanged,
    currentVersion,
    previousVersion,
  };
}

/**
 * Main entry point when run as a script
 */
if (import.meta.main) {
  const cargoTomlPath = process.argv[2];

  if (!cargoTomlPath) {
    console.error("Usage: bun run version-check.ts <path-to-Cargo.toml>");
    process.exit(1);
  }

  const result = checkVersionChange(cargoTomlPath);

  console.log(`Current version: ${result.currentVersion}`);
  console.log(`Previous version: ${result.previousVersion}`);
  console.log(`Version changed: ${result.versionChanged}`);

  // Output for GitHub Actions
  if (process.env.GITHUB_OUTPUT) {
    const fs = require("fs");
    const output = `version_changed=${result.versionChanged}\ncurrent_version=${result.currentVersion}\nprevious_version=${result.previousVersion}\n`;
    fs.appendFileSync(process.env.GITHUB_OUTPUT, output);
  }
}
