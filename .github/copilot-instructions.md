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
