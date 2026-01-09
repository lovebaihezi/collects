/**
 * Pattern Check - Enforce coding standards via ast-grep and ripgrep
 *
 * This script scans files for forbidden patterns and reports violations
 * with explanations for why certain patterns are not allowed.
 *
 * Uses:
 * - @ast-grep/napi for AST-based semantic pattern matching
 * - ripgrep-js for fast regex-based text search (requires ripgrep binary)
 * - Bun.file() for native Bun file I/O operations
 *
 * Configuration is defined in `.pattern-checks.jsonc` at the repository root.
 *
 * Example use cases:
 * - Prevent use of certain crates (e.g., use tracing instead of println!)
 * - Enforce coding conventions (e.g., no unwrap() in production code)
 * - Detect security anti-patterns (e.g., hardcoded secrets patterns)
 */

/**
 * Write a message directly to stdout with immediate flush.
 * This ensures output is visible immediately on CI systems like GitHub Actions.
 */
function logImmediate(message: string): void {
  Bun.write(Bun.stdout, message + "\n");
}

/**
 * Write an error message directly to stderr with immediate flush.
 */
function logError(message: string): void {
  Bun.write(Bun.stderr, message + "\n");
}

import { readdir } from "fs/promises";
import { join, dirname, relative } from "path";
import { fileURLToPath } from "url";
import { ripGrep, type Match } from "ripgrep-js";
import { parse, findInFiles, Lang, type SgNode } from "@ast-grep/napi";

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
  const file = Bun.file(configPath);
  const exists = await file.exists();

  if (!exists) {
    // Return empty config if file doesn't exist
    return {
      version: 1,
      description: "No pattern checks configured",
      rules: [],
    };
  }

  const content = await file.text();
  const jsonContent = stripJsoncComments(content);
  return JSON.parse(jsonContent) as PatternCheckConfig;
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
 * Map language string to ast-grep Lang enum
 */
function getLangFromString(language: string): Lang | null {
  const langMap: Record<string, Lang> = {
    rust: Lang.Rust,
    typescript: Lang.TypeScript,
    javascript: Lang.JavaScript,
    python: Lang.Python,
    go: Lang.Go,
    java: Lang.Java,
    c: Lang.C,
    cpp: Lang.Cpp,
    csharp: Lang.CSharp,
    kotlin: Lang.Kotlin,
    swift: Lang.Swift,
    ruby: Lang.Ruby,
    html: Lang.Html,
    css: Lang.Css,
    json: Lang.Json,
    yaml: Lang.Yaml,
    toml: Lang.Toml,
  };
  return langMap[language.toLowerCase()] ?? null;
}

/**
 * Check if ripgrep binary is available
 */
async function isRipgrepAvailable(): Promise<boolean> {
  try {
    // ripgrep-js will throw if rg binary is not found
    await ripGrep(process.cwd(), { regex: "^$", globs: ["*.nonexistent"] });
    return true;
  } catch {
    return false;
  }
}

/**
 * Search for patterns using ripgrep-js (requires ripgrep binary)
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

  try {
    // Build glob patterns including exclusions
    const allGlobs = [...globs, ...excludes.map((e) => `!${e}`)];

    const matches: Match[] = await ripGrep(rootDir, {
      regex: pattern,
      globs: allGlobs.length > 0 ? allGlobs : undefined,
    });

    for (const match of matches) {
      const lineText = match.lines.text.replace(/\n$/, "");

      // Extract each submatch
      for (const submatch of match.submatches) {
        results.push({
          file: match.path.text,
          line: match.line_number,
          column: submatch.start + 1, // 1-indexed
          match: submatch.match.text,
          lineContent: lineText,
        });
      }
    }
  } catch (error) {
    // ripgrep-js throws when no matches or binary not found
    // Return empty results
  }

  return results;
}

/**
 * Search for patterns using @ast-grep/napi
 */
