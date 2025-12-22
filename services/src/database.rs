use sqlx::postgres::{PgPool, PgPoolOptions};
use std::future::Future;

use crate::config::Config;

/// Initialize a PostgreSQL connection pool
pub async fn create_pool(config: &Config) -> anyhow::Result<PgPool> {
    let pool = PgPoolOptions::new().connect(config.database_url()).await?;

    tracing::info!("Database connection pool established");

    Ok(pool)
}

pub trait PersistentStructureDataService: Clone + Send + Sync + 'static {
    fn is_connected(&self) -> impl Future<Output = bool> + Send;
}

#[derive(Clone)]
pub struct NeonService {
    pub pool: PgPool,
}

impl NeonService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

impl PersistentStructureDataService for NeonService {
    async fn is_connected(&self) -> bool {
        sqlx::query!("SELECT 1").execute(&self.pool).await.is_ok()
    }
}
