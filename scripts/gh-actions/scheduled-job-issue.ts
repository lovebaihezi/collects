import { Octokit } from "@octokit/rest";
import { extractErrorLines } from "./ci-feedback.ts";
import { type JobSummary, formatCommitSha } from "./shared.ts";

/**
 * Scheduled Job Issue Creator - Creates issues when scheduled jobs fail
 *
 * This tool monitors scheduled workflow runs and creates issues when they fail,
 * providing diagnosis plans and possible root causes.
 */

interface ScheduledJobIssueOptions {
  token: string;
  owner: string;
  repo: string;
  runId: number;
  workflowName: string;
  workflowRunUrl: string;
  headSha: string;
}

interface DiagnosisPlan {
  category: string;
  possibleCauses: string[];
  diagnosisSteps: string[];
}

/**
 * Marker comment to identify issues created by this bot
 */
const ISSUE_MARKER = "<!-- SCHEDULED-JOB-FAILURE-BOT -->";

/**
 * Analyze error logs and generate diagnosis plans based on error patterns
 */
export function generateDiagnosisPlans(logs: string): DiagnosisPlan[] {
  const plans: DiagnosisPlan[] = [];
  const lowerLogs = logs.toLowerCase();

  // Authentication/Permission errors
  if (
    lowerLogs.includes("permission denied") ||
    lowerLogs.includes("unauthorized") ||
    lowerLogs.includes("403") ||
    lowerLogs.includes("authentication failed") ||
    lowerLogs.includes("workload identity")
  ) {
    plans.push({
      category: "🔐 Authentication/Permission Issue",
      possibleCauses: [
        "Expired or invalid credentials/tokens",
        "Workload Identity Federation misconfiguration",
        "Service account permissions changed or revoked",
        "IAM policy updates that affected the service account",
      ],
      diagnosisSteps: [
        "Check if the service account still exists and has the required permissions",
        "Verify Workload Identity Pool and Provider configuration",
        "Review recent IAM policy changes in Google Cloud Console",
        "Check if secrets (tokens) need rotation",
      ],
    });
  }

  // Network/API errors
  if (
    lowerLogs.includes("timeout") ||
    lowerLogs.includes("connection refused") ||
    lowerLogs.includes("network") ||
    lowerLogs.includes("dns") ||
    lowerLogs.includes("unreachable") ||
    lowerLogs.includes("502") ||
    lowerLogs.includes("503") ||
    lowerLogs.includes("504")
  ) {
    plans.push({
      category: "🌐 Network/Connectivity Issue",
      possibleCauses: [
        "External service temporary outage",
        "DNS resolution failure",
        "Network timeout due to rate limiting",
        "GitHub Actions runner connectivity issues",
      ],
      diagnosisSteps: [
        "Check external service status pages (Google Cloud, GitHub, etc.)",
        "Re-run the workflow to check if it's a transient issue",
        "Check for any ongoing incidents at status.github.com",
        "Review if any rate limits were exceeded",
      ],
    });
  }

  // Resource not found errors
  if (
    lowerLogs.includes("not found") ||
    lowerLogs.includes("404") ||
    lowerLogs.includes("does not exist") ||
    lowerLogs.includes("no such")
  ) {
    plans.push({
      category: "🔍 Resource Not Found",
      possibleCauses: [
        "Referenced resource was deleted or moved",
        "Incorrect resource path or identifier",
        "Resource exists in a different project/region",
        "Cleanup job already removed the resource",
      ],
      diagnosisSteps: [
        "Verify the resource exists in the expected location",
        "Check if the resource was recently deleted or moved",
        "Verify environment variables and configuration",
        "Check if a previous cleanup job affected this resource",
      ],
    });
  }

  // Docker/Container errors
  // Note: These string checks are for error log classification only, not URL validation.
  // CodeQL may flag "gcr.io" check as incomplete URL sanitization, but this is a false
  // positive - we're categorizing error messages, not validating or sanitizing URLs.
  if (
    lowerLogs.includes("docker") ||
    lowerLogs.includes("container") ||
    lowerLogs.includes("image") ||
    lowerLogs.includes("artifact registry") ||
    lowerLogs.includes("gcr.io") ||
    lowerLogs.includes("pkg.dev")
  ) {
    plans.push({
      category: "🐳 Docker/Container Issue",
      possibleCauses: [
        "Docker image not found or deleted",
        "Artifact Registry permissions changed",
        "Image tag mismatch or incorrect reference",
        "Quota limits exceeded in container registry",
      ],
      diagnosisSteps: [
        "List images in Artifact Registry to verify existence",
        "Check Artifact Registry permissions for the service account",
        "Verify image tags and digests are correct",
        "Review Artifact Registry quotas and usage",
      ],
    });
  }

  // Script/Code execution errors
  if (
    lowerLogs.includes("error:") ||
    lowerLogs.includes("exception") ||
    lowerLogs.includes("failed") ||
    lowerLogs.includes("exit code") ||
    lowerLogs.includes("non-zero")
  ) {
    plans.push({
      category: "⚠️ Script/Code Execution Error",
      possibleCauses: [
        "Bug in the workflow script",
        "Dependency version incompatibility",
        "Environment variable not set or incorrect",
        "External API response format changed",
      ],
      diagnosisSteps: [
        "Review the specific error message and stack trace",
        "Check if any dependencies were recently updated",
        "Verify all required environment variables are set",
        "Test the script locally if possible",
      ],
    });
  }

  // GCloud specific errors
  if (
    lowerLogs.includes("gcloud") ||
    lowerLogs.includes("google cloud") ||
    lowerLogs.includes("gcp") ||
    lowerLogs.includes("cloud run") ||
    lowerLogs.includes("braided-case")
  ) {
    plans.push({
      category: "☁️ Google Cloud Issue",
      possibleCauses: [
        "gcloud CLI version mismatch",
        "Project quota exceeded",
        "Service API not enabled",
        "Region/zone availability issue",
      ],
      diagnosisSteps: [
        "Check Google Cloud Console for any alerts or incidents",
        "Verify project quotas are not exceeded",
        "Ensure all required APIs are enabled",
        "Check if the issue is region-specific",
      ],
    });
  }

  // If no specific patterns matched, provide generic diagnosis
  if (plans.length === 0) {
    plans.push({
      category: "🔎 General Diagnosis",
      possibleCauses: [
        "Transient infrastructure issue",
        "Recent configuration or code changes",
        "External service dependency failure",
        "Resource constraints or quotas",
      ],
      diagnosisSteps: [
        "Review the complete error logs for specific error messages",
        "Check if recent commits might have affected this workflow",
        "Verify all external dependencies are available",
        "Re-run the workflow to check if it's a transient issue",
      ],
    });
  }

  return plans;
}

