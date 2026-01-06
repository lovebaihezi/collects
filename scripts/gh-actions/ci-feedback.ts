import { Octokit } from "@octokit/rest";
import { appendFileSync } from "fs";
import { type JobSummary, formatCommitSha } from "./shared.ts";

/**
 * CI Feedback Bot - Collects failures from CI workflow runs and posts feedback to PRs
 */

interface CIFeedbackOptions {
  token: string;
  owner: string;
  repo: string;
  runId: number;
  headSha: string;
  workflowRunUrl: string;
}

interface PostJobFeedbackOptions {
  token: string;
  owner: string;
  repo: string;
  prNumber: number;
  jobName: string;
  runId: number;
  headSha: string;
  workflowRunUrl: string;
}

interface PRInfo {
  hasPR: boolean;
  prNumber?: number;
}

/**
 * Strip ANSI escape codes from text
 * These codes are used for terminal coloring but appear as garbled characters in PR comments
 * @param text - The text containing potential ANSI escape codes
 * @returns Clean text with all ANSI escape codes removed
 */
export function stripAnsiCodes(text: string): string {
  // Matches ANSI escape sequences: ESC[ followed by any number of parameters and a final byte
  // This covers color codes, cursor movement, and other terminal control sequences
  // eslint-disable-next-line no-control-regex
  return text.replace(/\x1B\[[0-?]*[ -/]*[@-~]/g, "");
}

/**
 * Extract relevant error lines from job logs
 *
 * Prioritizes errors from the end of the log, as those are typically
 * the actual failure causes. The function:
 * 1. Scans the log from the end to find error blocks
 * 2. Includes context lines around each error
 * 3. Returns the most relevant errors (those closest to the end of the log)
 */
export function extractErrorLines(logs: string): string {
  // Strip ANSI escape codes first to get clean log lines
  const cleanLogs = stripAnsiCodes(logs);
  const logLines = cleanLogs.split("\n");

  // Patterns for error detection - includes common build/test failure indicators
  // More specific patterns are checked first (higher priority)
  const errorPatterns = [
    // Rust-specific errors (high priority)
    /error\[E\d+\]/i, // Rust compiler errors like error[E0433]
    /panicked at/i, // Rust panic messages
    /thread .+ panicked/i, // Thread panic
    // General errors (medium priority)
    /^error:/i, // Lines starting with "error:"
    /^error\s/i, // Lines starting with "error "
    /:\s*error:/i, // "file.rs: error:" style
    /FAILED/i, // Test failures
    /FAILURE/i, // General failures
    // Lower priority patterns
    /error/i,
    /failed/i,
    /exception/i,
    /panic/i,
  ];

  // Find all error line indices, prioritizing from the end
  const errorIndices: number[] = [];
  for (let i = logLines.length - 1; i >= 0; i--) {
    const line = logLines[i];
    if (errorPatterns.some((pattern) => pattern.test(line))) {
      errorIndices.push(i);
    }
  }

  // If no errors found, return last 30 lines
  if (errorIndices.length === 0) {
    return logLines.slice(-30).join("\n");
  }

  // Build error blocks with context, prioritizing errors from the end
  // Use a Set to track included line indices to preserve order and avoid duplicates
  const includedIndices = new Set<number>();
  const targetLineCount = 50;

  // Process errors from end to beginning (errorIndices is already reverse order)
  for (const errorIdx of errorIndices) {
    if (includedIndices.size >= targetLineCount) break;

    // Add context: 3 lines before and 3 lines after for better context
    const start = Math.max(0, errorIdx - 3);
    const end = Math.min(logLines.length, errorIdx + 4);

    for (let j = start; j < end; j++) {
      includedIndices.add(j);
    }
  }

  // Convert to sorted array and extract lines (preserves original order)
  const sortedIndices = Array.from(includedIndices).sort((a, b) => a - b);
  const resultLines = sortedIndices.map((i) => logLines[i]);

  // Limit to 50 lines to avoid huge comments
  return resultLines.slice(0, 50).join("\n");
}

/**
 * Convert various response data types to string
 * GitHub API can return string, ArrayBuffer, or Buffer depending on context
 */
function responseDataToString(data: unknown): string {
  if (typeof data === "string") {
    return data;
  } else if (data instanceof ArrayBuffer) {
    return new TextDecoder().decode(data);
  } else if (Buffer.isBuffer(data)) {
    return data.toString("utf8");
  } else {
    return String(data);
  }
}

/**
 * Count previous failures per job from existing comments
 */
export function countPreviousFailures(
  comments: Array<{ body?: string | null; user?: { type?: string } | null }>,
): Record<string, number> {
  const jobFailureCounts: Record<string, number> = {};

  // Find CI feedback comments from this workflow
  // Note: We only check for the marker, not user.type, because comments posted via
  // user PAT (like COPILOT_INVOKER_TOKEN) have type "User", not "Bot"
  const feedbackComments = comments.filter((comment) =>
    comment.body?.includes("<!-- CI-FEEDBACK-BOT -->"),
  );

  for (const comment of feedbackComments) {
    if (!comment.body) continue;

    // Parse job names from previous comments
    const jobMatches = comment.body.match(/### ❌ Job: `([^`]+)`/g);
    if (jobMatches) {
      for (const match of jobMatches) {
        const innerMatch = match.match(/### ❌ Job: `([^`]+)`/);
        if (innerMatch && innerMatch[1]) {
          const jobName = innerMatch[1];
          jobFailureCounts[jobName] = (jobFailureCounts[jobName] || 0) + 1;
        }
      }
    }
  }

  return jobFailureCounts;
}

/**
 * Build the comment body for CI failure report
 */
export function buildCommentBody(
  runId: number,
  workflowRunUrl: string,
  headSha: string,
  jobsToReport: JobSummary[],
  skippedJobs: JobSummary[],
  jobFailureCounts: Record<string, number>,
): string {
  let commentBody = `<!-- CI-FEEDBACK-BOT -->\n## 🚨 CI Failure Report\n\n`;
  commentBody += `**Workflow Run:** [#${runId}](${workflowRunUrl})\n`;
  commentBody += `**Commit:** \`${formatCommitSha(headSha)}\`\n\n`;
  commentBody += `The following CI jobs have failed:\n\n`;

  for (const summary of jobsToReport) {
    const failureCount = (jobFailureCounts[summary.name] || 0) + 1;
    commentBody += `### ❌ Job: \`${summary.name}\`\n\n`;
    commentBody += `**Attempt ${failureCount} of 3** | [View Full Logs](${summary.url})\n\n`;
    commentBody += `<details>\n<summary>Error Summary</summary>\n\n`;
    commentBody += `\`\`\`\n${summary.logs}\n\`\`\`\n\n`;
    commentBody += `</details>\n\n`;
  }

  if (skippedJobs.length > 0) {
    commentBody += `---\n\n`;
    commentBody += `⚠️ **Note:** The following jobs have failed 3+ times and will no longer trigger auto-feedback:\n`;
    commentBody +=
      skippedJobs.map((j) => `- \`${j.name}\``).join("\n") + "\n\n";
  }

  commentBody += `---\n\n`;
  commentBody += `@copilot Please analyze these CI failures and suggest fixes based on the error logs above.\n`;

  return commentBody;
}

/**
 * Get PR number associated with a workflow run
 *
 * This function uses multiple strategies to find the PR:
 * 1. Use the pullRequests array from the workflow run (fastest, but empty for fork PRs)
 * 2. Search for open PRs by head commit SHA (works for both fork and non-fork PRs)
 * 3. Search for PRs by head branch (fallback, doesn't work for forks)
 */
async function getPRInfo(
  octokit: Octokit,
  owner: string,
  repo: string,
  headBranch: string,
  headSha: string,
  pullRequests?: Array<{ number: number }>,
): Promise<PRInfo> {
  // Strategy 1: Use pullRequests from workflow run (fastest)
  if (pullRequests && pullRequests.length > 0) {
    return { hasPR: true, prNumber: pullRequests[0].number };
  }

  // Strategy 2: Search for PRs by head commit SHA (works for fork PRs)
  // This is more reliable than branch search because SHA is unique and works across forks
  try {
    const { data: searchResults } =
      await octokit.rest.search.issuesAndPullRequests({
        q: `repo:${owner}/${repo} is:pr is:open ${headSha}`,
      });

    if (searchResults.total_count > 0 && searchResults.items.length > 0) {
      return { hasPR: true, prNumber: searchResults.items[0].number };
    }
  } catch (error) {
    // Log search error but continue to fallback
    const message = error instanceof Error ? error.message : String(error);
    console.log(
      `SHA-based PR search failed: ${message}, falling back to branch search`,
    );
  }

  // Strategy 3: Try to find PR by head branch (fallback, doesn't work for forks)
  const { data: pulls } = await octokit.rest.pulls.list({
    owner,
    repo,
    head: `${owner}:${headBranch}`,
    state: "open",
  });

  if (pulls.length === 0) {
    return { hasPR: false };
  }

  return { hasPR: true, prNumber: pulls[0].number };
}

/**
 * Collect job summaries with logs for failed jobs
 */
async function collectJobSummaries(
  octokit: Octokit,
  owner: string,
  repo: string,
  runId: number,
): Promise<JobSummary[]> {
  // Get all jobs for the workflow run
  const { data: jobsData } = await octokit.rest.actions.listJobsForWorkflowRun({
    owner,
    repo,
    run_id: runId,
  });

  const failedJobs = jobsData.jobs.filter(
    (job) => job.conclusion === "failure",
  );

  if (failedJobs.length === 0) {
    console.log("No failed jobs found");
    return [];
  }

  const summaries: JobSummary[] = [];

  for (const job of failedJobs) {
    try {
      // Get job logs
      const response = await octokit.rest.actions.downloadJobLogsForWorkflowRun(
        {
          owner,
          repo,
          job_id: job.id,
        },
      );

      const logs = responseDataToString(response.data);

      summaries.push({
        name: job.name,
        url: job.html_url || "",
        logs: extractErrorLines(logs),
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.log(`Failed to get logs for job ${job.name}: ${message}`);
      summaries.push({
        name: job.name,
        url: job.html_url || "",
        logs: "Unable to retrieve logs",
      });
    }
  }

  return summaries;
}

/**
 * Main function to run CI feedback
 */
export async function runCIFeedback(options: CIFeedbackOptions): Promise<void> {
  const { token, owner, repo, runId, headSha, workflowRunUrl } = options;

  const octokit = new Octokit({ auth: token });

  // Get workflow run details to find PR
  const { data: workflowRun } = await octokit.rest.actions.getWorkflowRun({
    owner,
    repo,
    run_id: runId,
  });

  const prInfo = await getPRInfo(
    octokit,
    owner,
    repo,
    workflowRun.head_branch || "",
    headSha,
    workflowRun.pull_requests,
  );

  if (!prInfo.hasPR || !prInfo.prNumber) {
    console.log("No PR found for this workflow run");
    setOutput("has_pr", "false");
    return;
  }

  setOutput("has_pr", "true");
  setOutput("pr_number", String(prInfo.prNumber));

  // Collect job summaries
  const summaries = await collectJobSummaries(octokit, owner, repo, runId);

  if (summaries.length === 0) {
    console.log("No failed jobs to report");
    return;
  }

  // Check existing comments to count failures
  const { data: comments } = await octokit.rest.issues.listComments({
    owner,
    repo,
    issue_number: prInfo.prNumber,
  });

  const jobFailureCounts = countPreviousFailures(comments);

  // Filter jobs that have failed less than 3 times
  const jobsToReport = summaries.filter((summary) => {
    const count = jobFailureCounts[summary.name] || 0;
    if (count >= 3) {
      console.log(
        `Skipping job "${summary.name}" - already failed ${count} times`,
      );
      return false;
    }
    return true;
  });

  if (jobsToReport.length === 0) {
    console.log(
      "All failed jobs have exceeded the 3 failure limit. Not posting comment.",
    );
    return;
  }

  // Check if any jobs are being skipped due to failure limit
  const skippedJobs = summaries.filter((summary) => {
    const count = jobFailureCounts[summary.name] || 0;
    return count >= 3;
  });

  // Build and post comment
  const commentBody = buildCommentBody(
    runId,
    workflowRunUrl,
    headSha,
    jobsToReport,
    skippedJobs,
    jobFailureCounts,
  );

  await octokit.rest.issues.createComment({
    owner,
    repo,
    issue_number: prInfo.prNumber,
    body: commentBody,
  });

  console.log(`Posted CI feedback comment on PR #${prInfo.prNumber}`);
}

/**
 * Error result type for CI feedback operations
 */
export interface CIFeedbackResult {
  success: boolean;
  message: string;
  /** If true, this is an expected/recoverable error that shouldn't fail the workflow */
  recoverable?: boolean;
}

/**
 * Check if an error is a GitHub API error with a specific status
 */
function isGitHubApiError(
  error: unknown,
  status?: number,
): error is { status: number; message: string } {
  if (
    typeof error === "object" &&
    error !== null &&
    "status" in error &&
    typeof (error as { status: unknown }).status === "number"
  ) {
    if (status === undefined) return true;
    return (error as { status: number }).status === status;
  }
  return false;
}

/**
 * Format a helpful error message for common GitHub API errors
 */
export function formatApiErrorMessage(
  error: unknown,
  operation: string,
): string {
  // Extract the message from the error
  let baseMessage: string;
  if (error instanceof Error) {
    baseMessage = error.message;
  } else if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    baseMessage = (error as { message: string }).message;
  } else {
    baseMessage = String(error);
  }

  if (isGitHubApiError(error, 403)) {
    return (
      `${operation}: Permission denied (403). The token may not have sufficient permissions. ` +
      `Ensure the COPILOT_INVOKER_TOKEN secret has 'Pull requests: Read and write' and 'Actions: Read' permissions.`
    );
  }

  if (isGitHubApiError(error, 404)) {
    return (
      `${operation}: Resource not found (404). This may be a timing issue - ` +
      `job logs might not be available yet while the job is still completing.`
    );
  }

  if (isGitHubApiError(error, 401)) {
    return `${operation}: Authentication failed (401). The token may be invalid or expired.`;
  }

  if (isGitHubApiError(error)) {
    return `${operation}: GitHub API error (${(error as { status: number }).status}): ${baseMessage}`;
  }

  return `${operation}: ${baseMessage}`;
}

/**
 * Post-job feedback function - runs within the CI workflow itself
 * This approach has direct access to PR context, avoiding PR detection issues
 *
 * Returns a result object instead of throwing, allowing callers to handle errors gracefully
 */
export async function runPostJobFeedback(
  options: PostJobFeedbackOptions,
): Promise<CIFeedbackResult> {
  const {
    token,
    owner,
    repo,
    prNumber,
    jobName,
    runId,
    headSha,
    workflowRunUrl,
  } = options;

  const octokit = new Octokit({ auth: token });

  console.log(`Processing feedback for job "${jobName}" on PR #${prNumber}`);

  // Get job logs for the specific failed job
  let jobsData;
  try {
    const response = await octokit.rest.actions.listJobsForWorkflowRun({
      owner,
      repo,
      run_id: runId,
    });
    jobsData = response.data;
  } catch (error) {
    const message = formatApiErrorMessage(
      error,
      "Failed to list workflow jobs",
    );
    return { success: false, message, recoverable: true };
  }

  // Find the specific job by name
  const job = jobsData.jobs.find((j) => j.name === jobName);
  if (!job) {
    const availableJobs = jobsData.jobs.map((j) => j.name).join(", ");
    return {
      success: false,
      message: `Job "${jobName}" not found in workflow run. Available jobs: ${availableJobs}`,
      recoverable: true,
    };
  }

  // Get job logs
  let logs = "Unable to retrieve logs";
  try {
    const response = await octokit.rest.actions.downloadJobLogsForWorkflowRun({
      owner,
      repo,
      job_id: job.id,
    });

    logs = extractErrorLines(responseDataToString(response.data));
  } catch (error) {
    const message = formatApiErrorMessage(
      error,
      `Failed to get logs for job ${jobName}`,
    );
    console.log(message);
    // Continue with "Unable to retrieve logs" - this is expected sometimes
  }

  const summary: JobSummary = {
    name: jobName,
    url: job.html_url || "",
    logs,
  };

  // Check existing comments to count failures
  let comments;
  try {
    const response = await octokit.rest.issues.listComments({
      owner,
      repo,
      issue_number: prNumber,
    });
    comments = response.data;
  } catch (error) {
    const message = formatApiErrorMessage(error, "Failed to list PR comments");
    return { success: false, message, recoverable: true };
  }

  const jobFailureCounts = countPreviousFailures(comments);
  const failureCount = jobFailureCounts[jobName] || 0;

  // Skip if this job has already failed 3+ times
  if (failureCount >= 3) {
    return {
      success: true,
      message: `Job "${jobName}" has already failed ${failureCount} times. Skipping feedback.`,
    };
  }

  // Build and post comment for this single job
  const commentBody = buildCommentBody(
    runId,
    workflowRunUrl,
    headSha,
    [summary],
    [],
    jobFailureCounts,
  );

  try {
    await octokit.rest.issues.createComment({
      owner,
      repo,
      issue_number: prNumber,
      body: commentBody,
    });
  } catch (error) {
    const message = formatApiErrorMessage(
      error,
      "Failed to post CI feedback comment",
    );
    return { success: false, message, recoverable: true };
  }

  return {
    success: true,
    message: `Posted CI feedback comment for job "${jobName}" on PR #${prNumber}`,
  };
}

/**
 * Set output for GitHub Actions
 */
function setOutput(name: string, value: string): void {
  console.log(`${name}=${value}`);
  if (process.env.GITHUB_OUTPUT) {
    appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`);
  }
}

/**
 * CLI entry point
 */
export function runCIFeedbackCLI(): void {
  const token = process.env.GITHUB_TOKEN;
  const owner = process.env.GITHUB_REPOSITORY_OWNER;
  const githubRepository = process.env.GITHUB_REPOSITORY;
  const runIdStr = process.env.WORKFLOW_RUN_ID;
  const headSha = process.env.HEAD_SHA;
  const workflowRunUrl = process.env.WORKFLOW_RUN_URL;

  if (!token) {
    console.error("GITHUB_TOKEN is required");
    process.exit(1);
  }
  if (!owner) {
    console.error("GITHUB_REPOSITORY_OWNER is required");
    process.exit(1);
  }
  if (!githubRepository || !githubRepository.includes("/")) {
    console.error(
      "GITHUB_REPOSITORY is required and must be in format 'owner/repo'",
    );
    process.exit(1);
  }
  const repo = githubRepository.split("/")[1];
  if (!repo) {
    console.error("GITHUB_REPOSITORY must contain a repository name");
    process.exit(1);
  }
  if (!runIdStr) {
    console.error("WORKFLOW_RUN_ID is required");
    process.exit(1);
  }
  const runId = parseInt(runIdStr, 10);
  if (isNaN(runId)) {
    console.error("WORKFLOW_RUN_ID must be a valid number");
    process.exit(1);
  }
  if (!headSha) {
    console.error("HEAD_SHA is required");
    process.exit(1);
  }
  if (!workflowRunUrl) {
    console.error("WORKFLOW_RUN_URL is required");
    process.exit(1);
  }

  runCIFeedback({
    token,
    owner,
    repo,
    runId,
    headSha,
    workflowRunUrl,
  }).catch((error) => {
    console.error("CI Feedback failed:", error);
    process.exit(1);
  });
}

/**
 * CLI entry point for post-job feedback (runs within CI workflow)
 */
export function runPostJobFeedbackCLI(): void {
  const token = process.env.GITHUB_TOKEN;
  const owner = process.env.GITHUB_REPOSITORY_OWNER;
  const githubRepository = process.env.GITHUB_REPOSITORY;
  const prNumberStr = process.env.PR_NUMBER;
  const jobName = process.env.JOB_NAME;
  const runIdStr = process.env.RUN_ID;
  const headSha = process.env.HEAD_SHA;
  const workflowRunUrl = process.env.WORKFLOW_RUN_URL;

  if (!token) {
    console.error("GITHUB_TOKEN is required");
    process.exit(1);
  }
  if (!owner) {
    console.error("GITHUB_REPOSITORY_OWNER is required");
    process.exit(1);
  }
  if (!githubRepository || !githubRepository.includes("/")) {
    console.error(
      "GITHUB_REPOSITORY is required and must be in format 'owner/repo'",
    );
    process.exit(1);
  }
  const repo = githubRepository.split("/")[1];
  if (!repo) {
    console.error("GITHUB_REPOSITORY must contain a repository name");
    process.exit(1);
  }
  if (!prNumberStr) {
    console.log("PR_NUMBER not set - not a pull request event, skipping");
    process.exit(0);
  }
  const prNumber = parseInt(prNumberStr, 10);
  if (isNaN(prNumber)) {
    console.error("PR_NUMBER must be a valid number");
    process.exit(1);
  }
  if (!jobName) {
    console.error("JOB_NAME is required");
    process.exit(1);
  }
  if (!runIdStr) {
    console.error("RUN_ID is required");
    process.exit(1);
  }
  const runId = parseInt(runIdStr, 10);
  if (isNaN(runId)) {
    console.error("RUN_ID must be a valid number");
    process.exit(1);
  }
  if (!headSha) {
    console.error("HEAD_SHA is required");
    process.exit(1);
  }
  if (!workflowRunUrl) {
    console.error("WORKFLOW_RUN_URL is required");
    process.exit(1);
  }

  runPostJobFeedback({
    token,
    owner,
    repo,
    prNumber,
    jobName,
    runId,
    headSha,
    workflowRunUrl,
  })
    .then((result) => {
      if (result.success) {
        console.log(result.message);
      } else if (result.recoverable) {
        // Recoverable errors should not fail the workflow step
        console.warn(`⚠️ CI Feedback warning: ${result.message}`);
        console.log(
          "This is a recoverable error - the workflow step will not fail.",
        );
      } else {
        console.error(`❌ CI Feedback error: ${result.message}`);
        process.exit(1);
      }
    })
    .catch((error) => {
      // Unexpected errors - log but don't fail the workflow
      const message = error instanceof Error ? error.message : String(error);
      console.error(`⚠️ Unexpected CI Feedback error: ${message}`);
      console.log(
        "CI Feedback encountered an unexpected error but will not fail the workflow step.",
      );
    });
}
