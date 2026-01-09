use collects_utils::version_info::RuntimeEnv;
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
    #[serde(rename = "internal")]
    Internal,
    #[serde(rename = "test")]
    Test,
    #[serde(rename = "test-internal")]
    TestInternal,
    #[serde(rename = "pr")]
    Pr,
    #[serde(rename = "nightly")]
    Nightly,
}

impl From<&Env> for RuntimeEnv {
    fn from(env: &Env) -> Self {
        match env {
            Env::Local => RuntimeEnv::Local,
            Env::Prod => RuntimeEnv::Prod,
            Env::Internal => RuntimeEnv::Internal,
            Env::Test => RuntimeEnv::Test,
            Env::TestInternal => RuntimeEnv::TestInternal,
            Env::Pr => RuntimeEnv::Pr,
            Env::Nightly => RuntimeEnv::Nightly,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_env::from_iter;

    #[test]
    fn default_server_addr_for_pr_is_public() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "pr"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ])
        .expect("RawConfig should deserialize");

        let config = Config::from_raw(raw).expect("pr config should build");
        assert_eq!(config.server_addr(), "0.0.0.0");
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn r2_credentials_required_for_prod() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "prod"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
        ])
        .expect("RawConfig should deserialize");

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CF_ACCOUNT_ID"));
    }

    #[test]
    fn r2_credentials_optional_for_local() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
        ])
        .expect("RawConfig should deserialize");

        let config = Config::from_raw(raw).expect("local config should build without R2 creds");
        assert!(config.cf_account_id().is_none());
    }

    #[test]
    fn default_server_addr_for_test_internal_is_public() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "test-internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "test-audience"),
        ])
        .expect("RawConfig should deserialize");

        let config = Config::from_raw(raw).expect("test-internal config should build");
        assert_eq!(config.server_addr(), "0.0.0.0");
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn internal_env_requires_zero_trust_config() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
        ])
        .expect("RawConfig should deserialize");

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("CF_ACCESS_TEAM_DOMAIN"));
        assert!(err.contains("CF_ACCESS_AUD"));
    }

    #[test]
    fn internal_env_succeeds_with_zero_trust_config() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "test-audience"),
        ])
        .expect("RawConfig should deserialize");

        let config = Config::from_raw(raw).expect("internal config should build with Zero Trust");
        assert_eq!(
            config.cf_access_team_domain(),
            Some("myteam.cloudflareaccess.com")
        );
        assert_eq!(config.cf_access_aud(), Some("test-audience"));
    }

    #[test]
    fn test_internal_env_requires_zero_trust_config() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "test-internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
        ])
        .expect("RawConfig should deserialize");

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("CF_ACCESS_TEAM_DOMAIN"));
        assert!(err.contains("CF_ACCESS_AUD"));
    }

    #[test]
    fn test_internal_env_succeeds_with_zero_trust_config() {
        let raw: RawConfig = from_iter(vec![
            ("ENV", "test-internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "test-audience"),
        ])
        .expect("RawConfig should deserialize");

        let config =
            Config::from_raw(raw).expect("test-internal config should build with Zero Trust");
        assert_eq!(
            config.cf_access_team_domain(),
            Some("myteam.cloudflareaccess.com")
        );
        assert_eq!(config.cf_access_aud(), Some("test-audience"));
    }
}

impl Display for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Env::Local => write!(f, "local"),
            Env::Prod => write!(f, "prod"),
            Env::Internal => write!(f, "internal"),
            Env::Test => write!(f, "test"),
            Env::TestInternal => write!(f, "test-internal"),
            Env::Pr => write!(f, "pr"),
            Env::Nightly => write!(f, "nightly"),
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
    // Storage configuration (optional)
    cf_account_id: Option<String>,
    cf_access_key_id: Option<String>,
    cf_secret_access_key: Option<String>,
    cf_bucket: Option<String>,
    gcs_bucket: Option<String>,
    gcs_credentials: Option<String>,
    // Cloudflare Zero Trust configuration
    cf_access_team_domain: Option<String>,
    cf_access_aud: Option<String>,
    // JWT token secret for session tokens
    jwt_secret: String,
}

// An intermediate struct for deserializing environment variables
// where `server_addr` is optional.
#[derive(Deserialize)]
struct RawConfig {
    env: Env,
    database_url: String,
    server_addr: Option<String>,
    port: Option<u16>,
    // Storage configuration (optional)
    cf_account_id: Option<String>,
    cf_access_key_id: Option<String>,
    cf_secret_access_key: Option<String>,
    cf_bucket: Option<String>,
    gcs_bucket: Option<String>,
    gcs_credentials: Option<String>,
    // Cloudflare Zero Trust configuration
    cf_access_team_domain: Option<String>,
    cf_access_aud: Option<String>,
    // JWT token secret (optional, default generated for local/test)
    jwt_secret: Option<String>,
}

impl Config {
    /// Create a test configuration with default values.
    ///
    /// This function is available for both unit tests and integration tests.
    /// It should not be used in production code.
    pub fn new_for_test() -> Self {
        Self {
            env: Env::Local,
            database_url: "postgres://localhost:5432/test".to_string(),
            server_addr: "127.0.0.1".to_string(),
            port: 8080,
            cf_account_id: None,
            cf_access_key_id: None,
            cf_secret_access_key: None,
            cf_bucket: None,
            gcs_bucket: None,
            gcs_credentials: None,
            cf_access_team_domain: None,
            cf_access_aud: None,
            jwt_secret: "test-jwt-secret-key-for-local-development".to_string(),
        }
    }

