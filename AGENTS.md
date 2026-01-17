# Repository Guidelines

## Project Structure
- `services/`: Rust backend (Axum) API, DB migrations (`services/migrations/`), and service docs (`services/docs/`). Integration tests live in `services/tests/`.
- `cli/`: Rust CLI (`collects`) and command implementations (`cli/src/`), with tests in `cli/tests/`.
- `ui/`: Rust UI + web assets (Trunk/pnpm) and Cloudflare Workers artifacts/config.
- `scripts/`: Bun/TypeScript automation for deployments, secrets, CI helpers (notably `scripts/services/`).
- `assets/`, `docs/`, `rules/`, `rule-tests/`: shared assets, documentation, and ast-grep rule definitions/tests.

## Build, Test, and Development Commands
Use `just` from the repo root:
- `just check-fmt` / `just check-lint` / `just check-typos`: workspace formatting, lint, and typo checks.
- `just install-hooks`: install git hooks via `lefthook` (recommended before pushing).
- `just services::dev`: run the API locally (requires GCP secrets; see “Configuration”).
- `just cli::run test -- new --title "Hello"`: run the CLI with an environment feature flag.
- `just ui::run test` or `just ui::web-build test`: run/build the UI (native or web).

## Scripts Command Index (Short)
- `just scripts::help`: list all script commands
- `just scripts::actions-setup` / `just scripts::actions-migrate-repo`
- `just scripts::init-db <NEON_API_TOKEN> <NEON_PROJECT_ID>`
- `just scripts::r2-setup` / `just scripts::r2-list` / `just scripts::r2-verify`
- `just scripts::gcloud-deploy <env> <tag>`

## Coding Style & Naming
- Rust: formatted with `cargo fmt`; linted with `cargo clippy -- -D warnings` (see `just check-*`).
- TypeScript: formatted with Prettier via `scripts/` (`just scripts::check-fmt`) and UI web assets via `ui/mod.just`.
- Naming: Rust uses `snake_case` (fns/modules) and `CamelCase` (types); TS uses `camelCase` and `PascalCase`.
- Prefer clear module names/scopes (`services`, `cli`, `ui`, `scripts`) consistent with the repo layout.

## Testing Guidelines
- Services: `cargo test -p collects-services` (integration tests are in `services/tests/`).
- CLI: `cargo test -p collects-cli`.
- UI: `just ui::test` (runs both non-internal and `--all-features` paths).

## Commit & Pull Request Guidelines
- Use Conventional Commits (seen in history): `feat(scope): ...`, `fix(scope): ...`, `refactor(scope): ...`, `chore: ...`, `ci: ...`.
- PRs should include: a short description, testing notes (commands + results), and migration notes. If you add migrations, update and commit the SQLx cache (`just services::prepare <env>`).

## Security & Configuration
- Do not commit secrets. Deployments use GCP Secret Manager + Cloud Run.
- R2 is required in all environments. Ensure secrets exist: `cf-account-id`, `cf-access-key-id`, `cf-secret-access-key`, `cf-bucket` (watch for legacy typo `cf-secret-acess-key`).
- Setup/deploy helpers: `just scripts::r2-setup --project-id <id>` and `just services::gcloud-deploy <env> <tag>`.
