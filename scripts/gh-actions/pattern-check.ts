/**
 * Pattern Check - Enforce coding standards via regex pattern matching
 *
 * This script scans files for forbidden patterns and reports violations
 * with explanations for why certain patterns are not allowed.
 *
 * Configuration is defined in `.pattern-checks.jsonc` at the repository root.
 *
 * Example use cases:
 * - Prevent use of certain crates (e.g., use tracing instead of println!)
 * - Enforce coding conventions (e.g., no unwrap() in production code)
 * - Detect security anti-patterns (e.g., hardcoded secrets patterns)
 */

import { readdir, readFile, stat } from "fs/promises";
import { join, dirname, relative } from "path";
import { fileURLToPath } from "url";

// Get the project root directory (two levels up from scripts/gh-actions/)
const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, "..", "..");

const CONFIG_FILE = join(PROJECT_ROOT, ".pattern-checks.jsonc");

/**
 * Severity levels for pattern violations
 */
export type Severity = "error" | "warning";

/**
 * A single pattern check rule
 */
export interface PatternRule {
  /** Unique identifier for this rule */
  id: string;
  /** Regex pattern to search for (JavaScript regex syntax) */
  pattern: string;
  /** Glob patterns for files to check */
  files: string[];
  /** Glob patterns for files to exclude (optional) */
  exclude?: string[];
  /** Severity level: "error" causes CI failure, "warning" is informational */
  severity: Severity;
  /** Short message describing the violation */
  message: string;
  /** Detailed explanation of why this pattern is forbidden */
  explanation: string;
}

/**
 * Configuration file structure
 */
export interface PatternCheckConfig {
  version: number;
  description: string;
  rules: PatternRule[];
}

/**
 * A single violation found in a file
 */
export interface Violation {
  rule: PatternRule;
  file: string;
  line: number;
  column: number;
  match: string;
  lineContent: string;
}

/**
 * Result of running pattern checks
 */
export interface PatternCheckResult {
  success: boolean;
  violations: Violation[];
  checkedFiles: number;
  errorCount: number;
  warningCount: number;
}

/**
 * Strip comments from JSONC content
 * Supports // line comments and /* block comments *\/
 */
function stripJsoncComments(content: string): string {
  let result = "";
  let i = 0;
  let inString = false;
  let stringChar = "";

  while (i < content.length) {
    const char = content[i];
    const nextChar = content[i + 1];

    // Handle string start/end - count consecutive backslashes to handle escaped backslashes
    if ((char === '"' || char === "'") && !inString) {
      inString = true;
      stringChar = char;
      result += char;
      i++;
      continue;
    }

    if (inString && char === stringChar) {
      // Count preceding backslashes to determine if quote is escaped
      let backslashCount = 0;
      let j = i - 1;
      while (j >= 0 && content[j] === "\\") {
        backslashCount++;
        j--;
      }
      // Quote is escaped if preceded by odd number of backslashes
      if (backslashCount % 2 === 0) {
        inString = false;
      }
      result += char;
      i++;
      continue;
    }

    // Skip comments only when not in a string
    if (!inString) {
      // Line comment
      if (char === "/" && nextChar === "/") {
        // Skip until end of line
        while (i < content.length && content[i] !== "\n") {
          i++;
        }
        continue;
      }
      // Block comment
      if (char === "/" && nextChar === "*") {
        i += 2;
        while (i < content.length) {
          if (
            content[i] === "*" &&
            i + 1 < content.length &&
            content[i + 1] === "/"
          ) {
            i += 2;
            break;
          }
          i++;
        }
        continue;
      }
    }

    result += char;
    i++;
  }

  return result;
}

/**
 * Load the pattern check configuration file (supports JSONC with comments)
 */
export async function loadConfig(
  configPath: string = CONFIG_FILE,
): Promise<PatternCheckConfig> {
  try {
    const content = await readFile(configPath, "utf-8");
    const jsonContent = stripJsoncComments(content);
    return JSON.parse(jsonContent) as PatternCheckConfig;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      // Return empty config if file doesn't exist
      return {
        version: 1,
        description: "No pattern checks configured",
        rules: [],
      };
    }
    throw error;
  }
}

/**
 * Simple glob pattern matching (supports * and ** wildcards)
 *
 * Glob patterns:
 * - `*` matches any characters except `/`
 * - `**` matches any characters including `/` (any depth of directories)
 * - `**\/` at start matches any directory prefix (including no prefix)
 * - `\/**` at end matches any suffix
 * - `\/**\/` in middle matches any directories between
 */
