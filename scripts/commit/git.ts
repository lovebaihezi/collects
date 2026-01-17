import { $ } from "bun";

export async function detectSummary(): Promise<string> {
  const diff = await $`git diff --name-only --diff-filter=ACMRT`.quiet();
  const firstDiff = diff.stdout.toString().trim().split("\n")[0];
  if (firstDiff) {
    return firstDiff.replace(/\.[^/.]+$/, "").replace(/\//g, " ");
  }

  const status = await $`git status --porcelain`.quiet();
  const firstStatus = status.stdout
    .toString()
    .trim()
    .split("\n")[0]
    ?.trim()
    .split(/\s+/)[1];
  if (firstStatus) {
    return firstStatus.replace(/\.[^/.]+$/, "").replace(/\//g, " ");
  }

  return "changes";
}
