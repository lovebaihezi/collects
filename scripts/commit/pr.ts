import { $ } from "bun";

function uniq(items: string[]): string[] {
  const seen = new Set<string>();
  return items.filter((item) => {
    if (seen.has(item)) {
      return false;
    }
    seen.add(item);
    return true;
  });
}

async function listChangedPaths(): Promise<string[]> {
  const lastCommit = await $`git rev-parse --verify HEAD`.quiet();
  if (lastCommit.exitCode === 0) {
    const diff =
      await $`git diff --name-only --diff-filter=ACMRT HEAD~1`.quiet();
    const files = diff.stdout.toString().trim().split("\n").filter(Boolean);
    if (files.length > 0) {
      return files;
    }
  }

  const working = await $`git diff --name-only --diff-filter=ACMRT`.quiet();
  return working.stdout.toString().trim().split("\n").filter(Boolean);
}

function summarizePaths(paths: string[]): string[] {
  const areas = uniq(
    paths.map((path) => {
      const parts = path.split("/");
      return parts.length > 1 ? parts[0] : path;
    }),
  );
  return areas.slice(0, 6);
}

export async function buildPrBody(title: string): Promise<string> {
  const paths = await listChangedPaths();
  const areas = summarizePaths(paths);

  const summaryLines: string[] = [`- ${title}`];
  if (areas.length > 0) {
    summaryLines.push(`- touch ${areas.join(", ")}`);
  }

  return [
    "## Summary",
    ...summaryLines,
    "",
    "## Testing",
    "- not run (not requested)",
    "",
  ].join("\n");
}

export async function writePrBodyFile(body: string): Promise<string> {
  const filename = `commit-pr-body-${Date.now()}.md`;
  const path = `/tmp/${filename}`;
  await Bun.write(path, body);
  return path;
}
