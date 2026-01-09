/**
 * Pattern Check - Enforce coding standards via ast-grep and ripgrep
 *
 * This script scans files for forbidden patterns and reports violations
 * with explanations for why certain patterns are not allowed.
 *
 * Uses:
 * - ripgrep (rg) for fast regex-based text search
 * - ast-grep (sg) for AST-based semantic pattern matching
 *
 * Configuration is defined in `.pattern-checks.jsonc` at the repository root.
 *
 * Example use cases:
 * - Prevent use of certain crates (e.g., use tracing instead of println!)
 * - Enforce coding conventions (e.g., no unwrap() in production code)
 * - Detect security anti-patterns (e.g., hardcoded secrets patterns)
 */

import { readFile, stat, readdir } from "fs/promises";
import { join, dirname, relative } from "path";
import { fileURLToPath } from "url";
import { spawn } from "child_process";

// Get the project root directory (two levels up from scripts/gh-actions/)
const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, "..", "..");

const CONFIG_FILE = join(PROJECT_ROOT, ".pattern-checks.jsonc");

/**
 * Severity levels for pattern violations
 */
export type Severity = "error" | "warning";

/**
 * Type of pattern matching engine to use
 */
export type PatternType = "regex" | "ast";

/**
 * A single pattern check rule
 */
