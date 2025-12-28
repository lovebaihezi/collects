run env="":
    RUST_LOG=info cargo run {{ if env == "pr" { "--features env_pr" } else if env == "test" { "--features env_test" } else if env == "test-internal" { "--features env_test_internal" } else if env == "internal" { "--features env_internal" } else if env == "nightly" { "--features env_nightly" } else { "" } }}

build:
    cargo build

release:
    cargo build --release

test log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test --all-features

lint:
    cargo clippy
    trunk fmt --check

[parallel]
install:
    cargo install trunk
    pnpm i

web-build env="": install
    trunk build {{ if env == "pr" { "--features env_pr" } else if env == "test" { "--features env_test" } else if env == "test-internal" { "--features env_test_internal" } else if env == "internal" { "--features env_internal" } else if env == "nightly" { "--features env_nightly" } else { "" } }}
    du -h dist/*

web-fmt: install
    pnpm prettier --write src/worker.ts index.html

web-release env="": install
    trunk build --release {{ if env == "pr" { "--features env_pr" } else if env == "test" { "--features env_test" } else if env == "test-internal" { "--features env_test_internal" } else if env == "internal" { "--features env_internal" } else if env == "nightly" { "--features env_nightly" } else { "" } }}
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
