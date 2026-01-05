run env="":
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    RUST_LOG=info cargo run $FEATURES

build:
    cargo build

release:
    cargo build --release

# Run tests with all features enabled (includes internal features)
test log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test --all-features

# Run tests without internal features (non-internal code path)
test-non-internal log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test

# Run both test configurations to ensure all code paths are covered
test-all log_level="DEBUG":
    @echo "Running non-internal (normal) tests..."
    RUST_LOG={{log_level}} cargo test
    @echo ""
    @echo "Running tests with all features (includes internal)..."
    RUST_LOG={{log_level}} cargo test --all-features

# Run e2e tests for non-internal builds (login happy path)
# Requires E2E_TEST_USERNAME and E2E_TEST_OTP_SECRET environment variables
# for full login flow testing
e2e-test log_level="DEBUG":
    @echo "Running e2e tests for non-internal builds..."
    RUST_LOG={{log_level}} cargo test --test e2e_login_happy_path --features env_test -- --nocapture

# Run e2e tests for internal builds (user management)
e2e-test-internal log_level="DEBUG":
    @echo "Running e2e tests for internal builds..."
    RUST_LOG={{log_level}} cargo test --test e2e_internal_happy_path --features env_test_internal -- --nocapture

# Run all e2e tests (both normal and internal)
e2e-test-all log_level="DEBUG":
    @echo "Running e2e tests for non-internal builds..."
    RUST_LOG={{log_level}} cargo test --test e2e_login_happy_path --features env_test -- --nocapture
    @echo ""
    @echo "Running e2e tests for internal builds..."
    RUST_LOG={{log_level}} cargo test --test e2e_internal_happy_path --features env_test_internal -- --nocapture

lint:
    cargo clippy
    trunk fmt --check

[parallel]
install:
    cargo install trunk
    pnpm i

web-build env="": install
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    trunk build $FEATURES
    du -h dist/*

web-fmt: install
    pnpm prettier --write src/worker.ts index.html

web-release env="": install
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    trunk build --release $FEATURES
    du -h dist/*

wk-dev: (web-build)
    pnpm wrangler dev

wk-build env="": install (web-release env)
    @echo "build for env: {{env}}"
    pnpm wrangler build {{ if env != "" { "--config wrangler." + env + ".toml" } else { "" } }}

wk-deploy env="": (wk-build env)
    @echo {{env}}
    pnpm wrangler deploy {{ if env != "" { "--config wrangler." + env + ".toml" } else { "" } }}

wk-type: install
    pnpm wrangler types

# Unix implementation (Bash)
[unix]
package-native output_name features="":
    # Build the crate. Explicit target-dir ensures artifacts are in ui/target/release
    cargo build --release --target-dir target -p collects-ui {{ if features != "" { "--features " + features } else { "" } }}

    # Handle renaming
    mv target/release/collects-ui target/release/{{output_name}}

# Windows implementation (PowerShell)
[windows]
package-native output_name features="":
    #!pwsh
    # Build the crate. Explicit target-dir ensures artifacts are in ui/target/release
    cargo build --release --target-dir target -p collects-ui {{ if features != "" { "--features " + features } else { "" } }}

    # Handle renaming
    Move-Item -Path "target/release/collects-ui.exe" -Destination "target/release/{{output_name}}" -Force
