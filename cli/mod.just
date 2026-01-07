build:
    cargo build -p collects-cli

release:
    cargo build --release -p collects-cli

test log_level="DEBUG":
    RUST_LOG={{log_level}} cargo test -p collects-cli

lint:
    cargo clippy -p collects-cli

# Install the CLI to $CARGO_HOME/bin
install:
    cargo install --path .

# Run the CLI with arguments
run *args:
    cargo run -p collects-cli -- {{args}}
