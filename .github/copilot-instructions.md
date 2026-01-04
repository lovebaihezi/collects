# Copilot Agent Instructions

This document provides guidelines for AI coding agents (like GitHub Copilot) working on this repository.

## Commit Message Convention

All commits **MUST** follow the [Conventional Commits](https://www.conventionalcommits.org/) specification.

### Format

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Valid Types

| Type       | Description                                                      |
|------------|------------------------------------------------------------------|
| `feat`     | A new feature                                                    |
| `fix`      | A bug fix                                                        |
| `docs`     | Documentation only changes                                       |
| `style`    | Changes that do not affect code meaning (formatting, etc.)       |
| `refactor` | Code change that neither fixes a bug nor adds a feature          |
| `perf`     | A code change that improves performance                          |
| `test`     | Adding missing tests or correcting existing tests                |
| `build`    | Changes affecting build system or external dependencies          |
| `ci`       | Changes to CI configuration files and scripts                    |
| `chore`    | Other changes that don't modify src or test files                |
| `revert`   | Reverts a previous commit                                        |

### Examples

```
feat: add user authentication
fix(ui): resolve button alignment issue
docs: update README with installation instructions
refactor(api): simplify error handling logic
test: add unit tests for validation module
```

### Rules

1. **Type is required**: Every commit must start with a valid type
2. **Scope is optional**: Use parentheses to specify the affected area, e.g., `fix(ui):`
3. **Description is required**: Brief summary of the change in imperative mood
4. **Use lowercase**: Type and scope should be lowercase
5. **No period at end**: Don't end the description with a period

## PR Title Convention

PR titles follow the same conventional commits format. The PR title will be validated in CI using:

```bash
just scripts::check-pr-title "<pr-title>"
```

## Code Style

- **TypeScript**: Use Prettier for formatting, oxlint for linting
- **Rust**: Use `cargo fmt` for formatting, `cargo clippy` for linting
- Run `just check-fmt` and `just check-lint` before committing

### String Interning with Ustr

When working with strings that are frequently cloned or compared (e.g., usernames, identifiers, keys), **MUST** use `Ustr` instead of `String`.

**Why:**
- `Ustr` provides Copy semantics (zero-cost cloning)
- String interning means identical strings share memory
- Pre-computed hash for efficient HashMap lookups

**When to use `Ustr`:**
- Enum variants containing identifiers (e.g., `UserAction::ShowQrCode(Ustr)`)
- HashMap keys for lookups (e.g., `HashMap<Ustr, bool>`)
- Struct fields that are frequently cloned or passed around
- Any string that doesn't require mutation

**When to keep `String`:**
- User input fields that require mutation (e.g., `text_edit_singleline`)
- Error messages that are constructed once and rarely cloned
- Data that comes from external sources and won't be compared/hashed

**Example:**
```rust
// ✅ Good: Use Ustr for frequently cloned identifiers
pub enum UserAction {
    ShowQrCode(Ustr),
    EditUsername(Ustr),
}
let username = Ustr::from(&user.username);

// ❌ Bad: Unnecessary heap allocation on clone
pub enum UserAction {
    ShowQrCode(String),
    EditUsername(String),
}
let username = user.username.clone();
```

## Scripts Organization

All helper scripts, automation tools, and GitHub Actions utilities are located in the `scripts/` directory using **Bun** as the TypeScript runtime.

### Directory Structure

```
scripts/
├── main.ts              # CLI entry point (cac-based)
├── mod.just             # Just commands for scripts
├── package.json         # Bun dependencies
├── gh-actions/          # GitHub Actions-related scripts
│   ├── version-check.ts # Version change detection
│   └── ci-feedback.ts   # CI failure feedback for Copilot
└── services/            # Service management scripts
    ├── neon.ts          # Neon database management
    ├── gcloud.ts        # Google Cloud setup
    ├── env-config.ts    # Environment configuration
    └── pr-title.ts      # PR title validation
```

### Script Placement Rules

1. **GitHub Actions scripts**: Place in `scripts/gh-actions/`
   - Scripts called from workflow files (`.github/workflows/*.yml`)
   - Use `@octokit/rest` for GitHub API interactions
   - Export a CLI function (e.g., `runCIFeedbackCLI`) and register in `main.ts`

2. **Service management scripts**: Place in `scripts/services/`
   - Cloud provider integrations (GCloud, Cloudflare)
   - Database management (Neon)
   - Environment configuration utilities

3. **CLI Integration**: All scripts should be accessible via `main.ts`
   - Register commands using `cac` library
   - Run with: `bun run main.ts <command>`
   - Add corresponding just command in `mod.just`

### Creating a New Script

1. Create the TypeScript file in the appropriate subdirectory
2. Export a function (e.g., `runMyScriptCLI`)
3. Import and register the command in `main.ts`
4. Add a just command in `scripts/mod.just`
5. If new dependencies are needed, add to `scripts/package.json`

### Example: GitHub Actions Script

```typescript
// scripts/gh-actions/my-action.ts
import { Octokit } from "@octokit/rest";

export function runMyActionCLI(): void {
  const token = process.env.GITHUB_TOKEN;
  // ... implementation
}
```

```typescript
// In main.ts
import { runMyActionCLI } from "./gh-actions/my-action.ts";

cli.command("my-action", "Description of my action")
  .action(() => runMyActionCLI());
```

```just
# In scripts/mod.just - add the just command
my-action: install
    bun run main.ts my-action
```

```yaml
# In workflow file - ALWAYS use just commands, never call bun directly
- name: Setup Bun
  uses: oven-sh/setup-bun@v2

- name: Setup Just
  uses: extractions/setup-just@v2

- name: Run My Action
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
  run: just scripts::my-action
```

### GitHub Actions Best Practices

**IMPORTANT**: All commands in GitHub Actions workflows **MUST** be invoked through `just` commands, never directly via `bun` or other runtimes.

**Why:**
- Centralizes command definitions in justfiles for consistency
- The just command handles dependency installation automatically (via `install` dependency)
- Makes local development and CI use the same commands
- Easier to update commands in one place

**Do:**
```yaml
run: just scripts::my-action
run: just scripts::check-pr-title "${{ github.event.pull_request.title }}"
```

**Don't:**
```yaml
run: bun run main.ts my-action
run: bun install && bun run main.ts my-action
working-directory: scripts
run: bun run main.ts my-action
```

## StateCtx and Compute Pattern

The application uses a reactive state management system with `StateCtx` for state and `Compute` for derived/async values.

### Updating Computes

Computes **MUST** only be updated through the Command pattern using `Updater::set()`. Never mutate computes directly.

**Why:**
- Ensures state changes are trackable and predictable
- Allows async operations to safely update state
- Maintains separation between state reading and writing

**Do:**
```rust
// Create a Command to update the compute
#[derive(Default, Debug)]
pub struct ToggleMyFeatureCommand;

impl Command for ToggleMyFeatureCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let current = deps.get_compute_ref::<MyCompute>();
        updater.set(MyCompute {
            // ... update fields
            my_flag: !current.my_flag,
        });
    }
}

// Dispatch the command from UI code
ctx.dispatch::<ToggleMyFeatureCommand>();
```

**Don't:**
```rust
// ❌ Never mutate computes directly
let compute = ctx.get_compute_mut::<MyCompute>();
compute.my_flag = !compute.my_flag;
```

### Reading State in UI

Use `cached::<T>()` to read compute values in UI code:
```rust
let show_panel = self.state.ctx
    .cached::<MyCompute>()
    .map(|c| c.show_flag())
    .unwrap_or(false);
```

## Testing

- Run `just ui::test` for UI tests (with all features enabled, includes internal features)
- Run `just ui::test-non-internal` for non-internal tests only
- Run `just ui::test-all` for complete coverage (both test configurations)
- Ensure all tests pass before creating a PR

### How UI Tests Work

The UI testing system uses `kittest` and `egui_kittest` for testing egui-based widgets and applications. Tests are organized into:

1. **Unit Tests**: Located in `#[cfg(test)]` modules within source files (e.g., `ui/src/widgets/api_status.rs`)
2. **Integration Tests**: Located in `ui/tests/` directory

#### Test Infrastructure

**Key Dependencies (from `ui/Cargo.toml`):**
```toml
[dev-dependencies]
kittest = "0.3"
wiremock = "0.6"
egui_kittest = { version = "0.33", features = ["snapshot", "eframe"] }
tokio = { workspace = true, features = ["full", "test-util"] }
```

**Test Context (`ui/tests/common/mod.rs`):**
- `TestCtx<'a, State>` - For testing individual widgets with a mock server
- `TestCtx<'a, CollectsApp>` - For testing the full application

```rust
// Widget test example
let mut ctx = TestCtx::new(|ui, state| {
    my_widget(&state.ctx, ui);
}).await;

// Full app test example
let mut ctx = TestCtx::new_app().await;
let harness = ctx.harness_mut();
harness.step();
```

### Environment-Specific Tests

The UI supports multiple environments with different features enabled via Cargo feature flags. This affects which tests run and what code paths are tested.

#### Available Environment Features

| Feature | Description | Use Case |
|---------|-------------|----------|
| `env_test` | Test environment | Testing with test backend |
| `env_test_internal` | Test-internal environment | Admin features testing |
| `env_internal` | Internal environment | Admin features in production |
| `env_nightly` | Nightly builds | Nightly release testing |
| `env_pr` | PR preview environment | Pull request previews |
| (none) | Production | Default production build |

#### Running Tests for Different Environments

**Run all tests with all features (default command):**
```bash
# Runs tests with all features enabled
just ui::test

# Or directly with cargo
RUST_LOG=DEBUG cargo test --all-features
```

**Run tests for normal (non-internal) environment:**
```bash
# Run tests without internal features
cd ui && cargo test

# Or with specific test environment feature
cd ui && cargo test --features env_test
```

**Run tests for internal environment:**
```bash
# Run tests with internal features enabled
cd ui && cargo test --features env_test_internal

# Or with env_internal feature
cd ui && cargo test --features env_internal
```

**Run specific test file:**
```bash
# Run a specific integration test
cd ui && cargo test --test api_status_integration --all-features

# Run a specific test function
cd ui && cargo test test_api_status_with_200 --all-features
```

#### Conditional Compilation in Tests

Tests use `#[cfg]` attributes to conditionally compile based on environment features:

**Tests only for internal environments:**
```rust
// This entire test file only compiles when internal features are enabled
#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

#[tokio::test]
async fn test_internal_user_management() {
    // Test internal-only features like user management
}
```

**Tests only for normal (non-internal) environments:**
```rust
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
#[tokio::test]
async fn test_normal_login_flow() {
    // Test login flow that only exists in non-internal builds
}
```

**Conditional test setup:**
```rust
async fn setup_test_state_with_status(status_code: u16) -> (MockServer, State) {
    let mock_server = MockServer::start().await;

    // Base mock for all environments
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(status_code))
        .mount(&mock_server)
        .await;

    // Additional mock only for internal environments
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": []
        })))
        .mount(&mock_server)
        .await;

    let state = State::test(mock_server.uri());
    (mock_server, state)
}
```

#### Test File Organization by Environment

| Test File | Environment | Description |
|-----------|-------------|-------------|
| `api_status_integration.rs` | All | API status widget tests |
| `api_status_interval_test.rs` | Mixed | API status interval tests (some internal-only) |
| `api_status_toggle_test.rs` | All | API status toggle functionality |
| `login_integration.rs` | Mixed | Has both internal and non-internal tests |
| `user_management_integration.rs` | Internal only | Admin user management tests |
| `create_user_integration.rs` | Internal only | User creation tests |
| `otp_time_remaining_test.rs` | Internal only | OTP time remaining widget tests |
| `image_paste_integration.rs` | Non-internal only | Image paste functionality |

#### CI/CD Test Execution

In CI (`.github/workflows/ci.yml`), tests run **both** code paths to ensure complete coverage:

```yaml
- name: Run Non-Internal Tests
  working-directory: ui
  run: cargo test

- name: Run Internal Tests (all features)
  working-directory: ui
  run: cargo test --all-features
```

This ensures:
- **Non-internal tests** run first (`cargo test`) — tests code paths gated by `#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]`
- **Internal tests** run second (`cargo test --all-features`) — tests code paths gated by `#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]`

**Why both are needed:**
- `--all-features` enables internal features, so `#[cfg(not(...))]` tests are **excluded**
- Running without features enables non-internal code paths, so `#[cfg(any(...))]` tests are **excluded**
- Together, both runs provide complete test coverage

### Testing Requirements

#### 1. Unit Tests for New API and Widgets

Every new API endpoint and UI widget **MUST** have its own unit tests.

**For UI Widgets:**
- Add a `#[cfg(test)]` module at the bottom of the widget source file
- Use `kittest` and `egui_kittest` for UI testing
- Test the widget's rendering and interaction behavior
- Use helper functions for common patterns (e.g., tooltip triggering)

Example structure for widget unit tests:
```rust
// In src/widgets/my_widget.rs

pub fn my_widget(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    // Widget implementation
}

#[cfg(test)]
mod my_widget_test {
    use kittest::Queryable;
    use crate::test_utils::TestCtx;

    #[tokio::test]
    async fn test_my_widget_renders() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::my_widget(&state.ctx, ui);
        }).await;
        
        let harness = ctx.harness_mut();
        harness.step();
        
        // Assert widget renders correctly using kittest queries
        assert!(harness.query_by_label_contains("expected_label").is_some());
    }
}
```

**For API Endpoints (services):**
- Test request/response handling
- Test error cases (4xx, 5xx responses)
- Use `MockUserStorage` or similar mocks for dependencies
- Test authentication and authorization if applicable

#### 2. Integration Tests for New Features

Every new feature **MUST** have full integration tests in the `tests/` directory.

**For UI Features:**
- Place integration tests in `ui/tests/`
- Use the `TestCtx` helper from `tests/common/mod.rs`
- Test the complete feature flow including API interactions
- Use `wiremock` to mock backend responses

Example integration test structure:
```rust
// In ui/tests/my_feature_integration.rs
use crate::common::TestCtx;
use kittest::Queryable;

mod common;

#[tokio::test]
async fn test_my_feature_complete_flow() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();
    
    // Test the complete user flow
    harness.step();
    // ... assertions
}

#[tokio::test]
async fn test_my_feature_with_error() {
    let mut ctx = TestCtx::new_app_with_status(500).await;
    // ... test error handling
}
```

**For Service Features:**
- Place integration tests in `services/tests/`
- Test the complete request/response cycle
- Test with various authentication scenarios (Zero Trust, etc.)
- Test database interactions if applicable

#### 3. UI Testing Best Practices

**Use kittest queries to validate UI state, not internal StateCtx:**

Integration tests should verify what the user sees, not internal application state. Use `harness.query_by_label_contains()` to check if UI elements are visible.

**Do:**
```rust
// ✅ Query UI elements directly
fn is_panel_visible(harness: &egui_kittest::Harness<'_, MyApp>) -> bool {
    harness.query_by_label_contains("Panel Title").is_some()
}

assert!(is_panel_visible(harness), "Panel should be visible");
```

**Don't:**
```rust
// ❌ Don't check internal state in integration tests
let show_panel = harness.state().ctx.cached::<MyCompute>()
    .map(|c| c.show_flag())
    .unwrap_or(false);
assert!(show_panel);
```

**Use harness.key_press() to simulate user input:**

```rust
// Simulate F1 key press to toggle a feature
harness.key_press(egui::Key::F1);
harness.step();  // Process the key event
harness.step();  // Let UI update

// Verify the UI changed
assert!(harness.query_by_label_contains("Feature Panel").is_some());
```

**Wait for async operations in tests:**

If your test involves async API calls, ensure they complete before asserting:
```rust
// Run several frames to let initial API fetch complete
for _ in 0..10 {
    harness.step();
}
// Wait for async operations
tokio::time::sleep(std::time::Duration::from_millis(200)).await;
for _ in 0..5 {
    harness.step();
}

// Now test the feature
harness.key_press(egui::Key::F1);
harness.step();
```

**Add accessible labels to UI elements for testing:**

When creating UI panels or widgets that need to be tested, include labels that kittest can query:
```rust
// In your UI code
if show_api_status {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        ui.label("API Status");  // This label makes the panel queryable
        // ... rest of panel content
    });
}
```

#### 4. File Organization Guidelines

Split large files into smaller, focused files following these guidelines:

**For Public Functions (`pub fn`):**
- Place in dedicated module files under appropriate directories
- Keep related functions together (e.g., all user-related widgets in `widgets/users/`)
- Export through `mod.rs` files

**For Unit Tests:**
- Keep unit tests in the same file as the code they test using `#[cfg(test)]` modules
- Name test modules descriptively (e.g., `mod my_widget_test`)
- Use helper functions to reduce test code duplication

**For Integration Tests:**
- Place in the `tests/` directory of the relevant crate (`ui/tests/`, `services/tests/`)
- Name files with `_integration` or `_test` suffix (e.g., `api_status_integration.rs`)
- Share common test utilities through `tests/common/mod.rs`

**File Size Guidelines:**
- If a source file exceeds ~300 lines, consider splitting it
- Group related functionality into sub-modules
- Example: `widgets/internal/` contains `mod.rs` and `users/` subdirectory

**Directory Structure Example:**
```
ui/
├── src/
│   ├── widgets/
│   │   ├── mod.rs              # Re-exports all widgets
│   │   ├── api_status.rs       # Widget + unit tests
│   │   ├── signin_button.rs    # Widget + unit tests
│   │   └── internal/
│   │       ├── mod.rs          # Internal widgets re-exports
│   │       └── users/          # User management widgets
│   └── ...
└── tests/
    ├── common/
    │   └── mod.rs              # Shared test utilities
    ├── api_status_integration.rs
    └── user_management_integration.rs
```

## Version Display Format

Both UI and services **MUST** use a consistent version display format: `{env}:{info}`

### Environment-Specific Formats

| Environment | Format | Example |
|-------------|--------|---------|
| PR | `pr:{number}` | `pr:123` |
| Nightly | `nightly:{date}` | `nightly:2026-01-03` |
| Internal | `internal:{commit}` | `internal:abc1234` |
| Test-Internal | `test-internal:{commit}` | `test-internal:abc1234` |
| Test/Main | `main:{commit}` | `main:abc1234` |
| Production | `stable:{version}` | `stable:2026.1.2` |

### Implementation Details

- **UI**: Uses `collects_business::version_info::format_env_version()` function
- **Services**: Uses `format_version_header()` function in `services/src/lib.rs`
- Both rely on build-time environment variables:
  - `BUILD_COMMIT`: Short git commit hash
  - `BUILD_DATE`: Build timestamp (RFC3339 format)
  - `CARGO_PKG_VERSION`: Package version from Cargo.toml
  - `PR_NUMBER`: PR number (only for PR builds)
  - `SERVICE_ENV`: Environment name (services only)

### Where Version is Displayed

- **UI**: Shows in API status tooltip as "UI: {env}:{info}"
- **Services**: Returns in `x-service-version` HTTP header

## Release Methods

This project has automated release pipelines for both the UI and services.

### UI Release (Collects App)

The UI app is released automatically when the version in `ui/Cargo.toml` changes.

**Version Format:**
- Use date-based versioning: `YYYY.M.D` (e.g., `2026.1.1`)
- Update the `version` field in `ui/Cargo.toml`

**Release Triggers:**
1. **Production Release**: Merge to `main` with version change in `ui/Cargo.toml`
   - Creates a GitHub release with tag `v<version>`
   - Deploys to production Cloudflare Worker
   - Deploys to internal environment
2. **Nightly Release**: Scheduled daily at midnight UTC
   - Creates/updates `nightly` pre-release tag
3. **PR Preview**: On pull requests
   - Builds artifacts and uploads to PR
   - Deploys preview to PR-specific Cloudflare Worker

**Build Outputs:**
- `Collects-linux-x86_64` - Linux native binary
- `Collects-windows-x86_64.exe` - Windows native binary
- `Collects-macos-aarch64` - macOS Apple Silicon binary
- WASM build deployed to Cloudflare Workers

**Manual Build Commands:**
```bash
# Build native release
just ui::release

# Package native binary
just ui::package-native <output_name> [features]

# Build and deploy web version
just ui::wk-deploy [env]
```

### Services Release

The services (backend API) are deployed to Google Cloud Run.

**Deployment Environments:**
- `prod` - Production environment
- `internal` - Internal testing
- `nightly` - Nightly builds
- `test` - Test environment
- `test-internal` - Internal test environment
- `pr` - Pull request previews
- `local` - Local development

**Manual Deployment Commands:**
```bash
# Build release binary (static linked for Docker)
just services::release

# Build and push Docker image
just docker-push <image_tag>

# Deploy to Cloud Run
just services::gcloud-deploy <env> <image_tag>
```

**Database Migrations:**
- Run `just services::migrate <env>` to apply migrations
- Run `just services::prepare <env>` to update SQLx offline cache
- Always commit `.sqlx/` directory changes

## Useful Commands

```bash
# Install dependencies
just install-deps

# Check formatting
just check-fmt

# Check linting
just check-lint

# Check typos
just check-typos

# Validate PR title
just scripts::check-pr-title "feat: your feature description"
```