    /// Create a test configuration with Cloudflare Zero Trust enabled for internal routes.
    ///
    /// This is intended for integration tests that validate the `/internal/*` routes with
    /// Zero Trust middleware enabled (e.g. `cargo test --all-features`).
    pub fn new_for_test_internal(
        team_domain: impl Into<String>,
        audience: impl Into<String>,
    ) -> Self {
        let mut config = Self::new_for_test();
        config.cf_access_team_domain = Some(team_domain.into());
        config.cf_access_aud = Some(audience.into());
        config
    }

    /// Create a test configuration with a specific environment.
    ///
    /// This is intended for unit tests that need to test behavior with different environments.
    #[cfg(test)]
    pub fn new_for_test_with_env(env: Env) -> Self {
        Self {
            env,
            ..Self::new_for_test()
        }
    }

    pub fn environment(&self) -> &Env {
        &self.env
    }

    pub fn database_url(&self) -> &str {
        &self.database_url
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

    // Storage configuration getters
    pub fn cf_account_id(&self) -> Option<&str> {
        self.cf_account_id.as_deref()
    }

    pub fn cf_access_key_id(&self) -> Option<&str> {
        self.cf_access_key_id.as_deref()
    }

    pub fn cf_secret_access_key(&self) -> Option<&str> {
        self.cf_secret_access_key.as_deref()
    }

    pub fn cf_bucket(&self) -> Option<&str> {
        self.cf_bucket.as_deref()
    }

    pub fn gcs_bucket(&self) -> Option<&str> {
        self.gcs_bucket.as_deref()
    }

    pub fn gcs_credentials(&self) -> Option<&str> {
        self.gcs_credentials.as_deref()
    }

    pub fn cf_access_team_domain(&self) -> Option<&str> {
        self.cf_access_team_domain.as_deref()
    }

    pub fn cf_access_aud(&self) -> Option<&str> {
        self.cf_access_aud.as_deref()
    }

    /// Get the JWT secret for signing session tokens.
    pub fn jwt_secret(&self) -> &str {
        &self.jwt_secret
    }

    /// Initializes configuration by reading from environment variables
    /// and applying environment-aware defaults.
    pub fn init() -> anyhow::Result<Self> {
        info!("Loading configuration from environment variables");

        // First, deserialize into a temporary struct that allows for optional fields
        let raw_config: RawConfig = serde_env::from_iter(vars())?;
        Self::from_raw(raw_config)
    }

    fn from_raw(raw_config: RawConfig) -> anyhow::Result<Self> {
        let RawConfig {
            env,
            database_url,
            server_addr,
            port,
            cf_account_id,
            cf_access_key_id,
            cf_secret_access_key,
            cf_bucket,
            gcs_bucket,
            gcs_credentials,
            cf_access_team_domain,
            cf_access_aud,
            jwt_secret,
        } = raw_config;

        // Apply the default logic for `server_addr` based on the environment
        let server_addr = match server_addr {
            Some(addr) => {
                info!("Using provided SERVER_ADDR: {}", addr);
                addr
            }
            None => {
                let default_addr = match env {
                    Env::Local => "127.0.0.1",
                    _ => "0.0.0.0",
                };
                info!(
                    "SERVER_ADDR not set, defaulting to {} for {} environment",
                    default_addr, env
                );
                default_addr.to_string()
            }
        };

        let port = match port {
            Some(port) => port,
            None if matches!(env, Env::Local) => {
                info!("PORT not set, defaulting to 8080 for local environment");
                8080
            }
            None => anyhow::bail!("PORT must be set for {} environment", env),
        };

        // JWT secret is required for production, optional for local/test
        let jwt_secret = match jwt_secret {
            Some(secret) => secret,
            None if matches!(env, Env::Local | Env::Test | Env::TestInternal) => {
                info!("JWT_SECRET not set, using default for {} environment", env);
                "default-jwt-secret-for-local-development-only".to_string()
            }
            None => anyhow::bail!("JWT_SECRET must be set for {} environment", env),
        };

        // Zero Trust configuration is required for Internal and TestInternal environments
        if matches!(env, Env::Internal | Env::TestInternal)
            && (cf_access_team_domain.is_none() || cf_access_aud.is_none())
        {
            anyhow::bail!(
                "CF_ACCESS_TEAM_DOMAIN and CF_ACCESS_AUD must be set for {} environment. \
                 Internal routes require Zero Trust authentication.",
                env
            );
        }

        // R2 credentials are required for non-local environments
        if !matches!(env, Env::Local | Env::Test | Env::TestInternal) {
            if cf_account_id.is_none() {
                anyhow::bail!("CF_ACCOUNT_ID must be set for {} environment", env);
            }
            if cf_access_key_id.is_none() {
                anyhow::bail!("CF_ACCESS_KEY_ID must be set for {} environment", env);
            }
            if cf_secret_access_key.is_none() {
                anyhow::bail!("CF_SECRET_ACCESS_KEY must be set for {} environment", env);
            }
            if cf_bucket.is_none() {
                anyhow::bail!("CF_BUCKET must be set for {} environment", env);
            }
            info!("R2 storage credentials validated for {} environment", env);
        }

        // Construct the final, validated Config struct
        Ok(Config {
            env,
            database_url,
            port,
            server_addr,
            cf_account_id,
            cf_access_key_id,
            cf_secret_access_key,
            cf_bucket,
            gcs_bucket,
            gcs_credentials,
            cf_access_team_domain,
            cf_access_aud,
            jwt_secret,
        })
    }
}
