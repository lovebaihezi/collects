#!/usr/bin/env bun
import { runCommitCLI } from "./commit/index.ts";

runCommitCLI(process.argv.slice(2)).catch((err) => {
  console.error(err instanceof Error ? err.message : String(err));
  process.exit(1);
});
