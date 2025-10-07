use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::Config;

/// Initialize a PostgreSQL connection pool
pub async fn create_pool(config: &Config) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().connect(&config.database_url()).await?;

    tracing::info!("Database connection pool established");

    Ok(pool)
}
