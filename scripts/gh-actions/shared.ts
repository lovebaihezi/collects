/**
 * Shared types and constants for GitHub Actions scripts
 */

/**
 * Number of characters to display for commit SHA (standard Git short SHA)
 */
export const COMMIT_SHA_DISPLAY_LENGTH = 7;

/**
 * Summary of a job's failure information
 */
export interface JobSummary {
  name: string;
  url: string;
  logs: string;
}

/**
 * Format a commit SHA for display (truncates to COMMIT_SHA_DISPLAY_LENGTH characters)
 */
export function formatCommitSha(sha: string): string {
  return sha.substring(0, COMMIT_SHA_DISPLAY_LENGTH);
}

/**
 * Parse tags from gcloud artifacts docker images list output.
 * The gcloud command may return tags as either an array or a comma-separated string,
 * depending on the gcloud SDK version and output format.
 */
export function parseTags(tags: string | string[] | undefined): string[] {
  if (Array.isArray(tags)) {
    return tags;
  }
  if (tags) {
    return tags.split(",").map((t) => t.trim());
  }
  return [];
}
