//! CLI timing and latency profiling utilities.
//!
//! Uses `tracing` spans with automatic duration tracking via `FmtSpan::CLOSE`.
//! Functions annotated with `#[instrument]` will automatically have their
//! execution time logged when the span closes.
//!
//! # Usage
//!
//! Initialize tracing with timing enabled:
//! ```ignore
//! use timing::init_tracing;
//! init_tracing(true, true); // verbose=true, timing=true
//! ```
//!
//! Then use `#[instrument]` on functions to automatically track timing:
//! ```ignore
//! #[tracing::instrument(skip_all)]
//! async fn my_operation() {
//!     // ... operation code
//! }
//! ```

use tracing::level_filters::LevelFilter;
use tracing_subscriber::{
    EnvFilter,
    fmt::{self, format::FmtSpan},
    prelude::*,
};

/// Initialize tracing subscriber with optional timing output.
///
/// # Arguments
/// * `verbose` - If true, enables debug-level logging
/// * `timing` - If true, logs span close events with duration
pub fn init_tracing(verbose: bool, timing: bool) {
    let filter = if verbose {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::DEBUG.into())
            .from_env_lossy()
    } else if timing {
        // Span close events are logged at INFO level, so we need at least INFO
        EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy()
    } else {
        EnvFilter::builder()
            .with_default_directive(LevelFilter::WARN.into())
            .from_env_lossy()
    };

    // Configure span events based on timing flag
    let span_events = if timing {
        FmtSpan::CLOSE // Log duration when span closes
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

#[cfg(test)]
mod tests {
    // Note: tracing subscriber can only be initialized once per process,
    // so we keep tests minimal here. The init_tracing function uses global
    // state that cannot be tested in isolation without affecting other tests.
}
