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
