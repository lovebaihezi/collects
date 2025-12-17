use crate::config::Config;
use anyhow::Context;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use std::env;
use tracing_stackdriver::CloudTraceConfiguration;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing(config: &Config) -> anyhow::Result<()> {
    if config.is_local() {
        // Local development: Pretty printing
        tracing_subscriber::registry()
            .with(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("info,collects_services=debug")),
            )
            .with(tracing_subscriber::fmt::layer())
            .init();
    } else {
        // Production: JSON logging with Stackdriver & Cloud Trace
        let project_id = env::var("GOOGLE_CLOUD_PROJECT")
            .context("GOOGLE_CLOUD_PROJECT environment variable is required in production")?;

        // Set the global propagator to trace-context (W3C)
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

        let stackdriver_layer = tracing_stackdriver::layer()
            .with_cloud_trace(CloudTraceConfiguration { project_id });

        // Let type inference handle the subscriber type instead of forcing Registry
        let otel_layer = tracing_opentelemetry::layer();

        tracing_subscriber::registry()
            .with(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("info,collects_services=debug")),
            )
            .with(otel_layer)
            .with(stackdriver_layer)
            .init();
    }

    Ok(())
}
