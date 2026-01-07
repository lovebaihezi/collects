# Copilot Agent Instructions (Minimal Entrypoint)

This file is intentionally minimal. Do not expand it into a monolith.

## How to use these docs

1. Identify the area you are changing.
2. Load only the relevant doc(s) under `docs/ai/` into context.
3. Follow those rules precisely.

Index:
- `docs/ai/README.md` — start here (which doc to load)

## Non-negotiable rules (always apply)

### 1) Conventional Commits + PR titles
- All commits and PR titles **MUST** follow Conventional Commits.
- See: `docs/ai/commits.md`

### 2) State/Compute/Command rules (eframe/egui)
If you touch anything related to `StateCtx`, `Compute`, `Command`, end-of-frame logic, async work, or WASM:
- Read and follow: `docs/ai/state-model.md`

Key expectations (summary only; details are in the doc):
- Commands execute at **end-of-frame** from a queue.
- Commands read **snapshot clones** only (no borrowing live references).
- Updates go through queued mechanisms (e.g. `Updater::set(...)`).
- Concurrent async must be out-of-order safe using `(TypeId, generation)`.

### 3) CI + scripts
If you touch `.github/workflows/**`, `.github/actions/**`, or `scripts/**`:
- Read and follow: `docs/ai/scripts-and-ci.md`

### 4) Testing
If you add/change features or behavior:
- Read and follow: `docs/ai/testing.md`

### 5) Version formatting / release metadata
If you touch version display, headers, or build metadata:
- Read and follow: `docs/ai/versioning.md`

## General engineering expectations

- Don’t guess file locations or existing interfaces: search and read the relevant code first.
- Do not hardcode secrets or API keys.
- Prefer small, reviewable changes with tests when behavior changes.