export function matchGlob(pattern: string, filePath: string): boolean {
  // Normalize path separators
  const normalizedPath = filePath.replace(/\\/g, "/");
  const normalizedPattern = pattern.replace(/\\/g, "/");

  // Build regex from glob pattern
  // Process the pattern in segments
  const segments = normalizedPattern.split("/");
  const regexParts: string[] = [];

  for (let i = 0; i < segments.length; i++) {
    const segment = segments[i];

    if (segment === "**") {
      if (i === 0) {
        // **/ at start - match any prefix (including empty)
        // Look ahead: if there are more segments, we need to handle the optional prefix
        if (i < segments.length - 1) {
          regexParts.push("(.*\\/)?");
        } else {
          // ** at end without / - match anything
          regexParts.push(".*");
        }
      } else if (i === segments.length - 1) {
        // /** at end - match any suffix
        regexParts.push(".*");
      } else {
        // /**/ in middle - match any path segments
        regexParts.push("(.*\\/)?");
      }
    } else {
      // Regular segment - escape special chars and convert wildcards
      // First handle * and ? by replacing them with placeholders
      let segmentPattern = segment
        .replace(/\*/g, "<<<STAR>>>")
        .replace(/\?/g, "<<<QUESTION>>>");

      // Escape all regex special characters
      segmentPattern = segmentPattern
        .replace(/[.+^$|()[\]\\]/g, "\\$&")
        .replace(/{/g, "\\{")
        .replace(/}/g, "\\}");

      // Restore * and ? as regex patterns
      segmentPattern = segmentPattern
        .replace(/<<<STAR>>>/g, "[^/]*")
        .replace(/<<<QUESTION>>>/g, "[^/]");

      if (i > 0 && segments[i - 1] !== "**") {
        regexParts.push("\\/");
      }
      regexParts.push(segmentPattern);
    }
  }

  const regexPattern = "^" + regexParts.join("") + "$";

  try {
    return new RegExp(regexPattern).test(normalizedPath);
  } catch {
    // Invalid regex pattern
    return false;
  }
}

/**
 * Check if a file matches any of the given glob patterns
 */
export function matchesAnyGlob(patterns: string[], filePath: string): boolean {
  return patterns.some((pattern) => matchGlob(pattern, filePath));
}

/**
 * Recursively get all files in a directory
 */
async function getAllFiles(
  dir: string,
  basePath: string = dir,
): Promise<string[]> {
  const files: string[] = [];
  const entries = await readdir(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      // Skip hidden directories and common non-source directories
      if (
        entry.name.startsWith(".") ||
        entry.name === "node_modules" ||
        entry.name === "target"
      ) {
        continue;
      }
      files.push(...(await getAllFiles(fullPath, basePath)));
    } else if (entry.isFile()) {
      // Return relative path from base
      files.push(relative(basePath, fullPath).replace(/\\/g, "/"));
    }
  }

  return files;
}

/**
 * Get files that should be checked for a rule
 */
export async function getFilesForRule(
  rule: PatternRule,
  rootDir: string,
): Promise<string[]> {
  const allFiles = await getAllFiles(rootDir);

  return allFiles.filter((file) => {
    // Must match at least one include pattern
    if (!matchesAnyGlob(rule.files, file)) {
      return false;
    }

    // Must not match any exclude pattern
    if (rule.exclude && matchesAnyGlob(rule.exclude, file)) {
      return false;
    }

    return true;
  });
}

/**
 * Check a file for pattern violations
 */
export async function checkFile(
  filePath: string,
  rule: PatternRule,
): Promise<Violation[]> {
  const violations: Violation[] = [];

  try {
    const content = await readFile(filePath, "utf-8");
    const lines = content.split("\n");
    const regex = new RegExp(rule.pattern, "g");

    for (let lineIndex = 0; lineIndex < lines.length; lineIndex++) {
      const line = lines[lineIndex];
      let match: RegExpExecArray | null;

      // Reset regex lastIndex for each line to ensure the global regex
      // doesn't carry state from previous line matches
      regex.lastIndex = 0;

      while ((match = regex.exec(line)) !== null) {
        violations.push({
          rule,
          file: filePath,
          line: lineIndex + 1, // 1-indexed
          column: match.index + 1, // 1-indexed
          match: match[0],
          lineContent: line,
        });

        // Prevent infinite loop on zero-width matches (e.g., patterns like /(?=a)/g)
        // by manually advancing lastIndex when the match is empty
        if (match.index === regex.lastIndex) {
          regex.lastIndex++;
        }
      }
    }
  } catch (error) {
    // Skip files that can't be read (binary files, permission issues, etc.)
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      // Only warn for unexpected errors
      console.warn(`Warning: Could not read file ${filePath}: ${error}`);
    }
  }

  return violations;
}

