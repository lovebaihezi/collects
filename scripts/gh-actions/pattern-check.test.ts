import { describe, test, expect, beforeEach, afterEach } from "bun:test";
import { mkdir, writeFile, rm } from "fs/promises";
import { join } from "path";
import {
  matchGlob,
  matchesAnyGlob,
  loadConfig,
  runPatternCheck,
  type PatternRule,
  type PatternCheckConfig,
} from "./pattern-check.ts";

const TEST_DIR = join(import.meta.dir, ".test-pattern-check");
const CONFIG_FILE = join(TEST_DIR, ".pattern-checks.jsonc");

async function createTestFile(relativePath: string, content: string) {
  const fullPath = join(TEST_DIR, relativePath);
  const dir = fullPath.substring(0, fullPath.lastIndexOf("/"));
  await mkdir(dir, { recursive: true });
  await writeFile(fullPath, content, "utf-8");
}

async function createConfig(config: PatternCheckConfig) {
  await writeFile(CONFIG_FILE, JSON.stringify(config, null, 2), "utf-8");
}

describe("pattern-check", () => {
  beforeEach(async () => {
    await mkdir(TEST_DIR, { recursive: true });
  });

  afterEach(async () => {
    await rm(TEST_DIR, { recursive: true, force: true });
  });

  describe("matchGlob", () => {
    test("matches simple filename", () => {
      expect(matchGlob("test.rs", "test.rs")).toBe(true);
      expect(matchGlob("test.rs", "other.rs")).toBe(false);
    });

    test("matches with * wildcard", () => {
      expect(matchGlob("*.rs", "test.rs")).toBe(true);
      expect(matchGlob("*.rs", "other.rs")).toBe(true);
      expect(matchGlob("*.rs", "test.ts")).toBe(false);
      expect(matchGlob("test.*", "test.rs")).toBe(true);
      expect(matchGlob("test.*", "test.ts")).toBe(true);
    });

    test("matches with ** wildcard", () => {
      expect(matchGlob("**/*.rs", "src/lib.rs")).toBe(true);
      expect(matchGlob("**/*.rs", "src/deep/nested/file.rs")).toBe(true);
      expect(matchGlob("**/*.rs", "file.rs")).toBe(true);
      expect(matchGlob("src/**/*.rs", "src/lib.rs")).toBe(true);
      expect(matchGlob("src/**/*.rs", "other/lib.rs")).toBe(false);
    });

    test("matches directory patterns", () => {
      expect(matchGlob("**/tests/**", "src/tests/unit.rs")).toBe(true);
      expect(matchGlob("**/tests/**", "tests/test.rs")).toBe(true);
      expect(matchGlob("**/bin/**", "src/bin/main.rs")).toBe(true);
    });

    test("matches specific directories", () => {
      expect(matchGlob("**/main.rs", "src/main.rs")).toBe(true);
      expect(matchGlob("**/main.rs", "main.rs")).toBe(true);
      expect(matchGlob("**/main.rs", "src/lib.rs")).toBe(false);
    });
  });

  describe("matchesAnyGlob", () => {
    test("matches if any pattern matches", () => {
      const patterns = ["*.rs", "*.ts"];
      expect(matchesAnyGlob(patterns, "test.rs")).toBe(true);
      expect(matchesAnyGlob(patterns, "test.ts")).toBe(true);
      expect(matchesAnyGlob(patterns, "test.js")).toBe(false);
    });

    test("returns false for empty patterns", () => {
      expect(matchesAnyGlob([], "test.rs")).toBe(false);
    });
  });

  describe("loadConfig", () => {
    test("loads valid config file", async () => {
      const config: PatternCheckConfig = {
        version: 1,
        description: "Test config",
        rules: [
          {
            id: "test-rule",
            pattern: "println!",
            files: ["**/*.rs"],
            severity: "error",
            message: "Test message",
            explanation: "Test explanation",
          },
        ],
      };
      await createConfig(config);

      const loaded = await loadConfig(CONFIG_FILE);
      expect(loaded).toEqual(config);
    });

    test("returns empty config for missing file", async () => {
      const loaded = await loadConfig(join(TEST_DIR, "nonexistent.jsonc"));
      expect(loaded.rules).toHaveLength(0);
    });

    test("loads config with JSONC comments", async () => {
      const jsonc = `{
        // This is a line comment
        "version": 1,
        "description": "Test config with comments",
        /* This is a
           block comment */
        "rules": []
      }`;
      await writeFile(CONFIG_FILE, jsonc, "utf-8");

      const loaded = await loadConfig(CONFIG_FILE);
      expect(loaded.version).toBe(1);
      expect(loaded.description).toBe("Test config with comments");
      expect(loaded.rules).toHaveLength(0);
    });

    test("handles escaped quotes in strings", async () => {
      const jsonc = `{
        "version": 1,
        "description": "String with \\"escaped\\" quotes",
        "rules": []
      }`;
      await writeFile(CONFIG_FILE, jsonc, "utf-8");

      const loaded = await loadConfig(CONFIG_FILE);
      expect(loaded.description).toBe('String with "escaped" quotes');
    });

    test("handles block comment at end of file", async () => {
      const jsonc = `{
        "version": 1,
        "description": "Test",
        "rules": []
      }/* trailing comment */`;
      await writeFile(CONFIG_FILE, jsonc, "utf-8");

      const loaded = await loadConfig(CONFIG_FILE);
      expect(loaded.version).toBe(1);
    });
  });

  describe("runPatternCheck", () => {
    test("passes with no violations", async () => {
      await createTestFile("src/lib.rs", "fn clean() {}");
      await createConfig({
        version: 1,
        description: "Test",
        rules: [
          {
            id: "no-println",
            pattern: "println!\\(",
            files: ["**/*.rs"],
            severity: "error",
            message: "No println!",
            explanation: "Use tracing",
          },
        ],
      });

      const result = await runPatternCheck({
        configPath: CONFIG_FILE,
        rootDir: TEST_DIR,
      });

      expect(result.success).toBe(true);
      expect(result.violations).toHaveLength(0);
      expect(result.errorCount).toBe(0);
    });

    test("fails with error violations", async () => {
      await createTestFile("src/lib.rs", 'println!("hello");');
      await createConfig({
        version: 1,
        description: "Test",
        rules: [
          {
            id: "no-println",
            pattern: "println!\\(",
            files: ["**/*.rs"],
            severity: "error",
            message: "No println!",
            explanation: "Use tracing",
          },
        ],
      });

      const result = await runPatternCheck({
        configPath: CONFIG_FILE,
        rootDir: TEST_DIR,
      });

      expect(result.success).toBe(false);
      expect(result.violations).toHaveLength(1);
      expect(result.errorCount).toBe(1);
    });

    test("passes with only warnings", async () => {
      await createTestFile("src/lib.rs", "let _ = x.unwrap();");
      await createConfig({
        version: 1,
        description: "Test",
        rules: [
          {
            id: "no-unwrap",
            pattern: "\\.unwrap\\(\\)",
            files: ["**/*.rs"],
            severity: "warning",
            message: "Avoid unwrap()",
            explanation: "Use ? or expect() instead",
          },
        ],
      });

      const result = await runPatternCheck({
        configPath: CONFIG_FILE,
        rootDir: TEST_DIR,
      });

      expect(result.success).toBe(true);
      expect(result.violations).toHaveLength(1);
      expect(result.warningCount).toBe(1);
      expect(result.errorCount).toBe(0);
    });

    test("handles multiple rules", async () => {
      await createTestFile(
        "src/lib.rs",
        `println!("hello");
let x = y.unwrap();`,
      );
      await createConfig({
        version: 1,
        description: "Test",
        rules: [
          {
            id: "no-println",
            pattern: "println!\\(",
            files: ["**/*.rs"],
            severity: "error",
            message: "No println!",
            explanation: "Use tracing",
          },
          {
            id: "no-unwrap",
            pattern: "\\.unwrap\\(\\)",
            files: ["**/*.rs"],
            severity: "warning",
            message: "Avoid unwrap()",
            explanation: "Use ? or expect()",
          },
        ],
      });

      const result = await runPatternCheck({
        configPath: CONFIG_FILE,
        rootDir: TEST_DIR,
      });

      expect(result.success).toBe(false);
      expect(result.violations).toHaveLength(2);
      expect(result.errorCount).toBe(1);
      expect(result.warningCount).toBe(1);
    });

    test("passes with empty config", async () => {
      await createTestFile("src/lib.rs", 'println!("hello");');
      await createConfig({
        version: 1,
        description: "Empty",
        rules: [],
      });

      const result = await runPatternCheck({
        configPath: CONFIG_FILE,
        rootDir: TEST_DIR,
      });

      expect(result.success).toBe(true);
      expect(result.checkedFiles).toBe(0);
    });

    test("respects exclude patterns", async () => {
      await createTestFile("src/lib.rs", 'println!("lib");');
      await createTestFile("src/main.rs", 'println!("main");');
      await createConfig({
        version: 1,
        description: "Test",
        rules: [
          {
            id: "no-println",
            pattern: "println!\\(",
            files: ["**/*.rs"],
            exclude: ["**/main.rs"],
            severity: "error",
            message: "No println!",
            explanation: "Use tracing",
          },
        ],
      });

      const result = await runPatternCheck({
        configPath: CONFIG_FILE,
        rootDir: TEST_DIR,
      });

      expect(result.success).toBe(false);
      expect(result.violations).toHaveLength(1);
      expect(result.violations[0].file).toBe("src/lib.rs");
    });
  });
});
