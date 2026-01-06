import { describe, expect, test } from "bun:test";
import {
  generateDiagnosisPlans,
  formatDiagnosisPlans,
  buildIssueBody,
  buildIssueTitle,
} from "./scheduled-job-issue.ts";

describe("generateDiagnosisPlans", () => {
  test("detects authentication/permission issues", () => {
    const logs = "Error: permission denied when accessing resource";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Authentication"))).toBe(true);
    const authPlan = plans.find((p) => p.category.includes("Authentication"));
    expect(authPlan?.possibleCauses.length).toBeGreaterThan(0);
    expect(authPlan?.diagnosisSteps.length).toBeGreaterThan(0);
  });

  test("detects workload identity issues", () => {
    const logs = "Failed to configure workload identity federation";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Authentication"))).toBe(true);
  });

  test("detects 403 unauthorized errors", () => {
    const logs = "HTTP 403: Access denied to resource";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Authentication"))).toBe(true);
  });

  test("detects network/connectivity issues", () => {
    const logs = "Connection refused: unable to reach server";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Network"))).toBe(true);
    const networkPlan = plans.find((p) => p.category.includes("Network"));
    expect(networkPlan?.possibleCauses).toContain(
      "External service temporary outage",
    );
  });

  test("detects timeout errors", () => {
    const logs = "Request timeout after 30 seconds";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Network"))).toBe(true);
  });

  test("detects 502/503/504 gateway errors", () => {
    const logs = "HTTP 502 Bad Gateway from upstream server";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Network"))).toBe(true);
  });

  test("detects resource not found issues", () => {
    const logs = "Error 404: The requested resource was not found";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Not Found"))).toBe(true);
    const notFoundPlan = plans.find((p) => p.category.includes("Not Found"));
    expect(notFoundPlan?.possibleCauses).toContain(
      "Referenced resource was deleted or moved",
    );
  });

  test("detects Docker/container issues", () => {
    const logs =
      "Error: Unable to pull image from artifact registry us-east1-docker.pkg.dev";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Docker"))).toBe(true);
    const dockerPlan = plans.find((p) => p.category.includes("Docker"));
    expect(dockerPlan?.diagnosisSteps.some((s) => s.includes("Artifact"))).toBe(
      true,
    );
  });

  test("detects Google Cloud issues", () => {
    const logs = "gcloud: ERROR: (gcloud.run.deploy) Could not deploy service";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Google Cloud"))).toBe(true);
    const gcpPlan = plans.find((p) => p.category.includes("Google Cloud"));
    expect(
      gcpPlan?.diagnosisSteps.some((s) => s.includes("Google Cloud")),
    ).toBe(true);
  });

  test("detects script execution errors", () => {
    const logs = "Error: Process exited with non-zero exit code 1";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.some((p) => p.category.includes("Script"))).toBe(true);
  });

  test("provides generic diagnosis when no patterns match", () => {
    const logs = "some random log output without specific error patterns";
    const plans = generateDiagnosisPlans(logs);

    expect(plans.length).toBeGreaterThan(0);
    expect(plans.some((p) => p.category.includes("General"))).toBe(true);
  });

  test("returns multiple diagnosis plans for logs with multiple error types", () => {
    const logs = `
      Permission denied: cannot access resource
      Connection timeout after 30s
      Docker image not found
    `;
    const plans = generateDiagnosisPlans(logs);

    expect(plans.length).toBeGreaterThanOrEqual(3);
    expect(plans.some((p) => p.category.includes("Authentication"))).toBe(true);
    expect(plans.some((p) => p.category.includes("Network"))).toBe(true);
    expect(plans.some((p) => p.category.includes("Docker"))).toBe(true);
  });
});

