import { $ } from "bun";
import { detectSummary } from "./git.ts";
import { toSlug } from "./slug.ts";
import { buildPrBody, writePrBodyFile } from "./pr.ts";

export async function cmdBranchName(args: string[]): Promise<void> {
  const input = args.join(" ").trim();
  const summary = input.length > 0 ? input : await detectSummary();
  const slug = toSlug(summary);
  console.log(`feat-${slug}`);
}

export async function cmdPrCreate(args: string[]): Promise<void> {
  const title = args[0];
  const bodyPath = args[1];
  if (!title) {
    throw new Error("pr-create requires <title> [body-path]");
  }
  if (bodyPath) {
    await $`gh pr create --title ${title} --body-file ${bodyPath}`;
    return;
  }

  const body = await buildPrBody(title);
  const tempPath = await writePrBodyFile(body);
  await $`gh pr create --title ${title} --body-file ${tempPath}`;
}

export async function cmdPrAutoMerge(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (!prUrl) {
    throw new Error("pr-auto-merge requires <pr-url>");
  }
  await $`gh pr merge --auto --squash ${prUrl}`;
}

export async function cmdPrChecks(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (!prUrl) {
    throw new Error("pr-checks requires <pr-url>");
  }
  await $`gh pr checks --watch ${prUrl}`;
}

export async function cmdPrOpen(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (prUrl) {
    await $`gh pr view --web ${prUrl}`;
    return;
  }
  await $`gh pr view --web`;
}

export async function cmdPrUrl(): Promise<void> {
  await $`gh pr view --json url -q .url`;
}

export async function cmdPrStatus(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (prUrl) {
    await $`gh pr view ${prUrl} --json statusCheckRollup,reviewDecision`;
    return;
  }
  await $`gh pr view --json statusCheckRollup,reviewDecision`;
}

export async function cmdPrDraft(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (prUrl) {
    await $`gh pr ready --undo ${prUrl}`;
    return;
  }
  await $`gh pr ready --undo`;
}

export async function cmdPrReady(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (prUrl) {
    await $`gh pr ready ${prUrl}`;
    return;
  }
  await $`gh pr ready`;
}

export async function cmdPrComment(args: string[]): Promise<void> {
  const prUrl = args[0];
  const body = args.slice(1).join(" ").trim();
  if (!prUrl || !body) {
    throw new Error("pr-comment requires <pr-url> <body>");
  }
  await $`gh pr comment ${prUrl} --body ${body}`;
}

export async function cmdPrClose(args: string[]): Promise<void> {
  const prUrl = args[0];
  if (!prUrl) {
    throw new Error("pr-close requires <pr-url>");
  }
  await $`gh pr close ${prUrl}`;
}
