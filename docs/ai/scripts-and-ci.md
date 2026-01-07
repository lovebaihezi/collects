# Scripts and CI Rules (Concise)

This doc defines the repository conventions for helper scripts, GitHub Actions usage, and CI feedback behavior. Keep changes aligned with these rules.

---

## Golden Rules

1. **In GitHub Actions, always run repo commands via `just`.**
   - Do **not** call `bun`, `cargo`, `npm`, etc. directly from workflow steps unless there is an explicit exception documented elsewhere.
2. **PR workflows must post CI feedback on failure** (comment on PR) for every job that can fail.
3. **Keep automation code in `scripts/`** (Bun + TypeScript) and expose it through `scripts/main.ts` and `scripts/mod.just`.

---

## Scripts: Placement and Structure

All helper scripts and GitHub Actions utilities live under `scripts/` and use **Bun** as the TypeScript runtime.

Expected structure:

- `scripts/main.ts`: CLI entry point (uses `cac`)
- `scripts/mod.just`: `just` commands for scripts (preferred entrypoints)
- `scripts/package.json`: Bun dependencies
- `scripts/gh-actions/`: scripts invoked by GitHub Actions workflows
- `scripts/services/`: service/environment management scripts (cloud, DB, env config)

### Where to put a new script

- **Called from `.github/workflows/*.yml`?** put it in `scripts/gh-actions/`
- **Cloud/DB/env tooling?** put it in `scripts/services/`
- **General developer tooling?** still belongs under `scripts/` and should be invocable through `scripts/main.ts`

### How to add a new script (checklist)

1. Add the script file in the correct directory (see above).
2. Export a CLI function, e.g. `runMyScriptCLI()`.
3. Register a `cac` command in `scripts/main.ts`.
4. Add a `just` command in `scripts/mod.just` that calls the CLI.
5. Add any new deps to `scripts/package.json`.

---

## GitHub Actions: Required Practices

### Use `just` (MUST)

**Do**
- `run: just scripts::<command>`
- `run: just <repo-command>`

**Don’t**
- `run: bun run main.ts <command>`
- `run: bun install && bun run ...`
- `working-directory: scripts` + raw Bun calls

Rationale:
- Centralizes command definitions
- Ensures consistent local vs CI behavior
- The `just` commands handle installation/setup consistently

### CI feedback on PR failures (MUST for `pull_request` jobs)

All jobs that run on `pull_request` must include a final step that posts feedback **only when the job fails**.

**Required permissions**
- `contents: read`
- `pull-requests: write`

**Required step pattern**
- Must run at the end of each job
- Must be conditional on failure
- Must include a `job-name` that matches the job `name:` exactly

Example (template):

```/dev/null/workflow-snippet.yml#L1-22
jobs:
  my-check:
    name: "Check: Something"
    permissions:
      contents: read
      pull-requests: write
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run Validation
        run: just my-validation-command

      - name: Post CI Feedback on Failure
        if: failure() && github.event_name == 'pull_request'
        uses: ./.github/actions/ci-feedback
        with:
          github-token: ${{ secrets.COPILOT_INVOKER_TOKEN }}
          job-name: "Check: Something"
```

### When NOT to add CI feedback

Do not add the feedback step to workflows that only run on:
- `schedule`
- `workflow_dispatch`
- `workflow_run` (post-CI)
- cleanup-only jobs (e.g. `pull_request` with `types: [closed]`)

---

## Making Changes Safely

When you modify:
- `.github/workflows/*.yml`
- scripts under `scripts/**`
- `.github/actions/**`

Do the following:
1. Ensure every workflow step calls into `just`.
2. Ensure PR jobs include the CI feedback failure step.
3. Ensure new scripts are exposed via `scripts/main.ts` and `scripts/mod.just`.

---

## Quick Reference

- Run a script locally:
  - `just scripts::<command>`
- Validate PR title:
  - `just scripts::check-pr-title "<type(scope): description>"`
- If a workflow needs to call a script:
  - Add a `just` command first, then call that `just` command from the workflow.
