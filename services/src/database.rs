use async_trait::async_trait;
use sqlx::postgres::{PgPool, PgPoolOptions};

use crate::config::Config;

/// Initialize a PostgreSQL connection pool
pub async fn create_pool(config: &Config) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().connect(config.database_url()).await?;

    tracing::info!("Database connection pool established");

    Ok(pool)
}

#[async_trait]
pub trait SqlStorage: Clone + Send + Sync + 'static {
    async fn is_connected(&self) -> bool;
}

#[derive(Clone)]
pub struct PgStorage {
    pub pool: PgPool,
}

impl PgStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SqlStorage for PgStorage {
    async fn is_connected(&self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .is_ok()
    }
}