async function searchWithAstGrep(
  patternStr: string,
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

  const lang = getLangFromString(language);
  if (!lang) {
    console.warn(`Unsupported language for ast-grep: ${language}`);
    return results;
  }

  try {
    // Extract file extensions from glob patterns to optimize file scanning
    const allowedExtensions = extractExtensionsFromGlobs(globs);

    // Get all files matching the globs with extension filtering
    const files = await getAllFilesRecursive(
      rootDir,
      rootDir,
      allowedExtensions,
    );
    const matchingFiles = files.filter((f) => matchesAnyGlob(globs, f));

    for (const file of matchingFiles) {
      const fullPath = join(rootDir, file);
      try {
        const bunFile = Bun.file(fullPath);
        const content = await bunFile.text();
        const root = parse(lang, content);
        const pattern = root.root().find(patternStr);

        if (pattern) {
          // Find all matches
          const matches = root.root().findAll(patternStr);
          for (const match of matches) {
            const range = match.range();
            const text = match.text();
            const lines = content.split("\n");
            const lineContent = lines[range.start.line] || "";

            results.push({
              file,
              line: range.start.line + 1, // 1-indexed
              column: range.start.column + 1, // 1-indexed
              match: text,
              lineContent,
            });
          }
        }
      } catch {
        // Skip files that can't be parsed
      }
    }
  } catch (error) {
    console.warn(`ast-grep search error: ${error}`);
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
  verbose: boolean = false,
): Promise<Violation[]> {
  const violations: Violation[] = [];
  const patternType = rule.type || "regex";
  const startTime = performance.now();

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
      if (verbose) {
        logImmediate(`  Scanning with ast-grep for ${rule.language} files...`);
      }
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
    if (verbose) {
      logImmediate(`  Scanning with ripgrep...`);
    }
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
    if (verbose) {
      logImmediate(`  Finding files to check...`);
    }
    const fileStartTime = performance.now();
    const files = await getFilesForRule(rule, rootDir);
    const fileEndTime = performance.now();

    if (verbose) {
      logImmediate(
        `  Found ${files.length} file(s) in ${(fileEndTime - fileStartTime).toFixed(0)}ms`,
      );
    }

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      if (verbose) {
        logImmediate(`  Checking [${i + 1}/${files.length}]: ${file}`);
      }
      const fullPath = join(rootDir, file);
      const fileViolations = await checkFileInternal(fullPath, rule);
      for (const v of fileViolations) {
        v.file = file;
      }
      violations.push(...fileViolations);
    }
  }

  const endTime = performance.now();
  if (verbose) {
    logImmediate(`  Completed in ${(endTime - startTime).toFixed(0)}ms`);
  }

  return violations;
}

/**
 * Common directories to skip during file traversal
 */
const SKIP_DIRECTORIES = new Set([
  "node_modules",
  "target",
  "dist",
  "build",
  ".git",
  ".svn",
  ".hg",
  "__pycache__",
  ".cache",
  ".next",
  ".nuxt",
  "coverage",
  ".nyc_output",
  "vendor",
]);

/**
 * Common binary/non-source file extensions to skip
 */
const SKIP_EXTENSIONS = new Set([
  ".exe",
  ".dll",
  ".so",
  ".dylib",
  ".a",
  ".o",
  ".obj",
  ".bin",
  ".png",
  ".jpg",
  ".jpeg",
  ".gif",
  ".ico",
  ".svg",
  ".woff",
  ".woff2",
  ".ttf",
  ".eot",
  ".pdf",
  ".zip",
  ".tar",
  ".gz",
  ".rar",
  ".7z",
  ".lock",
  ".sum",
]);

/**
 * Extract file extensions from glob patterns (e.g., `**\/*.rs` -> `.rs`)
 */
function extractExtensionsFromGlobs(globs: string[]): Set<string> {
  const extensions = new Set<string>();
  for (const glob of globs) {
    // Match patterns like "*.rs", "**/*.rs", "src/**/*.ts"
    const match = glob.match(/\*\.(\w+)$/);
    if (match) {
      extensions.add(`.${match[1]}`);
    }
  }
  return extensions;
}

/**
 * Recursively get all files in a directory (fallback when ripgrep not available)
 * Optimized to skip non-source directories and files early
 */
