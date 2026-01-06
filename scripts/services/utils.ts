import * as p from "@clack/prompts";
import { $ } from "bun";
import { type } from "arktype";

/**
 * Runs a shell command with error handling and LLM prompt generation.
 */
export async function runCommand(command: string, context: string) {
  const s = p.spinner();
  try {
    // We use Bun.spawn to have better control or just use $ if simple
    // Using $ from bun as imported. We capture stdout to keep the UI clean.
    s.start(`Run Google Cloud CLI: ${command}`);
    const { stdout } = await $`${{ raw: command }}`.quiet();
    s.stop("GCLI succeeded");
    return stdout.toString();
  } catch (err: unknown) {
    s.stop(`Failed to run command: ${command}`);
    p.log.error(`COMMAND FAILED: ${command}`);

    let errorOutput = "";

    // ShellError is not exported from 'bun' in the current version, so we check the name/properties
    if (err instanceof $.ShellError) {
      errorOutput = err.stdout.toString() + err.stderr.toString();
    } else if (err instanceof Error) {
      errorOutput = err.message || String(err);
    }

    p.log.error(`ERROR: ${errorOutput.trim()}`);

    const llmPrompt = `I ran the command \`${command}\` to ${context} and got this error:

${errorOutput.trim()}

How do I fix this in Google Cloud?`;

    p.log.info("To get help from an AI assistant, use the following prompt:");
    p.log.message(llmPrompt);

    process.exit(1);
  }
}

/**
 * Asks for confirmation before running a command.
 */
export async function confirmAndRun(command: string, context: string) {
  p.log.info(`Next step: ${context}`);
  p.log.message(`Command: ${command}`);

  const shouldRun = await p.confirm({
    message: "Do you want to run this command?",
  });

  if (p.isCancel(shouldRun) || !shouldRun) {
    p.log.warn("Operation cancelled by user.");
    process.exit(0);
  }

  await runCommand(command, context);
  p.log.success("Command executed successfully.");
}

/**
 * Checks if a resource exists using gcloud describe/list.
 * Returns true if exists, false otherwise.
 * Mutes output to keep the flow clean.
 */
export async function checkResource(command: string): Promise<boolean> {
  try {
    // Run quietly, we only care about exit code
    await $`${{ raw: command }} --quiet`.quiet();
    return true;
  } catch {
    return false;
  }
}

/**
 * Validates and parses repository format.
 * Only allows alphanumeric characters, hyphens, underscores, and periods
 * in owner and repo names to prevent shell injection.
 */
export function validateRepo(repo: string): { owner: string; repo: string } {
  // Strict pattern: owner/repo where each part contains only safe characters
  // GitHub repo naming rules: alphanumeric, hyphen, underscore, period
  const repoType = type(/^[a-zA-Z0-9_.-]+\/[a-zA-Z0-9_.-]+$/);
  const result = repoType(repo);

  if (result instanceof type.errors) {
    p.log.error(`Invalid repository format: ${result.summary}`);
    p.log.error(
      "Repository must be in format 'owner/repo' with only alphanumeric characters, hyphens, underscores, and periods.",
    );
    process.exit(1);
  }

  const [owner, repoName] = result.split("/");
  return { owner, repo: repoName };
}

/**
 * Gets project number from project ID
 */
export async function getProjectNumber(projectId: string): Promise<string> {
  const projectNumber =
    await $`gcloud projects describe ${projectId} --format="value(projectNumber)"`.text();
  return projectNumber.trim();
}
