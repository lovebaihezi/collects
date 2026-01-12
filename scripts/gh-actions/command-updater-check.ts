/**
 * Command Updater Check
 *
 * Enforces that Command implementations do NOT use the plain `Updater` in `fn run(...)`
 * and instead use `LatestOnlyUpdater`.
 *
 * Why:
 * - Commands execute end-of-frame from a queue.
 * - Async completion can be out-of-order.
 * - Using `LatestOnlyUpdater` guarantees stale completes can’t publish.
 *
 * Intended usage:
 * - Local: `bun run main.ts command-updater-check`
 * - CI: as a step before/alongside `just ui::wk-build`
 *
 * Notes:
 * - This is a text-based scan, not a Rust parser. It’s intentionally “good enough” and fast.
 * - It flags obvious `fn run(... updater: Updater ...)` patterns.
 * - It ignores `collects/states/src/snapshot.rs` examples/docs (which may mention `Updater`).
 */

import { readdir, readFile, stat } from "fs/promises";
import { join, dirname, relative } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, "..", "..");

// Default workspace root + a few “common to scan” folders.
const DEFAULT_SCAN_DIRS = [
  join(PROJECT_ROOT), // scan all Rust in repo, but filtered by extension and exclusions
];

// Files that may legitimately contain docs/examples referencing `Updater`.
const DEFAULT_EXCLUDE_PATHS = new Set<string>([
  // Known doc/example that still references Updater in comments.
  join(PROJECT_ROOT, "states/src/snapshot.rs"),
]);

const DEFAULT_EXCLUDE_DIR_NAMES = new Set<string>([
  ".git",
  "target",
  "node_modules",
  "dist",
  "build",
  ".turbo",
  ".next",
]);

type Finding = {
  file: string; // absolute path
  line: number; // 1-based
  snippet: string;
};

type CheckOptions = {
  /**
   * Extra directories to scan (absolute or relative-to-project-root).
   * Generally you shouldn’t need this.
   */
  scanDirs?: string[];

  /**
   * Additional file paths to exclude (absolute or relative-to-project-root).
   */
  excludePaths?: string[];

  /**
   * If true, prints all scanned files count and ignored paths.
   */
  verbose?: boolean;
};

function toAbs(p: string): string {
  if (p.startsWith("/") || p.match(/^[A-Za-z]:[\\/]/)) return p;
  return join(PROJECT_ROOT, p);
}

async function isDirectory(path: string): Promise<boolean> {
  try {
    const s = await stat(path);
    return s.isDirectory();
  } catch {
    return false;
  }
}

async function* walk(dir: string): AsyncGenerator<string> {
  const entries = await readdir(dir, { withFileTypes: true });
  for (const ent of entries) {
    if (DEFAULT_EXCLUDE_DIR_NAMES.has(ent.name)) continue;
    const full = join(dir, ent.name);
    if (ent.isDirectory()) {
      yield* walk(full);
    } else if (ent.isFile()) {
      yield full;
    }
  }
}

function isRustFile(path: string): boolean {
  return path.endsWith(".rs");
}

/**
 * Detects `fn run(... updater: Updater ...)` in a pragmatic way.
 *
 * We intentionally:
 * - allow whitespace/newlines between tokens
 * - match both `updater: Updater` and `updater: crate::state::Updater` etc. (any path ending in `Updater`)
 * - but require it's in a `fn run(` signature to reduce false positives.
 *
 * If you refactor naming away from `updater`, this might miss it.
 * (That’s ok; it’s a guardrail, not a verifier.)
 */
