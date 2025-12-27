use collects_services::{
    config::Config,
    database::{self, PgStorage},
    routes, telemetry,
    users::PgUserStorage,
};
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
    let config: Config = Config::init().map_err(|e| {
        eprintln!("Failed to initialize configuration from environment:");
        eprintln!("Error: {:?}", e);
        eprintln!("\nEnvironment variables:");
        for (key, value) in std::env::vars() {
            eprintln!("  {}={:?}", key, value);
        }
        e
    })?;

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

    // Create socket address
    let addr = SocketAddr::from((config.server_addr().parse::<IpAddr>()?, config.port()));

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Initialize database connection pool
    let pool = database::create_pool(&config).await?;
    let sql_storage = PgStorage::new(pool);

    // Create user storage backed by PostgreSQL
    let user_storage = PgUserStorage::new(sql_storage.clone());

    // Build the application router with both SQL and User storage
    let route = routes(sql_storage, user_storage, config.clone()).await;

    info!("Starting server on {}", addr);
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
