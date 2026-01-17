import {
  cmdBranchName,
  cmdPrAutoMerge,
  cmdPrChecks,
  cmdPrClose,
  cmdPrComment,
  cmdPrCreate,
  cmdPrDraft,
  cmdPrOpen,
  cmdPrReady,
  cmdPrStatus,
  cmdPrUrl,
} from "./commands.ts";

export async function runCommitCLI(argv: string[]): Promise<void> {
  const [command, ...rest] = argv;
  switch (command) {
    case "branch-name":
      await cmdBranchName(rest);
      break;
    case "pr-create":
      await cmdPrCreate(rest);
      break;
    case "pr-auto-merge":
      await cmdPrAutoMerge(rest);
      break;
    case "pr-checks":
      await cmdPrChecks(rest);
      break;
    case "pr-open":
      await cmdPrOpen(rest);
      break;
    case "pr-url":
      await cmdPrUrl();
      break;
    case "pr-status":
      await cmdPrStatus(rest);
      break;
    case "pr-draft":
      await cmdPrDraft(rest);
      break;
    case "pr-ready":
      await cmdPrReady(rest);
      break;
    case "pr-comment":
      await cmdPrComment(rest);
      break;
    case "pr-close":
      await cmdPrClose(rest);
      break;
    default:
      throw new Error(
        "Usage: commit <branch-name|pr-create|pr-auto-merge|pr-checks|pr-open|pr-url|pr-status|pr-draft|pr-ready|pr-comment|pr-close> ...",
      );
  }
}