async function getAllFilesRecursive(
  dir: string,
  basePath: string = dir,
  allowedExtensions?: Set<string>,
  depth: number = 0,
  maxDepth: number = 20,
): Promise<string[]> {
  // Prevent infinite recursion
  if (depth > maxDepth) {
    console.warn(
      `Warning: Max directory depth (${maxDepth}) reached at ${dir}`,
    );
    return [];
  }

  const files: string[] = [];
  const entries = await readdir(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = join(dir, entry.name);
    if (entry.isDirectory()) {
      // Skip hidden directories and common non-source directories
      if (entry.name.startsWith(".") || SKIP_DIRECTORIES.has(entry.name)) {
        continue;
      }
      files.push(
        ...(await getAllFilesRecursive(
          fullPath,
          basePath,
          allowedExtensions,
          depth + 1,
          maxDepth,
        )),
      );
    } else if (entry.isFile()) {
      // Skip binary/non-source files
      const ext = entry.name.includes(".")
        ? `.${entry.name.split(".").pop()}`
        : "";
      if (SKIP_EXTENSIONS.has(ext.toLowerCase())) {
        continue;
      }

      // If we have specific extensions to look for, filter early
      if (allowedExtensions && allowedExtensions.size > 0) {
        if (!allowedExtensions.has(ext.toLowerCase())) {
          continue;
        }
      }

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
  // Extract file extensions from glob patterns to optimize file scanning
  const allowedExtensions = extractExtensionsFromGlobs(rule.files);

  // Use Node.js implementation for file listing with extension filtering
  const allFiles = await getAllFilesRecursive(
    rootDir,
    rootDir,
    allowedExtensions,
  );

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
    const bunFile = Bun.file(filePath);
    const exists = await bunFile.exists();
    if (!exists) {
      return violations;
    }

    const content = await bunFile.text();
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
    console.warn(`Warning: Could not read file ${filePath}: ${error}`);
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
  const overallStartTime = performance.now();

  const violations: Violation[] = [];
  const checkedFilesSet = new Set<string>();

  // Check if root directory exists by trying to read it
  try {
    await readdir(rootDir);
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
  // ast-grep is always available via @ast-grep/napi
  // ripgrep requires the binary to be installed
  const hasRipgrep = await isRipgrepAvailable();
  const hasAstGrep = true; // @ast-grep/napi is always available

  if (verbose) {
    logImmediate(
      `Tools available: ripgrep=${hasRipgrep}, ast-grep=${hasAstGrep} (via npm packages)`,
    );
  }

  // Load configuration
  const config = await loadConfig(configPath);

  if (config.rules.length === 0) {
    if (verbose) {
      logImmediate("No pattern check rules configured");
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
    logImmediate(`Loaded ${config.rules.length} pattern check rule(s)`);
  }

  // Check each rule using appropriate tool
  for (const rule of config.rules) {
    if (verbose) {
      const toolName =
        rule.type === "ast"
          ? "@ast-grep/napi"
          : hasRipgrep
            ? "ripgrep-js"
            : "internal";
      logImmediate(`\nChecking rule: ${rule.id} (using ${toolName})`);
    }

    const ruleViolations = await checkWithTools(
      rule,
      rootDir,
      hasRipgrep,
      hasAstGrep,
      verbose,
    );

    // Track checked files
    for (const v of ruleViolations) {
      checkedFilesSet.add(v.file);
    }

    violations.push(...ruleViolations);

    if (verbose) {
      logImmediate(`  Found ${ruleViolations.length} violation(s)`);
    }
  }

  // Count by severity
  const errorCount = violations.filter(
    (v) => v.rule.severity === "error",
  ).length;
  const warningCount = violations.filter(
    (v) => v.rule.severity === "warning",
  ).length;

  const overallEndTime = performance.now();
  if (verbose) {
    logImmediate(
      `\nTotal time: ${(overallEndTime - overallStartTime).toFixed(0)}ms`,
    );
  }

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
  logImmediate("Running pattern checks...\n");

  const result = await runPatternCheck({ verbose: true });

  if (result.violations.length > 0) {
    logImmediate("\n--- Violations Found ---\n");

    // Group violations by rule for better readability
    const violationsByRule = new Map<string, Violation[]>();
    for (const v of result.violations) {
      const existing = violationsByRule.get(v.rule.id) ?? [];
      existing.push(v);
      violationsByRule.set(v.rule.id, existing);
    }

    for (const [ruleId, ruleViolations] of violationsByRule) {
      const rule = ruleViolations[0].rule;
      logImmediate(`\n### Rule: ${ruleId}`);
      logImmediate(`    ${rule.message}`);
      logImmediate(`    Explanation: ${rule.explanation}\n`);

      for (const v of ruleViolations) {
        const severityIcon = v.rule.severity === "error" ? "❌" : "⚠️";
        logImmediate(`  ${severityIcon} ${v.file}:${v.line}:${v.column}`);
        logImmediate(`     ${v.lineContent.trim()}`);
      }
    }
  }

  logImmediate("\n--- Summary ---");
  logImmediate(`Files checked: ${result.checkedFiles}`);
  logImmediate(`Errors: ${result.errorCount}`);
  logImmediate(`Warnings: ${result.warningCount}`);

  if (!result.success) {
    logError("\n❌ Pattern check failed");
    process.exit(1);
  }

  logImmediate("\n✅ Pattern check passed");
}
