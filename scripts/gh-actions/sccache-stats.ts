/**
 * sccache Statistics Script
 *
 * Displays sccache statistics and reports cache performance metrics
 * for GitHub Actions workflows.
 */

import { $ } from "bun";

export interface SccacheStats {
  cacheHitRate: string;
  compileRequests: number;
  cacheHits: number;
  cacheMisses: number;
  cacheSize: string;
}

interface SccacheJsonStats {
  stats?: {
    compile_requests?: number;
    cache_hits?: {
      counts?: {
        Rust?: number;
      };
    };
    cache_misses?: {
      counts?: {
        Rust?: number;
      };
    };
    cache_location?: {
      S3?: {
        size?: string;
      };
    };
  };
}

/**
 * Get sccache statistics by running sccache --show-stats
 */
export async function getSccacheStats(): Promise<SccacheStats> {
  // First show human-readable stats
  console.log("=== sccache Statistics ===");
  try {
    const humanStats = await $`sccache --show-stats`.text();
    console.log(humanStats);
  } catch (error) {
    console.log("Could not get human-readable stats");
  }

  // Try to get JSON stats for parsing
  let jsonStats: SccacheJsonStats = {};
  try {
    const jsonResult = await $`sccache --show-stats --stats-format=json`.text();
    jsonStats = JSON.parse(jsonResult) as SccacheJsonStats;
  } catch {
    console.log("Could not parse JSON stats");
  }

  const compileRequests = jsonStats.stats?.compile_requests ?? 0;
  const cacheHits = jsonStats.stats?.cache_hits?.counts?.Rust ?? 0;
  const cacheMisses = jsonStats.stats?.cache_misses?.counts?.Rust ?? 0;
  const cacheSize = jsonStats.stats?.cache_location?.S3?.size ?? "N/A";

  // Calculate hit rate
  const total = cacheHits + cacheMisses;
  let cacheHitRate: string;
  if (total > 0) {
    cacheHitRate = ((cacheHits * 100) / total).toFixed(1);
  } else {
    cacheHitRate = "0.0";
  }

  return {
    cacheHitRate,
    compileRequests,
    cacheHits,
    cacheMisses,
    cacheSize,
  };
}

/**
 * Output stats to GitHub Actions
 */
async function outputToGitHub(stats: SccacheStats): Promise<void> {
  const githubOutput = process.env.GITHUB_OUTPUT;
  if (githubOutput) {
    const outputs = [
      `cache-hit-rate=${stats.cacheHitRate}`,
      `compile-requests=${stats.compileRequests}`,
      `cache-hits=${stats.cacheHits}`,
      `cache-misses=${stats.cacheMisses}`,
      `cache-size=${stats.cacheSize}`,
    ].join("\n");
    const file = Bun.file(githubOutput);
    const existingContent = (await file.exists()) ? await file.text() : "";
    await Bun.write(githubOutput, existingContent + outputs + "\n");
  }
}

/**
 * Print cache performance summary
 */
function printSummary(stats: SccacheStats): void {
  console.log("");
  console.log("=== Cache Performance Summary ===");
  console.log(`📊 Cache Hit Rate: ${stats.cacheHitRate}%`);
  console.log(`🎯 Cache Hits: ${stats.cacheHits}`);
  console.log(`❌ Cache Misses: ${stats.cacheMisses}`);
  console.log(`📦 Cache Size: ${stats.cacheSize}`);
}

/**
 * Stop sccache server
 */
async function stopSccache(): Promise<void> {
  try {
    await $`sccache --stop-server`.quiet();
  } catch {
    // Ignore errors when stopping server
  }
}

/**
 * Main CLI entry point for sccache-stats
 */
export async function runSccacheStatsCLI(): Promise<void> {
  try {
    const stats = await getSccacheStats();
    await outputToGitHub(stats);
    printSummary(stats);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Error getting sccache stats: ${message}`);
    // Output N/A values on error
    const fallbackStats: SccacheStats = {
      cacheHitRate: "N/A",
      compileRequests: 0,
      cacheHits: 0,
      cacheMisses: 0,
      cacheSize: "N/A",
    };
    await outputToGitHub(fallbackStats);
  } finally {
    await stopSccache();
  }
}
