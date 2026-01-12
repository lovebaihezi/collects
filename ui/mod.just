# Run the UI application with optional environment feature flags
# Usage: just ui::run [env]
run env="":
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    RUST_LOG=info cargo run $FEATURES

# Build the UI in debug mode
build:
    cargo build

# Build the UI in release mode
release:
    cargo build --release

# Run both test configurations to ensure all code paths are covered (default)
test log_level="DEBUG":
    @echo "Running non-internal (normal) tests..."
    RUST_LOG={{log_level}} cargo test
    @echo ""
    @echo "Running tests with all features (includes internal)..."
    RUST_LOG={{log_level}} cargo test --all-features

# Run tests without internal features only (non-internal code path)
test-non-internal log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test

# Run tests with all features enabled only (includes internal features)
test-internal log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test --all-features

# Run cargo clippy lints for the UI crate
lint:
    cargo clippy
    trunk fmt --check

# Install UI dependencies (trunk and pnpm packages)
[parallel]
install:
    cargo install trunk
    pnpm i

# Build web assets in debug mode with optional environment feature flags
# Usage: just ui::web-build [env]
web-build env="": install
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    trunk build $FEATURES
    du -h dist/*

# Format web assets (TypeScript and HTML)
web-fmt: install
    pnpm prettier --write src/worker.ts index.html

# Build web assets in release mode with optional environment feature flags
# Usage: just ui::web-release [env]
web-release env="": install
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    trunk build --release $FEATURES
    du -h dist/*

# Run wrangler development server with debug web build
wk-dev: (web-build)
    pnpm wrangler dev

# Build wrangler deployment with optional environment config
# Usage: just ui::wk-build [env]
wk-build env="": install (web-release env)
    #!/bin/bash
    set -e
    echo "build for env: {{env}}"
    pnpm wrangler build {{ if env != "" { "--config wrangler." + env + ".toml" } else { "" } }}

# Deploy to Cloudflare Workers with optional environment config
# Usage: just ui::wk-deploy [env]
wk-deploy env="": (wk-build env)
    echo {{env}}
    pnpm wrangler deploy {{ if env != "" { "--config wrangler." + env + ".toml" } else { "" } }}

# Generate TypeScript types for Cloudflare Workers
wk-type: install
    pnpm wrangler types

# Build native executable and rename for release packaging
# Usage: just ui::package-native <output_name> [features]
[unix]
package-native output_name features="":
    # Build the crate. Explicit target-dir ensures artifacts are in ui/target/release
    cargo build --release --target-dir target -p collects-ui {{ if features != "" { "--features " + features } else { "" } }}

    # Handle renaming
    mv target/release/collects-ui target/release/{{output_name}}

# Build native executable and rename for release packaging (Windows)
# Usage: just ui::package-native <output_name> [features]
[windows]
package-native output_name features="":
    #!pwsh
    # Build the crate. Explicit target-dir ensures artifacts are in ui/target/release
    cargo build --release --target-dir target -p collects-ui {{ if features != "" { "--features " + features } else { "" } }}

    # Handle renaming
    Move-Item -Path "target/release/collects-ui.exe" -Destination "target/release/{{output_name}}" -Force
