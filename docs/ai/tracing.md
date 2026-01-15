# Tracing and Timing (CLI Profiling)

This document describes how to use tracing for timing and profiling in the CLI.

---

## Overview

The CLI uses the `tracing` crate with automatic span timing via `FmtSpan::CLOSE`. Functions annotated with `#[instrument]` automatically have their execution time logged when the span closes.

---

## Quick start

### Enable timing output

```sh
# Show timing for any command
collects list --timing

# Show timing with verbose debug output
collects list --timing -v

# All subcommands support timing
collects login --timing
collects create -I --timing
collects view <id> --timing
```

### Example output

With `--timing`:
```
INFO list:restore_session:flush:await_tasks: close time.busy=40.1µs time.idle=1.07s
INFO list:restore_session:flush: close time.busy=64.0µs time.idle=1.07s
INFO list:restore_session: close time.busy=84.5µs time.idle=1.07s
```

With `--timing -v` (verbose):
```
DEBUG reqwest::connect: starting new connection: https://collects.lqxclqxc.com/
DEBUG hyper_util::client::legacy::connect::http: connecting to [fdfe:dcba:9876::233]:443
DEBUG hyper_util::client::legacy::connect::http: connected to [fdfe:dcba:9876::233]:443
...
INFO list:restore_session: close time.busy=84.5µs time.idle=1.07s
```

---

## Understanding the output

### Span hierarchy

Spans are hierarchical, showing parent-child relationships:
```
list:restore_session:flush:await_tasks
│    │               │     └── await_tasks (innermost)
│    │               └── flush
│    └── restore_session
└── list (outermost command)
```

### Timing fields

- **`time.busy`** — CPU time actually spent executing code in the span
- **`time.idle`** — Time waiting (network I/O, sleep, etc.)

Example interpretation:
```
time.busy=84.5µs time.idle=1.07s
```
- 84.5µs of actual Rust code execution
- 1.07s waiting for network response

---

## Adding tracing to new code

### 1. Add `#[instrument]` to functions

```rust
use tracing::instrument;

#[instrument(skip_all, name = "my_operation")]
async fn my_operation(ctx: &mut StateCtx) -> Result<()> {
    // ... operation code
}
```

### 2. Add context fields

```rust
#[instrument(skip_all, name = "view", fields(content_id = id.as_deref().unwrap_or("interactive")))]
async fn run_view(mut ctx: StateCtx, id: Option<String>, download: bool) -> Result<()> {
    // ...
}
```

### 3. Skip large arguments

Always use `skip_all` or `skip(ctx, ...)` for large structs to avoid performance overhead:

```rust
// Good: skip all arguments, add only relevant fields
#[instrument(skip_all, name = "create", fields(file_count = files.len()))]
async fn run_create(ctx: StateCtx, files: Vec<PathBuf>, title: Option<String>) -> Result<()> {
    // ...
}

// Bad: logs entire StateCtx which is expensive
#[instrument]
async fn run_create(ctx: StateCtx, files: Vec<PathBuf>, title: Option<String>) -> Result<()> {
    // ...
}
```

---

## Initialization

The timing system is initialized in `cli/src/timing.rs`:

```rust
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
};

pub fn init_tracing(verbose: bool, timing: bool) {
    let filter = if verbose {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy()
    } else {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::WARN.into())
            .from_env_lossy()
    };

    // FmtSpan::CLOSE logs duration when span closes
    let span_events = if timing {
        FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_target(verbose)
                .with_level(true)
                .with_span_events(span_events)
                .with_writer(std::io::stderr),
        )
        .with(filter)
        .init();
}
```

---

## Environment variable override

You can override the log filter via `RUST_LOG`:

```sh
# Show only collects crate logs at debug level
RUST_LOG=collects=debug collects list --timing

# Show all HTTP client activity
RUST_LOG=reqwest=debug,hyper=debug collects list --timing -v

# Show everything
RUST_LOG=trace collects list --timing -v
```

---

## Best practices

### DO

- Use `#[instrument(skip_all)]` on async command handlers
- Add meaningful `name` to distinguish similar functions
- Add `fields(...)` for key parameters that help debugging
- Use `--timing` for end-user latency visibility
- Use `--timing -v` for detailed debugging

### DON'T

- Don't instrument hot loops or frequently called functions
- Don't log large structs (use `skip_all` or `skip(...)`)
- Don't initialize tracing multiple times (it will panic)

---

## Typical workflow for debugging latency

1. Run command with `--timing` to see high-level breakdown
2. If latency is in network, run with `--timing -v` to see HTTP details
3. Look at `time.idle` vs `time.busy` to distinguish I/O wait from CPU work
4. Add more `#[instrument]` calls to narrow down slow code paths

---

## Related docs

- `state-model.md` — StateCtx / Command architecture (commands are where network calls happen)
- `testing.md` — How to test CLI commands