export interface PatternRule {
  /** Unique identifier for this rule */
  id: string;
  /** Pattern to search for (regex for ripgrep, or ast-grep pattern) */
  pattern: string;
  /** Type of pattern: "regex" uses ripgrep, "ast" uses ast-grep */
  type?: PatternType;
  /** Language for ast-grep patterns (e.g., "rust", "typescript") */
  language?: string;
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
 * Check if a command exists on the system
 */
async function commandExists(command: string): Promise<boolean> {
  return new Promise((resolve) => {
    const proc = spawn("which", [command], {
      stdio: ["ignore", "pipe", "pipe"],
    });
    proc.on("close", (code) => resolve(code === 0));
    proc.on("error", () => resolve(false));
  });
}

/**
 * Run a command and capture its output
 */
async function runCommand(
  command: string,
  args: string[],
  cwd: string,
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  return new Promise((resolve) => {
    const proc = spawn(command, args, {
      cwd,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";

    proc.stdout.on("data", (data: Buffer) => {
      stdout += data.toString();
    });

    proc.stderr.on("data", (data: Buffer) => {
      stderr += data.toString();
    });

    proc.on("close", (code) => {
      resolve({ stdout, stderr, exitCode: code ?? 0 });
    });

    proc.on("error", (err) => {
      resolve({ stdout, stderr: err.message, exitCode: 1 });
    });
  });
}

/**
 * Search for patterns using ripgrep (fast regex-based search)
 */
async function searchWithRipgrep(
  pattern: string,
  globs: string[],
  excludes: string[],
  rootDir: string,
): Promise<
  Array<{
    file: string;
    line: number;
    column: number;
    match: string;
    lineContent: string;
  }>
> {
  const results: Array<{
    file: string;
    line: number;
    column: number;
    match: string;
    lineContent: string;
  }> = [];

  // Build ripgrep arguments
  const args = [
    "--json", // JSON output for structured parsing
    "--no-heading",
    "--line-number",
    "--column",
    "-e",
    pattern,
  ];

  // Add glob patterns
  for (const glob of globs) {
    args.push("--glob", glob);
  }

  // Add exclude patterns
  for (const exclude of excludes) {
    args.push("--glob", `!${exclude}`);
  }

  // Add the search directory
  args.push(".");

  const { stdout, exitCode } = await runCommand("rg", args, rootDir);

  // Exit code 1 means no matches (not an error), 0 means matches found
  if (exitCode > 1) {
    return results;
  }

  // Parse JSON lines output
  const lines = stdout.trim().split("\n").filter(Boolean);
  for (const line of lines) {
    try {
      const parsed = JSON.parse(line);
      if (parsed.type === "match") {
        const data = parsed.data;
        const filePath = data.path.text;
        const lineNum = data.line_number;
        const lineText = data.lines.text.replace(/\n$/, "");

        // Extract each submatch
        for (const submatch of data.submatches) {
          results.push({
            file: filePath,
            line: lineNum,
            column: submatch.start + 1, // 1-indexed
            match: submatch.match.text,
            lineContent: lineText,
          });
        }
      }
    } catch {
      // Skip malformed JSON lines
    }
  }

  return results;
}

/**
 * Search for patterns using ast-grep (AST-based semantic search)
 */
async function searchWithAstGrep(
  pattern: string,
  language: string,
  globs: string[],
  rootDir: string,
): Promise<
  Array<{
    file: string;
    line: number;
    column: number;
    match: string;
    lineContent: string;
  }>
> {
  const results: Array<{
    file: string;
    line: number;
    column: number;
    match: string;
    lineContent: string;
  }> = [];

  // Build ast-grep arguments
  const args = ["scan", "--json", "-p", pattern, "-l", language];

  const { stdout, exitCode } = await runCommand("sg", args, rootDir);

  if (exitCode !== 0) {
    return results;
  }

  // Parse JSON output
  try {
    const parsed = JSON.parse(stdout);
    if (Array.isArray(parsed)) {
      for (const match of parsed) {
        results.push({
          file: match.file || match.path || "",
          line: match.range?.start?.line || match.start?.line || 1,
          column: match.range?.start?.column || match.start?.column || 1,
          match: match.text || match.matched || "",
          lineContent: match.lines || match.text || "",
        });
      }
    }
  } catch {
    // Try parsing as JSON lines
    const lines = stdout.trim().split("\n").filter(Boolean);
    for (const line of lines) {
      try {
        const match = JSON.parse(line);
        results.push({
          file: match.file || match.path || "",
          line: match.range?.start?.line || match.start?.line || 1,
          column: match.range?.start?.column || match.start?.column || 1,
          match: match.text || match.matched || "",
          lineContent: match.lines || match.text || "",
        });
      } catch {
        // Skip malformed lines
      }
    }
  }

  return results;
}

/**
 * Check files for pattern violations using the appropriate search tool
 */
export async function checkWithTools(
  rule: PatternRule,
  rootDir: string,
  useRipgrep: boolean,
  useAstGrep: boolean,
): Promise<Violation[]> {
  const violations: Violation[] = [];
  const patternType = rule.type || "regex";

  if (patternType === "ast") {
    // Validate language is provided for AST patterns
    if (!rule.language) {
      console.error(
        `Rule ${rule.id}: 'language' field is required when type is 'ast'`,
      );
      return violations;
    }

    if (useAstGrep) {
      // Use ast-grep for AST-based patterns
      const results = await searchWithAstGrep(
        rule.pattern,
        rule.language,
        rule.files,
        rootDir,
      );

      for (const result of results) {
        // Check if file matches include/exclude patterns
        if (rule.exclude && matchesAnyGlob(rule.exclude, result.file)) {
          continue;
        }
        if (!matchesAnyGlob(rule.files, result.file)) {
          continue;
        }

        violations.push({
          rule,
          file: result.file,
          line: result.line,
          column: result.column,
          match: result.match,
          lineContent: result.lineContent,
        });
      }
    } else {
      console.warn(
        `Rule ${rule.id}: ast-grep not available, skipping AST-based rule`,
      );
    }
  } else if (useRipgrep) {
    // Use ripgrep for regex-based patterns
    const results = await searchWithRipgrep(
      rule.pattern,
      rule.files,
      rule.exclude || [],
      rootDir,
    );

    for (const result of results) {
      violations.push({
        rule,
        file: result.file,
        line: result.line,
        column: result.column,
        match: result.match,
        lineContent: result.lineContent,
      });
    }
  } else {
    // Fallback to internal implementation
    const files = await getFilesForRule(rule, rootDir);
    for (const file of files) {
      const fullPath = join(rootDir, file);
      const fileViolations = await checkFileInternal(fullPath, rule);
      for (const v of fileViolations) {
        v.file = file;
      }
      violations.push(...fileViolations);
    }
  }

  return violations;
}

/**
 * Recursively get all files in a directory (fallback when ripgrep not available)
 */
async function getAllFilesRecursive(
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
      files.push(...(await getAllFilesRecursive(fullPath, basePath)));
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
  // Try to use ripgrep first (much faster for large repos)
  const hasRipgrep = await commandExists("rg");

  if (hasRipgrep) {
    const args = ["--files"];

    for (const glob of rule.files) {
      args.push("--glob", glob);
    }

    for (const exclude of rule.exclude || []) {
      args.push("--glob", `!${exclude}`);
    }

    args.push(".");

    const { stdout } = await runCommand("rg", args, rootDir);
    return stdout.trim().split("\n").filter(Boolean);
  }

  // Fallback to Node.js implementation
  const allFiles = await getAllFilesRecursive(rootDir);

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
 * Internal fallback: Check a file for pattern violations using JS regex
 */
async function checkFileInternal(
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

      // Reset regex lastIndex for each line
      regex.lastIndex = 0;

      while ((match = regex.exec(line)) !== null) {
        violations.push({
          rule,
          file: filePath,
          line: lineIndex + 1,
          column: match.index + 1,
          match: match[0],
          lineContent: line,
        });

        if (match.index === regex.lastIndex) {
          regex.lastIndex++;
        }
      }
    }
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
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

  // Check if external tools are available
  const hasRipgrep = await commandExists("rg");
  const hasAstGrep = await commandExists("sg");

  if (verbose) {
    console.log(
      `Tools available: ripgrep=${hasRipgrep}, ast-grep=${hasAstGrep}`,
    );
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

  // Check each rule using appropriate tool
  for (const rule of config.rules) {
    if (verbose) {
      const toolName =
        rule.type === "ast" ? "ast-grep" : hasRipgrep ? "ripgrep" : "internal";
      console.log(`\nChecking rule: ${rule.id} (using ${toolName})`);
    }

    const ruleViolations = await checkWithTools(
      rule,
      rootDir,
      hasRipgrep,
      hasAstGrep,
    );

    // Track checked files
    for (const v of ruleViolations) {
      checkedFilesSet.add(v.file);
    }

    violations.push(...ruleViolations);

    if (verbose) {
      console.log(`  Found ${ruleViolations.length} violation(s)`);
    }
  }

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
    checkedFiles: checkedFilesSet.size,
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