/**
 * Run pattern checks on the repository
 */
export async function runPatternCheck(options: {
  configPath?: string;
  rootDir?: string;
  verbose?: boolean;
}): Promise<PatternCheckResult> {
  const configPath = options.configPath ?? CONFIG_FILE;
  const rootDir = options.rootDir ?? PROJECT_ROOT;
  const verbose = options.verbose ?? false;

  const violations: Violation[] = [];
  let checkedFiles = 0;
  const checkedFilesSet = new Set<string>();

  // Check if root directory exists
  try {
    await stat(rootDir);
  } catch {
    return {
      success: false,
      violations: [],
      checkedFiles: 0,
      errorCount: 0,
      warningCount: 0,
    };
  }

  // Load configuration
  const config = await loadConfig(configPath);

  if (config.rules.length === 0) {
    if (verbose) {
      console.log("No pattern check rules configured");
    }
    return {
      success: true,
      violations: [],
      checkedFiles: 0,
      errorCount: 0,
      warningCount: 0,
    };
  }

  if (verbose) {
    console.log(`Loaded ${config.rules.length} pattern check rule(s)`);
  }

  // Check each rule
  for (const rule of config.rules) {
    if (verbose) {
      console.log(`\nChecking rule: ${rule.id}`);
    }

    const files = await getFilesForRule(rule, rootDir);

    if (verbose) {
      console.log(`  Found ${files.length} file(s) to check`);
    }

    for (const file of files) {
      const fullPath = join(rootDir, file);
      checkedFilesSet.add(file);

      const fileViolations = await checkFile(fullPath, rule);

      if (fileViolations.length > 0) {
        // Update file path to be relative
        for (const v of fileViolations) {
          v.file = file;
        }
        violations.push(...fileViolations);
      }
    }
  }

  checkedFiles = checkedFilesSet.size;

  // Count by severity
  const errorCount = violations.filter(
    (v) => v.rule.severity === "error",
  ).length;
  const warningCount = violations.filter(
    (v) => v.rule.severity === "warning",
  ).length;

  return {
    success: errorCount === 0,
    violations,
    checkedFiles,
    errorCount,
    warningCount,
  };
}

/**
 * Format a violation for display
 */
export function formatViolation(violation: Violation): string {
  const severityIcon = violation.rule.severity === "error" ? "❌" : "⚠️";
  const severityLabel =
    violation.rule.severity === "error" ? "error" : "warning";

  return [
    `${severityIcon} ${violation.file}:${violation.line}:${violation.column} [${severityLabel}]`,
    `   Rule: ${violation.rule.id}`,
    `   Message: ${violation.rule.message}`,
    `   Found: ${violation.match}`,
    `   Line: ${violation.lineContent.trim()}`,
    `   Explanation: ${violation.rule.explanation}`,
  ].join("\n");
}

/**
 * CLI entry point for pattern check
 */
export async function runPatternCheckCLI(): Promise<void> {
  console.log("Running pattern checks...\n");

  const result = await runPatternCheck({ verbose: true });

  if (result.violations.length > 0) {
    console.log("\n--- Violations Found ---\n");

    // Group violations by rule for better readability
    const violationsByRule = new Map<string, Violation[]>();
    for (const v of result.violations) {
      const existing = violationsByRule.get(v.rule.id) ?? [];
      existing.push(v);
      violationsByRule.set(v.rule.id, existing);
    }

    for (const [ruleId, ruleViolations] of violationsByRule) {
      const rule = ruleViolations[0].rule;
      console.log(`\n### Rule: ${ruleId}`);
      console.log(`    ${rule.message}`);
      console.log(`    Explanation: ${rule.explanation}\n`);

      for (const v of ruleViolations) {
        const severityIcon = v.rule.severity === "error" ? "❌" : "⚠️";
        console.log(`  ${severityIcon} ${v.file}:${v.line}:${v.column}`);
        console.log(`     ${v.lineContent.trim()}`);
      }
    }
  }

  console.log("\n--- Summary ---");
  console.log(`Files checked: ${result.checkedFiles}`);
  console.log(`Errors: ${result.errorCount}`);
  console.log(`Warnings: ${result.warningCount}`);

  if (!result.success) {
    console.error("\n❌ Pattern check failed");
    process.exit(1);
  }

  console.log("\n✅ Pattern check passed");
}