/**
 * Format diagnosis plans into markdown
 */
export function formatDiagnosisPlans(plans: DiagnosisPlan[]): string {
  let output = "## 🩺 Diagnosis Plans\n\n";

  for (const plan of plans) {
    output += `### ${plan.category}\n\n`;
    output += "**Possible Causes:**\n";
    for (const cause of plan.possibleCauses) {
      output += `- ${cause}\n`;
    }
    output += "\n**Diagnosis Steps:**\n";
    for (let i = 0; i < plan.diagnosisSteps.length; i++) {
      output += `${i + 1}. ${plan.diagnosisSteps[i]}\n`;
    }
    output += "\n";
  }

  return output;
}

/**
 * Build the issue body for a scheduled job failure
 */
export function buildIssueBody(
  workflowName: string,
  runId: number,
  workflowRunUrl: string,
  headSha: string,
  jobSummaries: JobSummary[],
): string {
  const allLogs = jobSummaries.map((j) => j.logs).join("\n");
  const diagnosisPlans = generateDiagnosisPlans(allLogs);

  let body = `${ISSUE_MARKER}\n`;
  body += `## 🚨 Scheduled Job Failure: ${workflowName}\n\n`;
  body += `A scheduled background job has failed and requires attention.\n\n`;
  body += `**Workflow Run:** [#${runId}](${workflowRunUrl})\n`;
  body += `**Commit:** \`${formatCommitSha(headSha)}\`\n`;
  body += `**Time:** ${new Date().toISOString()}\n\n`;

  body += `---\n\n`;
  body += `## ❌ Failed Jobs\n\n`;

  for (const summary of jobSummaries) {
    body += `### Job: \`${summary.name}\`\n\n`;
    body += `[View Full Logs](${summary.url})\n\n`;
    body += `<details>\n<summary>Error Summary</summary>\n\n`;
    body += `\`\`\`\n${summary.logs}\n\`\`\`\n\n`;
    body += `</details>\n\n`;
  }

  body += `---\n\n`;
  body += formatDiagnosisPlans(diagnosisPlans);

  body += `---\n\n`;
  body += `## 🔧 Suggested Actions\n\n`;
  body += `1. **Review the error logs** above to identify the root cause\n`;
  body += `2. **Check the diagnosis plans** for common causes and steps\n`;
  body += `3. **Re-run the workflow** if it appears to be a transient issue\n`;
  body += `4. **Fix the underlying issue** and close this issue once resolved\n\n`;

  body += `---\n\n`;
  body += `> 🤖 This issue was automatically created by the scheduled job monitoring system.\n`;
  body += `> Close this issue once the problem has been resolved.\n`;

  return body;
}

/**
 * Build the issue title
 */
export function buildIssueTitle(workflowName: string, date: Date): string {
  const dateStr = date.toISOString().split("T")[0]; // YYYY-MM-DD
  return `🔴 Scheduled Job Failed: ${workflowName} (${dateStr})`;
}

