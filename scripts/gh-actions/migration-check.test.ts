import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { mkdir, writeFile, rm } from "fs/promises";
import { join } from "path";
import { checkMigrations } from "./migration-check.ts";

const TEST_DIR = join(import.meta.dir, ".test-migrations");
const MIGRATIONS_DIR = join(TEST_DIR, "migrations");
const CHECKSUM_FILE = join(TEST_DIR, "migrations/.checksums.json");

async function createMigration(filename: string, content: string) {
  await writeFile(join(MIGRATIONS_DIR, filename), content, "utf-8");
}

describe("migration-check", () => {
  beforeEach(async () => {
    await mkdir(MIGRATIONS_DIR, { recursive: true });
  });

  afterEach(async () => {
    await rm(TEST_DIR, { recursive: true, force: true });
  });

  test("passes with no migrations and no checksum file", async () => {
    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(true);
    expect(result.errors).toHaveLength(0);
    expect(result.newMigrations).toHaveLength(0);
  });

  test("detects new migrations not yet locked", async () => {
    await createMigration(
      "20240101000000_init.sql",
      "CREATE TABLE test (id INT);",
    );

    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(true);
    expect(result.errors).toHaveLength(0);
    expect(result.newMigrations).toHaveLength(1);
    expect(result.newMigrations[0]).toBe("20240101000000_init.sql");
  });

  test("passes when locked migration is unchanged", async () => {
    const content = "CREATE TABLE test (id INT);";
    await createMigration("20240101000000_init.sql", content);

    // First, lock the migration
    await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
      update: true,
    });

    // Then verify it passes
    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(true);
    expect(result.errors).toHaveLength(0);
    expect(result.newMigrations).toHaveLength(0);
  });

  test("fails when locked migration is modified", async () => {
    const originalContent = "CREATE TABLE test (id INT);";
    await createMigration("20240101000000_init.sql", originalContent);

    // Lock the migration
    await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
      update: true,
    });

    // Modify the migration
    const modifiedContent = "CREATE TABLE test (id UUID);";
    await createMigration("20240101000000_init.sql", modifiedContent);

    // Verify it fails
    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(false);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toContain("Migration file modified");
    expect(result.errors[0]).toContain("20240101000000_init.sql");
  });

  test("fails when locked migration is deleted", async () => {
    const content = "CREATE TABLE test (id INT);";
    await createMigration("20240101000000_init.sql", content);

    // Lock the migration
    await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
      update: true,
    });

    // Delete the migration
    await rm(join(MIGRATIONS_DIR, "20240101000000_init.sql"));

    // Verify it fails
    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(false);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toContain("Locked migration file deleted");
    expect(result.errors[0]).toContain("20240101000000_init.sql");
  });

  test("locks new migrations with update flag", async () => {
    await createMigration(
      "20240101000000_first.sql",
      "CREATE TABLE first (id INT);",
    );

    // Lock first migration
    let result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
      update: true,
    });

    expect(result.success).toBe(true);
    expect(result.newMigrations).toHaveLength(1);

    // Add second migration
    await createMigration(
      "20240102000000_second.sql",
      "CREATE TABLE second (id INT);",
    );

    // Check without update - should show new migration
    result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(true);
    expect(result.newMigrations).toHaveLength(1);
    expect(result.newMigrations[0]).toBe("20240102000000_second.sql");

    // Lock second migration
    result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
      update: true,
    });

    expect(result.success).toBe(true);
    expect(result.newMigrations).toHaveLength(1);

    // Verify both are now locked
    result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(true);
    expect(result.newMigrations).toHaveLength(0);
  });

  test("ignores non-sql files", async () => {
    await createMigration(
      "20240101000000_init.sql",
      "CREATE TABLE test (id INT);",
    );
    await writeFile(join(MIGRATIONS_DIR, "README.md"), "# Migrations", "utf-8");
    await writeFile(join(MIGRATIONS_DIR, ".gitkeep"), "", "utf-8");

    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(true);
    expect(result.newMigrations).toHaveLength(1);
    expect(result.newMigrations[0]).toBe("20240101000000_init.sql");
  });

  test("handles multiple migrations correctly", async () => {
    await createMigration(
      "20240101000000_first.sql",
      "CREATE TABLE first (id INT);",
    );
    await createMigration(
      "20240102000000_second.sql",
      "CREATE TABLE second (id INT);",
    );
    await createMigration(
      "20240103000000_third.sql",
      "CREATE TABLE third (id INT);",
    );

    // Lock all migrations
    await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
      update: true,
    });

    // Modify middle migration
    await createMigration(
      "20240102000000_second.sql",
      "CREATE TABLE second (id UUID);",
    );

    const result = await checkMigrations({
      migrationsDir: MIGRATIONS_DIR,
      checksumPath: CHECKSUM_FILE,
    });

    expect(result.success).toBe(false);
    expect(result.errors).toHaveLength(1);
    expect(result.errors[0]).toContain("20240102000000_second.sql");
  });
});
