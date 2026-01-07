# Testing rules (Collects)

This repo must be tested in a way that covers both **non-internal** and **internal** feature-gated code paths.

## What to run (default)

### UI
- Run **non-internal UI tests**:
  - `just ui::test-non-internal` (or `cd ui && cargo test`)
- Run **internal UI tests** (all features):
  - `just ui::test` (or `cd ui && cargo test --all-features`)

### Services
- Run the relevant command(s) for the change you made. Prefer the repo `just` commands and follow existing workflow conventions.
- If you add new endpoints or change request/response behavior:
  - add/adjust **service integration tests** under `services/tests/`
  - ensure both success and failure cases are covered

## Feature-gated test coverage (IMPORTANT)

Some tests are conditionally compiled:
- Running `cargo test --all-features` enables internal-only code paths and **excludes** tests behind `#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]`.
- Running `cargo test` (no features) enables non-internal code paths and **excludes** internal-only tests behind `#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]`.

Therefore, you must run both:
- non-internal tests (no features)
- internal tests (`--all-features`)

## Where tests live

### UI tests
- Widget unit tests: colocated under `#[cfg(test)]` in `ui/src/**`
- Integration tests: `ui/tests/**`
- Common test helpers: `ui/tests/common/mod.rs`

### Services tests
- Integration tests: `services/tests/**`

## UI testing best practices

1. **Assert on UI, not internal state**
   - Use `kittest` / `egui_kittest` queries (e.g. `query_by_label_contains(...)`) to validate what a user would see.
   - Avoid reading `StateCtx` directly in integration tests unless it’s a unit test for business logic.

2. **Drive frames deterministically**
   - Many UI behaviors require multiple frames. Step the harness enough times to let the UI settle.
   - For flows involving async/network mocks, step frames + wait briefly as needed, then step again before asserting.

3. **Make UI elements queryable**
   - Add stable labels/titles for panels and important controls so tests can find them.

4. **Mock external calls**
   - Prefer `wiremock` for networked UI flows.
   - Cover both 2xx and error responses (4xx/5xx) and verify user-visible error handling.

## Requirements for new work

- New widget: add a widget unit test (in the same file) and an integration test if it’s part of a larger flow.
- New feature: add an end-to-end integration test under the relevant crate’s `tests/` directory.
- New API endpoint: add service tests for happy path + error cases, plus auth/permission cases if applicable.

## Before opening a PR

At minimum:
- UI: `just ui::test-non-internal` and `just ui::test`
- Fix failing tests before requesting review.