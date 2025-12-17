use collects_services::{config::Config, database, routes, telemetry};
use std::net::{IpAddr, SocketAddr};
use tracing::info;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const BUILD_DATE: &str = env!("BUILD_DATE");
const BUILD_COMMIT: &str = env!("BUILD_COMMIT");
const BUILD_BRANCH: &str = env!("BUILD_BRANCH");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration first to determine environment for tracing
    let config: Config = Config::init()?;

    // Initialize tracing
    telemetry::init_tracing(&config)?;

    // Print build information
    print_build_info();

    info!(
        environment = %config.environment(),
        server_addr = %config.server_addr(),
        port = %config.port(),
        "Configuration loaded"
    );

    // Initialize database connection pool
    let pool = database::create_pool(&config).await?;

    // Build the application router
    let route = routes(pool, config.clone()).await;

    // Create socket address
    let addr = SocketAddr::from((config.server_addr().parse::<IpAddr>()?, config.port()));

    info!("Starting server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, route).await?;

    Ok(())
}

/// Print build information
fn print_build_info() {
    info!("===========================================");
    info!("  Collects Services");
    info!("===========================================");
    info!("Build Date:   {}", BUILD_DATE);
    info!("Build Commit: {}", BUILD_COMMIT);
    info!("Build Branch: {}", BUILD_BRANCH);
    info!("===========================================");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_info_constants_exist() {
        // Verify build info constants are available
        assert!(!BUILD_DATE.is_empty());
        assert!(!BUILD_COMMIT.is_empty());
        assert!(!BUILD_BRANCH.is_empty());
    }
}