function findUpdaterInRunSignatures(fileText: string): Finding[] {
  const findings: Finding[] = [];

  // Quick prefilter for speed
  if (!fileText.includes("fn run") || !fileText.includes("Updater")) {
    return findings;
  }

  // Strategy:
  // - scan line-by-line, but maintain a rolling “signature buffer”
  //   to catch multi-line function signatures.
  const lines = fileText.split("\n");

  const MAX_SIG_LINES = 40;

  for (let i = 0; i < lines.length; i++) {
    // Start signature capture on a line containing `fn run`
    if (!lines[i].includes("fn run")) continue;

    let buf = "";
    const startLine = i;

    for (let j = i; j < Math.min(lines.length, i + MAX_SIG_LINES); j++) {
      buf += lines[j] + "\n";

      // Heuristic: once we reach `{` or `;`, stop scanning this signature
      // (`fn run(...) {` or trait `fn run(...);`)
      if (lines[j].includes("{") || lines[j].includes(";")) {
        break;
      }
    }

    // Ensure this looks like a function signature with run(
    if (!buf.match(/\bfn\s+run\s*\(/)) continue;

    // Exempt correct usage: LatestOnlyUpdater
    if (buf.includes("LatestOnlyUpdater")) {
      continue;
    }

    // Flag any parameter typed as ...Updater (plain Updater usage).
    // Accept optional module path ending in Updater, but exclude LatestOnlyUpdater.
    //
    // Examples caught:
    // - updater: Updater
    // - updater: crate::state::Updater
    // - updater: collects_states::state::Updater
    //
    // Examples NOT caught (acceptable):
    // - updater: LatestOnlyUpdater
    // - updater: crate::state::LatestOnlyUpdater
    const badParam = buf.match(
      /\bupdater\s*:\s*(?:[A-Za-z_][A-Za-z0-9_]*::)*Updater\b/,
    );

    if (badParam) {
      // find the first line within the signature containing the match for nicer reporting
      let foundLine = startLine;
      for (
        let j = startLine;
        j < Math.min(lines.length, startLine + MAX_SIG_LINES);
        j++
      ) {
        if (lines[j].includes("updater") && lines[j].includes("Updater")) {
          foundLine = j;
          break;
        }
      }

      findings.push({
        file: "", // filled by caller
        line: foundLine + 1,
        snippet: lines[foundLine].trim(),
      });
    }
  }

  return findings;
}

export async function checkCommandUpdaterUsage(
  options: CheckOptions = {},
): Promise<{ success: boolean; findings: Finding[]; scannedFiles: number }> {
  const scanDirs = (options.scanDirs ?? DEFAULT_SCAN_DIRS).map(toAbs);

  const excludePaths = new Set<string>([
    ...DEFAULT_EXCLUDE_PATHS,
    ...(options.excludePaths ?? []).map(toAbs),
  ]);

  const findings: Finding[] = [];
  let scannedFiles = 0;

  for (const dir of scanDirs) {
    if (!(await isDirectory(dir))) continue;

    for await (const file of walk(dir)) {
      if (!isRustFile(file)) continue;

      // Only scan files under the project root (defensive)
      if (!file.startsWith(PROJECT_ROOT)) continue;

      if (excludePaths.has(file)) {
        if (options.verbose) {
          console.log(
            `Skipping excluded file: ${relative(PROJECT_ROOT, file)}`,
          );
        }
        continue;
      }

      scannedFiles++;

      const text = await readFile(file, "utf-8");

      const fileFindings = findUpdaterInRunSignatures(text);
      for (const f of fileFindings) {
        findings.push({
          ...f,
          file,
        });
      }
    }
  }

  return { success: findings.length === 0, findings, scannedFiles };
}

export async function runCommandUpdaterCheckCLI(): Promise<void> {
  console.log(
    "Checking Command updater usage (LatestOnlyUpdater enforcement)...\n",
  );

  const result = await checkCommandUpdaterUsage({
    verbose: false,
  });

  console.log(`Scanned Rust files: ${result.scannedFiles}`);

  if (result.findings.length === 0) {
    console.log("\n✅ Command updater check passed");
    return;
  }

  console.error(
    `\n❌ Command updater check failed: found ${result.findings.length} potential violation(s)\n`,
  );

  for (const f of result.findings) {
    const rel = relative(PROJECT_ROOT, f.file);
    console.error(`- ${rel}:${f.line}`);
    if (f.snippet) console.error(`  ${f.snippet}`);
    console.error("");
  }

  console.error("Fix:\n- Update `fn run(...)` to use `LatestOnlyUpdater`.\n");

  process.exit(1);
}

// Allow direct execution via `bun run collects/scripts/gh-actions/command-updater-check.ts`
if (import.meta.main) {
  await runCommandUpdaterCheckCLI();
}
