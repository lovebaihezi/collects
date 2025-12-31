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

## Testing

- Run `just ui::test` for UI tests
- Ensure all tests pass before creating a PR

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
        
        // Assert widget renders correctly
        assert!(harness.query_by_label("expected_label").is_some());
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

#### 3. File Organization Guidelines

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
