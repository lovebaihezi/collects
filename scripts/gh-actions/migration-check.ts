/**
 * Migration Integrity Check
 *
 * This script verifies that existing SQLx migration files haven't been modified.
 * SQLx requires that migrations which have already been applied to a database
 * remain unchanged, otherwise the migration history becomes inconsistent.
 *
 * How it works:
 * 1. A checksum file (services/migrations/.checksums.json) stores SHA256 hashes
 *    of all migration files that have been "locked" (i.e., applied to production).
 * 2. On each check, we verify that locked migrations haven't changed.
 * 3. New migrations (not in the checksum file) are allowed.
 * 4. The `--update` flag adds new migrations to the checksum file.
 */

import { createHash } from "crypto";
import { readdir, readFile, writeFile, stat } from "fs/promises";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

// Get the project root directory (two levels up from scripts/gh-actions/)
const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, "..", "..");

const MIGRATIONS_DIR = join(PROJECT_ROOT, "services/migrations");
const CHECKSUM_FILE = join(PROJECT_ROOT, "services/migrations/.checksums.json");

interface ChecksumRecord {
  filename: string;
  sha256: string;
  lockedAt: string; // ISO timestamp when this migration was locked
}

interface ChecksumFile {
  version: number;
  description: string;
  migrations: ChecksumRecord[];
}

/**
 * Calculate SHA256 hash of file contents
 */
async function hashFile(filepath: string): Promise<string> {
  const content = await readFile(filepath);
  return createHash("sha256").update(content).digest("hex");
}

/**
 * Get all .sql migration files from the migrations directory
 */
async function getMigrationFiles(migrationsDir: string): Promise<string[]> {
  const entries = await readdir(migrationsDir);
  return entries.filter((f) => f.endsWith(".sql")).sort(); // SQLx migrations are sorted by filename (timestamp prefix)
}

/**
 * Load existing checksum file or return empty structure
 */
async function loadChecksums(checksumPath: string): Promise<ChecksumFile> {
  try {
    const content = await readFile(checksumPath, "utf-8");
    return JSON.parse(content) as ChecksumFile;
  } catch {
    // File doesn't exist or is invalid, return empty structure
    return {
      version: 1,
      description:
        "Migration file checksums. DO NOT manually edit. Run `just scripts::migration-lock` to update.",
      migrations: [],
    };
  }
}

/**
 * Save checksum file
 */
async function saveChecksums(
  checksumPath: string,
  checksums: ChecksumFile,
): Promise<void> {
  const content = JSON.stringify(checksums, null, 2) + "\n";
  await writeFile(checksumPath, content, "utf-8");
}

/**
 * Check migration integrity
 * Returns true if all checks pass, false otherwise
 */
export async function checkMigrations(options: {
  migrationsDir?: string;
  checksumPath?: string;
  update?: boolean;
  verbose?: boolean;
}): Promise<{ success: boolean; newMigrations: string[]; errors: string[] }> {
  const migrationsDir = options.migrationsDir ?? MIGRATIONS_DIR;
  const checksumPath = options.checksumPath ?? CHECKSUM_FILE;
  const update = options.update ?? false;
  const verbose = options.verbose ?? false;

  const errors: string[] = [];
  const newMigrations: string[] = [];

  // Check if migrations directory exists
  try {
    await stat(migrationsDir);
  } catch {
    errors.push(`Migrations directory not found: ${migrationsDir}`);
    return { success: false, newMigrations, errors };
  }

  // Load existing checksums
  const checksums = await loadChecksums(checksumPath);
  const lockedMap = new Map(checksums.migrations.map((m) => [m.filename, m]));

  // Get current migration files
  const currentFiles = await getMigrationFiles(migrationsDir);

  if (verbose) {
    console.log(`Found ${currentFiles.length} migration file(s)`);
    console.log(`Locked migrations: ${checksums.migrations.length}`);
  }

  // Check each migration file
  for (const filename of currentFiles) {
    const filepath = join(migrationsDir, filename);
    const currentHash = await hashFile(filepath);
    const locked = lockedMap.get(filename);

    if (locked) {
      // This migration is locked - verify it hasn't changed
      if (locked.sha256 !== currentHash) {
        errors.push(
          `Migration file modified: ${filename}\n` +
            `  Expected: ${locked.sha256}\n` +
            `  Got:      ${currentHash}\n` +
            `  This migration was locked on ${locked.lockedAt}.\n` +
            `  Modifying applied migrations breaks database consistency!`,
        );
      } else if (verbose) {
        console.log(`✓ ${filename} (locked, unchanged)`);
      }
    } else {
      // New migration - not yet locked
      newMigrations.push(filename);
      if (verbose) {
        console.log(`○ ${filename} (new, not locked)`);
      }
    }
  }

  // Check for deleted migrations (migrations in checksum but not on disk)
  for (const locked of checksums.migrations) {
    if (!currentFiles.includes(locked.filename)) {
      errors.push(
        `Locked migration file deleted: ${locked.filename}\n` +
          `  This migration was locked on ${locked.lockedAt}.\n` +
          `  Deleting applied migrations breaks database consistency!`,
      );
    }
  }

  // If update mode is enabled and there are new migrations, add them
  if (update && newMigrations.length > 0) {
    const now = new Date().toISOString();
    for (const filename of newMigrations) {
      const filepath = join(migrationsDir, filename);
      const hash = await hashFile(filepath);
      checksums.migrations.push({
        filename,
        sha256: hash,
        lockedAt: now,
      });
      console.log(`Locked: ${filename}`);
    }

    // Sort by filename to maintain order
    checksums.migrations.sort((a, b) => a.filename.localeCompare(b.filename));

    await saveChecksums(checksumPath, checksums);
    console.log(`\nUpdated ${checksumPath}`);
  }

  return {
    success: errors.length === 0,
    newMigrations,
    errors,
  };
}

/**
 * CLI entry point for migration check
 */
export async function runMigrationCheckCLI(options: {
  update?: boolean;
}): Promise<void> {
  console.log("Checking migration file integrity...\n");

  const result = await checkMigrations({
    update: options.update,
    verbose: true,
  });

  if (result.errors.length > 0) {
    console.error("\n❌ Migration integrity check failed:\n");
    for (const error of result.errors) {
      console.error(`  ${error}\n`);
    }
    process.exit(1);
  }

  if (result.newMigrations.length > 0 && !options.update) {
    console.log(
      `\n⚠️  Found ${result.newMigrations.length} new migration(s) not yet locked:`,
    );
    for (const filename of result.newMigrations) {
      console.log(`   - ${filename}`);
    }
    console.log(
      "\nRun `just scripts::migration-lock` to lock these migrations after they've been applied.",
    );
  }

  console.log("\n✅ Migration integrity check passed");
}

/**
 * CLI entry point for locking new migrations
 */
export async function runMigrationLockCLI(): Promise<void> {
  console.log("Locking new migration files...\n");

  const result = await checkMigrations({
    update: true,
    verbose: true,
  });

  if (result.errors.length > 0) {
    console.error("\n❌ Migration integrity check failed:\n");
    for (const error of result.errors) {
      console.error(`  ${error}\n`);
    }
    process.exit(1);
  }

  if (result.newMigrations.length === 0) {
    console.log("\nNo new migrations to lock.");
  } else {
    console.log(`\n✅ Locked ${result.newMigrations.length} migration(s)`);
    console.log("Don't forget to commit the updated .checksums.json file!");
  }
}