describe("formatDiagnosisPlans", () => {
  test("formats plans with proper markdown structure", () => {
    const plans = [
      {
        category: "🔐 Test Category",
        possibleCauses: ["Cause 1", "Cause 2"],
        diagnosisSteps: ["Step 1", "Step 2"],
      },
    ];

    const formatted = formatDiagnosisPlans(plans);

    expect(formatted).toContain("## 🩺 Diagnosis Plans");
    expect(formatted).toContain("### 🔐 Test Category");
    expect(formatted).toContain("**Possible Causes:**");
    expect(formatted).toContain("- Cause 1");
    expect(formatted).toContain("- Cause 2");
    expect(formatted).toContain("**Diagnosis Steps:**");
    expect(formatted).toContain("1. Step 1");
    expect(formatted).toContain("2. Step 2");
  });

  test("handles multiple plans", () => {
    const plans = [
      {
        category: "Category A",
        possibleCauses: ["A cause"],
        diagnosisSteps: ["A step"],
      },
      {
        category: "Category B",
        possibleCauses: ["B cause"],
        diagnosisSteps: ["B step"],
      },
    ];

    const formatted = formatDiagnosisPlans(plans);

    expect(formatted).toContain("### Category A");
    expect(formatted).toContain("### Category B");
    expect(formatted).toContain("- A cause");
    expect(formatted).toContain("- B cause");
  });

  test("handles empty plans array", () => {
    const formatted = formatDiagnosisPlans([]);

    expect(formatted).toContain("## 🩺 Diagnosis Plans");
    expect(formatted).not.toContain("###");
  });
});

describe("buildIssueBody", () => {
  test("includes all required sections", () => {
    const body = buildIssueBody(
      "Artifact Cleanup",
      12345,
      "https://github.com/owner/repo/actions/runs/12345",
      "abc1234567890",
      [
        {
          name: "cleanup",
          url: "https://example.com/job/1",
          logs: "error: something failed",
        },
      ],
    );

    expect(body).toContain("<!-- SCHEDULED-JOB-FAILURE-BOT -->");
    expect(body).toContain("Artifact Cleanup");
    expect(body).toContain("#12345");
    expect(body).toContain("`abc1234`");
    expect(body).toContain("❌ Failed Jobs");
    expect(body).toContain("`cleanup`");
    expect(body).toContain("🩺 Diagnosis Plans");
    expect(body).toContain("🔧 Suggested Actions");
  });

  test("includes job logs in collapsible section", () => {
    const body = buildIssueBody(
      "Test Workflow",
      99999,
      "https://example.com",
      "def5678",
      [
        {
          name: "test-job",
          url: "https://example.com",
          logs: "test error log",
        },
      ],
    );

    expect(body).toContain("<details>");
    expect(body).toContain("<summary>Error Summary</summary>");
    expect(body).toContain("test error log");
    expect(body).toContain("</details>");
  });

  test("handles multiple failed jobs", () => {
    const body = buildIssueBody(
      "Multi Job Workflow",
      11111,
      "https://example.com",
      "ghi9012",
      [
        { name: "job-1", url: "https://example.com/1", logs: "error 1" },
        { name: "job-2", url: "https://example.com/2", logs: "error 2" },
      ],
    );

    expect(body).toContain("`job-1`");
    expect(body).toContain("`job-2`");
    expect(body).toContain("error 1");
    expect(body).toContain("error 2");
  });

  test("includes timestamp", () => {
    const beforeTime = new Date().toISOString().split("T")[0];
    const body = buildIssueBody("Test", 1, "https://example.com", "abc123", []);
    const afterTime = new Date().toISOString().split("T")[0];

    // Check that the timestamp in the body matches today's date
    expect(body).toMatch(new RegExp(`${beforeTime}|${afterTime}`));
  });
});

describe("buildIssueTitle", () => {
  test("includes workflow name and date", () => {
    const date = new Date("2026-01-06T10:00:00Z");
    const title = buildIssueTitle("Artifact Cleanup", date);

    expect(title).toContain("🔴");
    expect(title).toContain("Scheduled Job Failed");
    expect(title).toContain("Artifact Cleanup");
    expect(title).toContain("2026-01-06");
  });

  test("handles different workflow names", () => {
    const date = new Date("2026-01-06");

    expect(buildIssueTitle("Daily Backup", date)).toContain("Daily Backup");
    expect(buildIssueTitle("Weekly Report", date)).toContain("Weekly Report");
  });

  test("formats date correctly", () => {
    const date = new Date("2025-12-25T23:59:59Z");
    const title = buildIssueTitle("Test", date);

    expect(title).toContain("2025-12-25");
  });
});
