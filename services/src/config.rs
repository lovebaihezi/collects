use serde::Deserialize;
use std::env::vars;
use std::fmt::Display;
use tracing::info;

#[derive(Debug, Clone, Deserialize)]
pub enum Env {
    #[serde(rename = "local")]
    Local,
    #[serde(rename = "prod")]
    Prod,
}

impl Display for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Env::Local => write!(f, "local"),
            Env::Prod => write!(f, "prod"),
        }
    }
}

// The final, validated configuration struct.
// `server_addr` is guaranteed to be a valid string.
#[derive(Debug, Clone)]
pub struct Config {
    env: Env,
    database_url: String,
    server_addr: String,
    port: u16,
    clerk_frontend_api: String,
}

// An intermediate struct for deserializing environment variables
// where `server_addr` is optional.
#[derive(Deserialize)]
struct RawConfig {
    env: Env,
    database_url: String,
    server_addr: Option<String>,
    port: u16,
    clerk_frontend_api: String,
}

impl Config {
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            env: Env::Local,
            database_url: "postgres://localhost:5432/test".to_string(),
            server_addr: "127.0.0.1".to_string(),
            port: 8080,
            clerk_frontend_api: "clerk.test".to_string(),
        }
    }

    pub fn environment(&self) -> &Env {
        &self.env
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn clerk_frontend_api(&self) -> &str {
        &self.clerk_frontend_api
    }

    pub fn server_addr(&self) -> &str {
        &self.server_addr
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn is_local(&self) -> bool {
        matches!(self.env, Env::Local)
    }

    pub fn is_prod(&self) -> bool {
        matches!(self.env, Env::Prod)
    }

    /// Initializes configuration by reading from environment variables
    /// and applying environment-aware defaults.
    pub fn init() -> anyhow::Result<Self> {
        info!("Loading configuration from environment variables");

        // First, deserialize into a temporary struct that allows for optional fields
        let raw_config: RawConfig = serde_env::from_iter(vars())?;

        // Apply the default logic for `server_addr` based on the environment
        let server_addr = match raw_config.server_addr {
            Some(addr) => {
                info!("Using provided SERVER_ADDR: {}", addr);
                addr
            }
            None => {
                let default_addr = match raw_config.env {
                    Env::Prod => "0.0.0.0",
                    Env::Local => "127.0.0.1",
                };
                info!(
                    "SERVER_ADDR not set, defaulting to {} for {} environment",
                    default_addr, raw_config.env
                );
                default_addr.to_string()
            }
        };

        // Construct the final, validated Config struct
        Ok(Config {
            env: raw_config.env,
            database_url: raw_config.database_url,
            port: raw_config.port,
            server_addr,
            clerk_frontend_api: raw_config.clerk_frontend_api,
        })
    }
}
