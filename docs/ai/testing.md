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
  - **update OpenAPI documentation** by adding `#[utoipa::path(...)]` annotations to new endpoints
    - See existing examples in `services/src/v1/*.rs`
    - Register new types in `services/src/openapi.rs` if needed
    - API docs are available at `/api/docs` (internal environments only, requires Zero Trust)

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

## Business layer testing with mock servers

For testing business commands (CreateContent, ListContents, GetContent, etc.) without hitting real API endpoints, use the `test_utils` module in `collects-business`.

### Setup

The `test_utils` module provides:
- `TestContext`: wraps a `MockServer` and `StateCtx` configured to use it
- Helper methods to mock API responses (`mock_list_contents`, `mock_get_content`, etc.)
- `set_authenticated(token)`: sets auth state for authenticated endpoints
- `flush_and_wait()`: flushes commands and awaits all async tasks

### Example test

```rust
use collects_business::test_utils::{TestContext, sample_content_item};
use collects_business::{ListContentsCommand, ListContentsCompute, ListContentsInput, ListContentsStatus};

#[tokio::test]
async fn test_list_contents_success() {
    let mut test_ctx = TestContext::new().await;

    // Set up authentication (most endpoints require it)
    test_ctx.set_authenticated("test_token");

    // Mount mock response
    let items = vec![sample_content_item("1"), sample_content_item("2")];
    test_ctx.mock_list_contents(items, 2).await;

    // Set input and enqueue command
    test_ctx.ctx.update::<ListContentsInput>(|input| {
        input.limit = Some(10);
        input.offset = Some(0);
    });
    test_ctx.ctx.enqueue_command::<ListContentsCommand>();

    // Flush and await all tasks
    test_ctx.flush_and_wait().await;

    // Verify result
    let compute = test_ctx.ctx.compute::<ListContentsCompute>();
    match &compute.status {
        ListContentsStatus::Success(items) => {
            assert_eq!(items.len(), 2);
        }
        other => panic!("Expected Success, got {:?}", other),
    }

    test_ctx.shutdown().await;
}
```

### Key patterns

1. **Always call `shutdown()`** at the end of tests to clean up async tasks
2. **Use `flush_and_wait()`** to execute commands - it:
   - Syncs pending compute updates
   - Flushes the command queue (spawns async tasks)
   - Awaits all tasks in the JoinSet
   - Syncs again to apply results
3. **Mock responses must match API formats** - check the actual command implementation for expected response shapes
4. **Test both success and error cases** - including unauthenticated access

### Available mock helpers

- `mock_login(success, token, username)` - login endpoint
- `mock_validate_token(valid, username)` - token validation
- `mock_list_contents(items, total)` - list contents
- `mock_get_content(id, content)` - get single content
- `mock_get_content_not_found(id)` - 404 response
- `mock_view_url(content_id, url, expires_at)` - presigned URL
- `mock_create_content(content_id)` - inline content creation
- `mock_full_upload(upload_id, content_id)` - file upload flow (init + put + complete)

## Requirements for new work

- New widget: add a widget unit test (in the same file) and an integration test if it's part of a larger flow.
- New feature: add an end-to-end integration test under the relevant crate's `tests/` directory.
- New API endpoint: add service tests for happy path + error cases, plus auth/permission cases if applicable.
- New business command: add mock server tests in `business/src/test_utils.rs` covering success, error, and unauthenticated cases.

## Before opening a PR

At minimum:
- UI: `just ui::test-non-internal` and `just ui::test`
- Fix failing tests before requesting review.