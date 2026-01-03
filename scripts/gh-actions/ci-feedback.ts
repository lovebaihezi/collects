import { Octokit } from "@octokit/rest";
import { appendFileSync } from "fs";

/**
 * CI Feedback Bot - Collects failures from CI workflow runs and posts feedback to PRs
 */

interface JobSummary {
  name: string;
  url: string;
  logs: string;
}

interface CIFeedbackOptions {
  token: string;
  owner: string;
  repo: string;
  runId: number;
  headSha: string;
  workflowRunUrl: string;
}

interface PRInfo {
  hasPR: boolean;
  prNumber?: number;
}

/**
 * Extract relevant error lines from job logs
 */
function extractErrorLines(logs: string): string {
  const logLines = logs.split("\n");
  const seenLines = new Set<string>();
  const errorLines: string[] = [];
  const relevantPatterns = [
    /error/i,
    /failed/i,
    /failure/i,
    /exception/i,
    /panic/i,
  ];

  // Get lines around errors
  for (let i = 0; i < logLines.length; i++) {
    const line = logLines[i];
    if (relevantPatterns.some((pattern) => pattern.test(line))) {
      // Add context: 2 lines before and 2 lines after
      const start = Math.max(0, i - 2);
      const end = Math.min(logLines.length, i + 3);
      for (let j = start; j < end; j++) {
        if (!seenLines.has(logLines[j])) {
          seenLines.add(logLines[j]);
          errorLines.push(logLines[j]);
        }
      }
    }
  }

  // If no specific errors found, get last 30 lines
  if (errorLines.length === 0) {
    return logLines.slice(-30).join("\n");
  }

  // Limit to 50 lines to avoid huge comments
  return errorLines.slice(0, 50).join("\n");
}

/**
 * Count previous failures per job from existing comments
 */
function countPreviousFailures(
  comments: Array<{ body?: string | null; user?: { type?: string } | null }>,
): Record<string, number> {
  const jobFailureCounts: Record<string, number> = {};

  // Find CI feedback comments from this workflow
  const feedbackComments = comments.filter(
    (comment) =>
      comment.body &&
      comment.body.includes("<!-- CI-FEEDBACK-BOT -->") &&
      comment.user &&
      comment.user.type === "Bot",
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
function buildCommentBody(
  runId: number,
  workflowRunUrl: string,
  headSha: string,
  jobsToReport: JobSummary[],
  skippedJobs: JobSummary[],
  jobFailureCounts: Record<string, number>,
): string {
  let commentBody = `<!-- CI-FEEDBACK-BOT -->\n## 🚨 CI Failure Report\n\n`;
  commentBody += `**Workflow Run:** [#${runId}](${workflowRunUrl})\n`;
  commentBody += `**Commit:** \`${headSha.substring(0, 7)}\`\n\n`;
  commentBody += `The following CI jobs have failed:\n\n`;

  for (const summary of jobsToReport) {
    const failureCount = (jobFailureCounts[summary.name] || 0) + 1;
    commentBody += `### ❌ Job: \`${summary.name}\`\n\n`;
    commentBody += `**Failure #${failureCount}/3** | [View Full Logs](${summary.url})\n\n`;
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
 */
async function getPRInfo(
  octokit: Octokit,
  owner: string,
  repo: string,
  headBranch: string,
  pullRequests?: Array<{ number: number }>,
): Promise<PRInfo> {
  if (pullRequests && pullRequests.length > 0) {
    return { hasPR: true, prNumber: pullRequests[0].number };
  }

  // Try to find PR by head branch
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

      // Convert to string - response.data can be string or ArrayBuffer
      let logs: string;
      if (typeof response.data === "string") {
        logs = response.data;
      } else if (response.data instanceof ArrayBuffer) {
        logs = new TextDecoder().decode(response.data);
      } else if (Buffer.isBuffer(response.data)) {
        logs = response.data.toString("utf8");
      } else {
        logs = String(response.data);
      }

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
