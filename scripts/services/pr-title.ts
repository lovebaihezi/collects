/**
 * Valid conventional commit types
 */
const VALID_TYPES = [
  "feat",
  "fix",
  "docs",
  "style",
  "refactor",
  "perf",
  "test",
  "build",
  "ci",
  "chore",
  "revert",
] as const;

type CommitType = (typeof VALID_TYPES)[number];

/**
 * Conventional commit pattern - built dynamically from VALID_TYPES
 * Format: <type>[optional scope]: <description>
 */
const COMMIT_PATTERN = new RegExp(
  `^(${VALID_TYPES.join("|")})(\\([a-zA-Z0-9_-]+\\))?: .+`,
);

/**
 * Result of PR title validation
 */
export interface PrTitleValidationResult {
  valid: boolean;
  title: string;
  error?: string;
}

/**
 * Type descriptions for help text
 */
const TYPE_DESCRIPTIONS: Record<CommitType, string> = {
  feat: "A new feature",
  fix: "A bug fix",
  docs: "Documentation only changes",
  style: "Changes that do not affect code meaning (formatting, etc.)",
  refactor: "Code change that neither fixes a bug nor adds a feature",
  perf: "A code change that improves performance",
  test: "Adding missing tests or correcting existing tests",
  build: "Changes affecting build system or external dependencies",
  ci: "Changes to CI configuration files and scripts",
  chore: "Other changes that don't modify src or test files",
  revert: "Reverts a previous commit",
};

/**
 * Validates a PR title against conventional commit format
 */
export function validatePrTitle(title: string): PrTitleValidationResult {
  const trimmedTitle = title.trim();

  // Validate the PR title
  if (COMMIT_PATTERN.test(trimmedTitle)) {
    return { valid: true, title: trimmedTitle };
  }

  return {
    valid: false,
    title: trimmedTitle,
    error: "Invalid PR title format",
  };
}

/**
 * Generates error message for invalid PR title
 */
export function formatErrorMessage(title: string): string {
  const lines = [
    "ERROR: Invalid PR title format!",
    "",
    "Your PR title:",
    `  ${title}`,
    "",
    "Expected format: <type>[optional scope]: <description>",
    "",
    "Valid types:",
  ];

  for (const type of VALID_TYPES) {
    lines.push(`  ${type.padEnd(10)}: ${TYPE_DESCRIPTIONS[type]}`);
  }

  lines.push("");
  lines.push("Examples:");
  lines.push("  feat: add user authentication");
  lines.push("  fix(ui): resolve button alignment issue");
  lines.push("  docs: update README with installation instructions");
  lines.push("");

  return lines.join("\n");
}

/**
 * Main function to run PR title validation
 */
export function runPrTitleCheck(title: string): void {
  if (!title) {
    console.error("ERROR: PR title is required");
    process.exit(1);
  }

  const result = validatePrTitle(title);

  if (result.valid) {
    console.log(`✓ Valid PR title: ${result.title}`);
    process.exit(0);
  }

  console.error(formatErrorMessage(result.title));
  process.exit(1);
}
