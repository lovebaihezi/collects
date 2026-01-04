import { describe, expect, test } from "bun:test";
import {
  stripAnsiCodes,
  extractErrorLines,
  countPreviousFailures,
  buildCommentBody,
} from "./ci-feedback.ts";

describe("stripAnsiCodes", () => {
  test("removes basic color codes", () => {
    const input = "\x1B[36;1mhello\x1B[0m";
    expect(stripAnsiCodes(input)).toBe("hello");
  });

  test("removes multiple ANSI codes from a line", () => {
    const input =
      "2026-01-04T09:47:32.7124756Z \x1B[36;1m: install rustup if needed\x1B[0m";
    expect(stripAnsiCodes(input)).toBe(
      "2026-01-04T09:47:32.7124756Z : install rustup if needed",
    );
  });

  test("removes various ANSI escape sequences", () => {
    // Bold
    expect(stripAnsiCodes("\x1B[1mBold\x1B[0m")).toBe("Bold");
    // Red foreground
    expect(stripAnsiCodes("\x1B[31mRed\x1B[0m")).toBe("Red");
    // Green background
    expect(stripAnsiCodes("\x1B[42mGreen BG\x1B[0m")).toBe("Green BG");
    // Combined attributes
    expect(stripAnsiCodes("\x1B[1;31;42mStyled\x1B[0m")).toBe("Styled");
  });

  test("handles text without ANSI codes", () => {
    const input = "plain text without codes";
    expect(stripAnsiCodes(input)).toBe("plain text without codes");
  });

  test("handles empty string", () => {
    expect(stripAnsiCodes("")).toBe("");
  });

  test("handles multiline text with ANSI codes", () => {
    const input = `\x1B[36;1mline1\x1B[0m
\x1B[31merror line\x1B[0m
\x1B[32msuccess\x1B[0m`;
    expect(stripAnsiCodes(input)).toBe(`line1
error line
success`);
  });
});

describe("extractErrorLines", () => {
  test("extracts lines containing 'error'", () => {
    const logs = `line 1
line 2
error: something went wrong
line 4
line 5`;
    const result = extractErrorLines(logs);
    expect(result).toContain("error: something went wrong");
  });

  test("includes context lines around errors", () => {
    const logs = `line 1
line 2
line 3
error: failure here
line 5
line 6
line 7`;
    const result = extractErrorLines(logs);
    // Should include 2 lines before and 2 lines after
    expect(result).toContain("line 2");
    expect(result).toContain("line 3");
    expect(result).toContain("error: failure here");
    expect(result).toContain("line 5");
    expect(result).toContain("line 6");
  });

  test("strips ANSI codes before extracting", () => {
    const logs = `\x1B[36;1mnormal line\x1B[0m
\x1B[31;1merror: red error message\x1B[0m
\x1B[32;1mafter line\x1B[0m`;
    const result = extractErrorLines(logs);
    expect(result).not.toContain("\x1B[");
    expect(result).toContain("error: red error message");
  });

  test("returns last 30 lines when no error patterns found", () => {
    const lines = Array.from({ length: 50 }, (_, i) => `line ${i + 1}`);
    const logs = lines.join("\n");
    const result = extractErrorLines(logs);
    const resultLines = result.split("\n");
    expect(resultLines).toHaveLength(30);
    expect(result).toContain("line 50");
    expect(result).toContain("line 21");
    expect(result).not.toContain("line 20");
  });

  test("limits output to 50 lines", () => {
    // Create many error lines
    const lines = Array.from(
      { length: 100 },
      (_, i) => `error on line ${i + 1}`,
    );
    const logs = lines.join("\n");
    const result = extractErrorLines(logs);
    const resultLines = result.split("\n");
    expect(resultLines.length).toBeLessThanOrEqual(50);
  });

  test("detects various error patterns", () => {
    const logs = `normal
failed: build step
normal
exception thrown
normal
panic occurred
normal
FAILURE in test`;
    const result = extractErrorLines(logs);
    expect(result).toContain("failed: build step");
    expect(result).toContain("exception thrown");
    expect(result).toContain("panic occurred");
    expect(result).toContain("FAILURE in test");
  });
});

describe("countPreviousFailures", () => {
  test("returns empty object for no comments", () => {
    const result = countPreviousFailures([]);
    expect(result).toEqual({});
  });

  test("counts failures from bot comments with CI-FEEDBACK-BOT marker", () => {
    const comments = [
      {
        body: `<!-- CI-FEEDBACK-BOT -->
## 🚨 CI Failure Report

### ❌ Job: \`build\`
Some logs here

### ❌ Job: \`test\`
More logs`,
        user: { type: "Bot" },
      },
    ];
    const result = countPreviousFailures(comments);
    expect(result).toEqual({ build: 1, test: 1 });
  });

  test("ignores comments without CI-FEEDBACK-BOT marker", () => {
    const comments = [
      {
        body: `### ❌ Job: \`build\``,
        user: { type: "Bot" },
      },
    ];
    const result = countPreviousFailures(comments);
    expect(result).toEqual({});
  });

  test("ignores comments from non-bot users", () => {
    const comments = [
      {
        body: `<!-- CI-FEEDBACK-BOT -->
### ❌ Job: \`build\``,
        user: { type: "User" },
      },
    ];
    const result = countPreviousFailures(comments);
    expect(result).toEqual({});
  });

  test("accumulates failures across multiple comments", () => {
    const comments = [
      {
        body: `<!-- CI-FEEDBACK-BOT -->
### ❌ Job: \`build\``,
        user: { type: "Bot" },
      },
      {
        body: `<!-- CI-FEEDBACK-BOT -->
### ❌ Job: \`build\`
### ❌ Job: \`lint\``,
        user: { type: "Bot" },
      },
    ];
    const result = countPreviousFailures(comments);
    expect(result).toEqual({ build: 2, lint: 1 });
  });
});

describe("buildCommentBody", () => {
  test("builds comment with job failures", () => {
    const result = buildCommentBody(
      12345,
      "https://github.com/owner/repo/actions/runs/12345",
      "abc1234567890",
      [{ name: "build", url: "https://example.com/job/1", logs: "error log" }],
      [],
      {},
    );

    expect(result).toContain("<!-- CI-FEEDBACK-BOT -->");
    expect(result).toContain("## 🚨 CI Failure Report");
    expect(result).toContain("#12345");
    expect(result).toContain("`abc1234`");
    expect(result).toContain("### ❌ Job: `build`");
    expect(result).toContain("error log");
    expect(result).toContain("@copilot");
  });

  test("shows correct failure count", () => {
    const result = buildCommentBody(
      12345,
      "https://example.com",
      "abc1234567890",
      [{ name: "test", url: "https://example.com", logs: "error" }],
      [],
      { test: 1 },
    );

    expect(result).toContain("**Failure #2/3**");
  });

  test("includes skipped jobs note", () => {
    const result = buildCommentBody(
      12345,
      "https://example.com",
      "abc1234567890",
      [{ name: "build", url: "https://example.com", logs: "error" }],
      [{ name: "lint", url: "https://example.com", logs: "old error" }],
      {},
    );

    expect(result).toContain(
      "⚠️ **Note:** The following jobs have failed 3+ times",
    );
    expect(result).toContain("`lint`");
  });
});
