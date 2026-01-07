# Commits and PR Titles (Conventional Commits)

This repository requires **Conventional Commits** for:
- all commit messages
- all PR titles

Keep messages short, imperative, and consistent.

---

## Commit message format

```/dev/null/conventional-commits.txt#L1-5
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### Types (allowed)

Use one of:

- `feat` — new feature
- `fix` — bug fix
- `docs` — documentation only
- `style` — formatting only (no behavior change)
- `refactor` — code change without feature/bug fix
- `perf` — performance improvement
- `test` — tests only
- `build` — build system / dependency changes
- `ci` — CI configuration changes
- `chore` — maintenance (not src/test behavior)
- `revert` — revert a previous commit

### Scope (optional)

Use `(<scope>)` to indicate the area affected.

Suggested scopes (examples, not exhaustive):
- `ui`
- `business`
- `states`
- `services`
- `scripts`
- `ci`
- `docs`

Pick a scope that helps reviewers locate the change quickly.

### Description (required)

Rules:
- imperative mood (“add”, “fix”, “change”, “remove”)
- lowercase start preferred
- no trailing period
- keep it specific (what changed, not why)

---

## Examples

```/dev/null/commit-examples.txt#L1-12
feat: add user authentication
fix(ui): resolve button alignment issue
docs: update README with installation instructions
refactor(api): simplify error handling logic
test(ui): add coverage for login flow
build: bump dependencies
ci: add PR title validation
chore: update tooling
```

---

## Breaking changes

If a change is breaking, use one of:
- `!` after type/scope
- a `BREAKING CHANGE:` footer

Examples:

```/dev/null/breaking-change-examples.txt#L1-11
feat!: change config file format

feat(ui)!: remove legacy login flow

refactor: rename API fields

BREAKING CHANGE: clients must send the new header "x-example"
```

Use breaking changes sparingly and only when necessary.

---

## PR title rules

Your PR title must follow the **same Conventional Commits format** as commits:

```/dev/null/pr-title-format.txt#L1-1
<type>[optional scope]: <description>
```

Keep the PR title aligned with the primary change in the PR.

---

## Validation

PR titles are validated in CI. You can validate locally with:

```/dev/null/pr-title-validation.txt#L1-1
just scripts::check-pr-title "<type>[optional scope]: <description>"
```

If validation fails:
- fix the PR title first
- then ensure commits also follow the same convention

---

## Practical guidance

- Prefer one clear PR title over trying to describe every change.
- If your PR contains multiple commits, **each commit** must still follow the convention.
- Avoid vague descriptions like “update stuff” or “fix bug”. Write what changed.