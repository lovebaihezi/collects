import { describe, expect, test } from "bun:test";
import {
  categorizeImage,
  countImagesByCategory,
  extractPrNumber,
  formatImageCounts,
  formatImageCountsMarkdown,
  type DockerImage,
  type ImageCounts,
} from "./artifact-cleanup.ts";

describe("extractPrNumber", () => {
  test("extracts PR number from valid pr-{number} tag", () => {
    expect(extractPrNumber("pr-123")).toBe(123);
    expect(extractPrNumber("pr-1")).toBe(1);
    expect(extractPrNumber("pr-99999")).toBe(99999);
  });

  test("returns null for invalid tags", () => {
    expect(extractPrNumber("pr-")).toBeNull();
    expect(extractPrNumber("pr-abc")).toBeNull();
    expect(extractPrNumber("main-abc123")).toBeNull();
    expect(extractPrNumber("v1.0.0")).toBeNull();
    expect(extractPrNumber("nightly-20260112")).toBeNull();
    expect(extractPrNumber("")).toBeNull();
    expect(extractPrNumber("pr-123-extra")).toBeNull();
  });
});

describe("categorizeImage", () => {
  test("categorizes PR images", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: ["pr-123"],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("pr");
  });

  test("categorizes nightly images", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: ["nightly-20260112"],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("nightly");
  });

  test("categorizes main branch images", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: ["main-abc123f"],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("main");
  });

  test("categorizes production images", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: ["v1.2.3"],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("production");
  });

  test("categorizes unknown images with unrecognized tags", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: ["custom-tag"],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("unknown");
  });

  test("categorizes untagged images as unknown", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: [],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("unknown");
  });

  test("prioritizes first matching tag", () => {
    const image: DockerImage = {
      digest: "sha256:abc",
      tags: ["pr-123", "v1.0.0"],
      createTime: new Date(),
    };
    expect(categorizeImage(image)).toBe("pr");
  });
});

describe("countImagesByCategory", () => {
  test("counts empty array", () => {
    const result = countImagesByCategory([]);
    expect(result).toEqual({
      total: 0,
      pr: 0,
      nightly: 0,
      main: 0,
      production: 0,
      unknown: 0,
    });
  });

  test("counts images by category", () => {
    const images: DockerImage[] = [
      { digest: "sha256:1", tags: ["pr-1"], createTime: new Date() },
      { digest: "sha256:2", tags: ["pr-2"], createTime: new Date() },
      {
        digest: "sha256:3",
        tags: ["nightly-20260112"],
        createTime: new Date(),
      },
      { digest: "sha256:4", tags: ["main-abc123"], createTime: new Date() },
      { digest: "sha256:5", tags: ["v1.0.0"], createTime: new Date() },
      { digest: "sha256:6", tags: ["custom"], createTime: new Date() },
    ];

    const result = countImagesByCategory(images);
    expect(result.total).toBe(6);
    expect(result.pr).toBe(2);
    expect(result.nightly).toBe(1);
    expect(result.main).toBe(1);
    expect(result.production).toBe(1);
    expect(result.unknown).toBe(1);
  });
});

describe("formatImageCounts", () => {
  test("formats counts correctly", () => {
    const counts: ImageCounts = {
      total: 10,
      pr: 3,
      nightly: 2,
      main: 2,
      production: 2,
      unknown: 1,
    };

    const result = formatImageCounts(counts);
    expect(result).toContain("Total: 10");
    expect(result).toContain("PR builds: 3");
    expect(result).toContain("Nightly builds: 2");
    expect(result).toContain("Main branch builds: 2");
    expect(result).toContain("Production releases: 2");
    expect(result).toContain("Unknown/Untagged: 1");
  });
});

describe("formatImageCountsMarkdown", () => {
  test("formats counts as markdown table", () => {
    const counts: ImageCounts = {
      total: 5,
      pr: 1,
      nightly: 1,
      main: 1,
      production: 1,
      unknown: 1,
    };

    const result = formatImageCountsMarkdown("📊 Before Cleanup", counts);
    expect(result).toContain("### 📊 Before Cleanup");
    expect(result).toContain("| Image Type | Count |");
    expect(result).toContain("| **Total** | 5 |");
    expect(result).toContain("| PR builds | 1 |");
    expect(result).toContain("| Nightly builds | 1 |");
    expect(result).toContain("| Main branch builds | 1 |");
    expect(result).toContain("| Production releases | 1 |");
    expect(result).toContain("| Unknown/Untagged | 1 |");
  });

  test("handles zero counts", () => {
    const counts: ImageCounts = {
      total: 0,
      pr: 0,
      nightly: 0,
      main: 0,
      production: 0,
      unknown: 0,
    };

    const result = formatImageCountsMarkdown("After Cleanup", counts);
    expect(result).toContain("### After Cleanup");
    expect(result).toContain("| **Total** | 0 |");
  });
});