/**
 * Convert response data to string (for log downloads)
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
 * Check if there's already an open issue for this workflow failure
 */
async function findExistingIssue(
  octokit: Octokit,
  owner: string,
  repo: string,
  workflowName: string,
): Promise<number | null> {
  try {
    // Search for open issues with our marker and workflow name
    const { data: issues } = await octokit.rest.issues.listForRepo({
      owner,
      repo,
      state: "open",
      labels: "scheduled-job-failure",
      per_page: 100,
    });

    for (const issue of issues) {
      if (
        issue.body?.includes(ISSUE_MARKER) &&
        issue.title.includes(workflowName)
      ) {
        return issue.number;
      }
    }
  } catch {
    // If search fails, continue to create a new issue
    console.log("Failed to search for existing issues, will create new one");
  }

  return null;
}

/**
 * Collect job summaries for failed jobs in a workflow run
 */
async function collectJobSummaries(
  octokit: Octokit,
  owner: string,
  repo: string,
  runId: number,
): Promise<JobSummary[]> {
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
 * Main function to create an issue for a scheduled job failure
 */
export async function createScheduledJobIssue(
  options: ScheduledJobIssueOptions,
): Promise<void> {
  const { token, owner, repo, runId, workflowName, workflowRunUrl, headSha } =
    options;

  const octokit = new Octokit({ auth: token });

  console.log(
    `Processing failure for workflow "${workflowName}" (run #${runId})`,
  );

  // Check for existing open issue to avoid duplicates
  const existingIssue = await findExistingIssue(
    octokit,
    owner,
    repo,
    workflowName,
  );

  if (existingIssue) {
    console.log(
      `Found existing open issue #${existingIssue} for "${workflowName}". Adding comment instead of creating new issue.`,
    );

    // Add a comment to the existing issue instead
    const jobSummaries = await collectJobSummaries(octokit, owner, repo, runId);

    if (jobSummaries.length === 0) {
      console.log("No failed jobs to report");
      return;
    }

    let commentBody = `## 🔄 New Failure Occurrence\n\n`;
    commentBody += `**Workflow Run:** [#${runId}](${workflowRunUrl})\n`;
    commentBody += `**Commit:** \`${formatCommitSha(headSha)}\`\n`;
    commentBody += `**Time:** ${new Date().toISOString()}\n\n`;

    for (const summary of jobSummaries) {
      commentBody += `### Job: \`${summary.name}\`\n\n`;
      commentBody += `[View Full Logs](${summary.url})\n\n`;
      commentBody += `<details>\n<summary>Error Summary</summary>\n\n`;
      commentBody += `\`\`\`\n${summary.logs}\n\`\`\`\n\n`;
      commentBody += `</details>\n\n`;
    }

    commentBody += `> This is an additional occurrence of the same failure.\n`;

    await octokit.rest.issues.createComment({
      owner,
      repo,
      issue_number: existingIssue,
      body: commentBody,
    });

    console.log(`Added failure comment to existing issue #${existingIssue}`);
    return;
  }

  // Collect job summaries
  const jobSummaries = await collectJobSummaries(octokit, owner, repo, runId);

  if (jobSummaries.length === 0) {
    console.log("No failed jobs to report");
    return;
  }

  // Build issue content
  const issueTitle = buildIssueTitle(workflowName, new Date());
  const issueBody = buildIssueBody(
    workflowName,
    runId,
    workflowRunUrl,
    headSha,
    jobSummaries,
  );

  // Create the issue
  const { data: issue } = await octokit.rest.issues.create({
    owner,
    repo,
    title: issueTitle,
    body: issueBody,
    labels: ["scheduled-job-failure", "automated"],
  });

  console.log(`Created issue #${issue.number}: ${issueTitle}`);
  console.log(`Issue URL: ${issue.html_url}`);
}

/**
 * CLI entry point
 */
export function runScheduledJobIssueCLI(): void {
  const token = process.env.GITHUB_TOKEN;
  const owner = process.env.GITHUB_REPOSITORY_OWNER;
  const githubRepository = process.env.GITHUB_REPOSITORY;
  const runIdStr = process.env.WORKFLOW_RUN_ID;
  const workflowName = process.env.WORKFLOW_NAME;
  const workflowRunUrl = process.env.WORKFLOW_RUN_URL;
  const headSha = process.env.HEAD_SHA;

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
  if (!workflowName) {
    console.error("WORKFLOW_NAME is required");
    process.exit(1);
  }
  if (!workflowRunUrl) {
    console.error("WORKFLOW_RUN_URL is required");
    process.exit(1);
  }
  if (!headSha) {
    console.error("HEAD_SHA is required");
    process.exit(1);
  }

  createScheduledJobIssue({
    token,
    owner,
    repo,
    runId,
    workflowName,
    workflowRunUrl,
    headSha,
  }).catch((error) => {
    console.error("Failed to create scheduled job issue:", error);
    process.exit(1);
  });
}
