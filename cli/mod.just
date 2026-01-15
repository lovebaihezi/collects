# Show available CLI commands
default:
    @echo ""
    @echo "╔══════════════════════════════════════════════════════════════════════════════╗"
    @echo "║                          COLLECTS CLI - COMMANDS                             ║"
    @echo "╚══════════════════════════════════════════════════════════════════════════════╝"
    @echo ""
    @echo "   Build & Run:"
    @echo "   $ just cli::build              Build in debug mode"
    @echo "   $ just cli::release            Build in release mode"
    @echo "   $ just cli::run [env]          Run with optional environment"
    @echo ""
    @echo "   Installation:"
    @echo "   $ just cli::install [env]      Install binary to ~/.cargo/bin"
    @echo ""
    @echo "   Shell Completions:"
    @echo "   $ just cli::completions <shell>   Generate shell completions"
    @echo "   $ just cli::install-completions   Install fish completions"
    @echo ""
    @echo "   Supported shells: fish, bash, zsh, elvish, powershell"
    @echo ""
    @echo "   Environments: test, nightly, internal (default: test)"
    @echo ""

# Build CLI in debug mode
build:
    cargo build -p collects-cli

# Build CLI in release mode
release:
    cargo build --release -p collects-cli

# Run CLI with optional environment feature flags
# Usage: just cli::run [env] [args...]
run env="" *args="":
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    cargo run --bin collects $FEATURES -- {{args}}

# Install CLI binary to ~/.cargo/bin with optional environment
# Usage: just cli::install [env]
install env="":
    #!/bin/bash
    set -e
    FEATURES=$(just scripts::env-feature {{env}})
    cargo install --path . $FEATURES

# Generate shell completions
# Usage: just cli::completions <shell>
# Shells: fish, bash, zsh, elvish, powershell
completions shell:
    #!/bin/bash
    set -e
    cargo run --bin collects --quiet -- completions {{shell}}

# Install fish completions to the standard fish completions directory
install-completions:
    #!/bin/bash
    set -e
    FISH_COMPLETIONS_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions"
    mkdir -p "$FISH_COMPLETIONS_DIR"
    cargo run --bin collects --quiet -- completions fish > "$FISH_COMPLETIONS_DIR/collects.fish"
    echo "Fish completions installed to $FISH_COMPLETIONS_DIR/collects.fish"

# Run cargo clippy lints for CLI
lint:
    cargo clippy --bin collects

# Format CLI code
fmt:
    cargo fmt -p collects-cli

# Run tests for CLI
test:
    cargo test -p collects-cli
