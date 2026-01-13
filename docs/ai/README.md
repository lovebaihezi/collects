# AI Docs Index (Selective Context Loading)

This folder contains **small, task-focused** documents. When working on the repo, load *only* the docs relevant to your change into context.

## Golden rules (read this first)

1. **Keep context small:** include only the doc(s) listed under the task you’re doing.
2. **Don’t guess:** if you’re changing behavior, find the relevant doc here and follow it.
3. **UI thread is the single writer:** async work must not mutate live state directly (see `state-model.md`).

---

## Which doc should you load?

### If you touch `StateCtx`, `Compute`, `Command`, async work, or eframe frame lifecycle
Read:
- `docs/ai/state-model.md`
- `docs/ai/send-safe-state.md` (if adding/modifying State types)

Typical tasks:
- adding a new command
- running concurrent async work (native + WASM)
- adding end-of-frame command queue flushing
- Updater usage, snapshot-based reads, request generation rules
- determining if a State is Send-safe or UI-affine

---

### If you touch CI workflows, scripts, or GitHub Actions
Read:
- `docs/ai/scripts-and-ci.md`

Typical tasks:
- editing `.github/workflows/*.yml`
- adding scripts under `scripts/`
- adding CI feedback steps
- “always run via `just`” rules

---

### If you touch tests or add a feature that needs tests
Read:
- `docs/ai/testing.md`

Typical tasks:
- adding widget unit tests
- adding integration tests under `ui/tests/`
- internal vs non-internal feature gating (`--all-features` vs default)
- which `just` commands to run

---

### If you touch version display / release metadata / build env vars
Read:
- `docs/ai/versioning.md`

Typical tasks:
- `{env}:{info}` formatting rules
- header/version display consistency across UI/services
- build-time environment variables

---

### If you touch commits, PR titles, or want to know required conventions
Read:
- `docs/ai/commits.md`

Typical tasks:
- selecting Conventional Commit type/scope
- validating PR title format

---

## Suggested workflow for agents

When you start a task:
1. Identify which area you’re changing (state model / CI / tests / versioning / commits).
2. Load the corresponding doc(s) above into context.
3. Make the smallest correct change.
4. Run the relevant checks (see `testing.md` and `scripts-and-ci.md`).

---

## Doc map

- `docs/ai/README.md` — this index (start here)
- `docs/ai/state-model.md` — StateCtx / Compute / Command rules (async-safe, snapshot-based, end-of-frame)
- `docs/ai/send-safe-state.md` — Send-safe vs UI-affine state boundary rules
- `docs/ai/testing.md` — test strategy + commands
- `docs/ai/scripts-and-ci.md` — scripts + GitHub Actions rules
- `docs/ai/versioning.md` — version format + release metadata rules
- `docs/ai/commits.md` — Conventional Commits + PR title rules

---

## Autopsy reports

Post-mortems for significant bugs that escaped testing. Read these to understand past mistakes and avoid repeating them.

- `docs/ai/autopsy-otp-countdown-2025-01.md` — OTP countdown timer not updating + stale code flash on reveal
  - **Key lessons:** egui reactive rendering requires explicit `request_repaint()` for time-based updates; state machine UI must handle all states (including `InFlight`)