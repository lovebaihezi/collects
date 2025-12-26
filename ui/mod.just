run:
    RUST_LOG=info cargo run

build:
    cargo build

release:
    cargo build --release

test log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test

lint:
    cargo clippy
    trunk fmt --check

[parallel]
install:
    cargo install trunk
    pnpm i

web-build env="": install
    {{ if env == "pr" { "trunk build --features env_pr" } else if env == "test" { "trunk build --features env_test" } else if env == "internal" { "trunk build --features env_internal" } else if env == "nightly" { "trunk build --features env_nightly" } else { "trunk build" } }}
    du -h dist/*

web-fmt: install
    pnpm prettier --write src/worker.ts index.html

web-release env="": install
    {{ if env == "pr" { "trunk build --release --features env_pr" } else if env == "test" { "trunk build --release --features env_test" } else if env == "internal" { "trunk build --release --features env_internal" } else if env == "nightly" { "trunk build --release --features env_nightly" } else { "trunk build --release" } }}
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
