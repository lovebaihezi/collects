use collects_utils::version_info::RuntimeEnv;
use serde::Deserialize;
use std::env::vars;
use std::fmt::Display;
use tracing::info;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
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
            Env::Local => Self::Local,
            Env::Prod => Self::Prod,
            Env::Internal => Self::Internal,
            Env::Test => Self::Test,
            Env::TestInternal => Self::TestInternal,
            Env::Pr => Self::Pr,
            Env::Nightly => Self::Nightly,
        }
    }
}

impl Display for Env {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Prod => write!(f, "prod"),
            Self::Internal => write!(f, "internal"),
            Self::Test => write!(f, "test"),
            Self::TestInternal => write!(f, "test-internal"),
            Self::Pr => write!(f, "pr"),
            Self::Nightly => write!(f, "nightly"),
        }
    }
}

impl Env {
    /// Returns true if this environment requires R2 storage credentials.
    fn requires_r2(&self) -> bool {
        // R2 is required for all runtime environments (including local/test).
        true
    }

    /// Returns true if this environment requires Zero Trust configuration.
    fn requires_zero_trust(&self) -> bool {
        matches!(self, Self::Internal | Self::TestInternal)
    }

    /// Returns true if this environment requires a JWT secret to be explicitly set.
    fn requires_jwt_secret(&self) -> bool {
        !matches!(self, Self::Local | Self::Test | Self::TestInternal)
    }

    /// Returns true if this is a local or test environment.
    fn is_local_or_test(&self) -> bool {
        matches!(self, Self::Local | Self::Test | Self::TestInternal)
    }

    /// Returns the default server address for this environment.
    fn default_server_addr(&self) -> &'static str {
        match self {
            Self::Local => "127.0.0.1",
            _ => "0.0.0.0",
        }
    }
}

/// Cloudflare R2 storage configuration.
/// All fields are required when this config is present.
#[derive(Debug, Clone)]
pub struct R2Config {
    account_id: String,
    access_key_id: String,
    secret_access_key: String,
    bucket: String,
}

impl R2Config {
    pub fn account_id(&self) -> &str {
        &self.account_id
    }

    pub fn access_key_id(&self) -> &str {
        &self.access_key_id
    }

    pub fn secret_access_key(&self) -> &str {
        &self.secret_access_key
    }

    pub fn bucket(&self) -> &str {
        &self.bucket
    }
}

/// Cloudflare Zero Trust configuration for internal routes.
/// All fields are required when this config is present.
#[derive(Debug, Clone)]
pub struct ZeroTrustConfig {
    team_domain: String,
    audience: String,
}

impl ZeroTrustConfig {
    pub fn team_domain(&self) -> &str {
        &self.team_domain
    }

    pub fn audience(&self) -> &str {
        &self.audience
    }
}

/// Raw configuration deserialized directly from environment variables.
/// All optional fields are checked post-deserialization based on environment.
#[derive(Deserialize)]
struct RawConfig {
    env: Env,
    database_url: String,
    server_addr: Option<String>,
    port: Option<u16>,
    jwt_secret: Option<String>,

    // R2 storage fields (grouped logically, validated together)
    cf_account_id: Option<String>,
    cf_access_key_id: Option<String>,
    cf_secret_access_key: Option<String>,
    cf_bucket: Option<String>,

    // Zero Trust fields (grouped logically, validated together)
    cf_access_team_domain: Option<String>,
    cf_access_aud: Option<String>,
}

impl RawConfig {
    /// Try to construct `R2Config` if all required fields are present.
    /// Returns None if no R2 fields are set, or Err if partially configured.
    fn try_r2_config(&self) -> Result<Option<R2Config>, &'static str> {
        match (
            &self.cf_account_id,
            &self.cf_access_key_id,
            &self.cf_secret_access_key,
            &self.cf_bucket,
        ) {
            (Some(account_id), Some(access_key_id), Some(secret_access_key), Some(bucket)) => {
                Ok(Some(R2Config {
                    account_id: account_id.clone(),
                    access_key_id: access_key_id.clone(),
                    secret_access_key: secret_access_key.clone(),
                    bucket: bucket.clone(),
                }))
            }
            (None, None, None, None) => Ok(None),
            _ => Err(
                "Partial R2 configuration: all of CF_ACCOUNT_ID, CF_ACCESS_KEY_ID, \
                 CF_SECRET_ACCESS_KEY, and CF_BUCKET must be set together",
            ),
        }
    }

    /// Try to construct `ZeroTrustConfig` if all required fields are present.
    /// Returns None if no Zero Trust fields are set, or Err if partially configured.
    fn try_zero_trust_config(&self) -> Result<Option<ZeroTrustConfig>, &'static str> {
        match (&self.cf_access_team_domain, &self.cf_access_aud) {
            (Some(team_domain), Some(audience)) => Ok(Some(ZeroTrustConfig {
                team_domain: team_domain.clone(),
                audience: audience.clone(),
            })),
            (None, None) => Ok(None),
            _ => Err(
                "Partial Zero Trust configuration: both CF_ACCESS_TEAM_DOMAIN and \
                 CF_ACCESS_AUD must be set together",
            ),
        }
    }
}

/// The final, validated configuration struct.
#[derive(Debug, Clone)]
pub struct Config {
    env: Env,
    database_url: String,
    server_addr: String,
    port: u16,
    jwt_secret: String,
    r2: Option<R2Config>,
    zero_trust: Option<ZeroTrustConfig>,
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
            jwt_secret: "test-jwt-secret-key-for-local-development".to_string(),
            r2: Some(R2Config {
                account_id: "test-account".to_string(),
                access_key_id: "test-access-key".to_string(),
                secret_access_key: "test-secret".to_string(),
                bucket: "test-bucket".to_string(),
            }),
            zero_trust: None,
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
        Self {
            zero_trust: Some(ZeroTrustConfig {
                team_domain: team_domain.into(),
                audience: audience.into(),
            }),
            ..Self::new_for_test()
        }
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

    /// Get the JWT secret for signing session tokens.
    pub fn jwt_secret(&self) -> &str {
        &self.jwt_secret
    }

    // R2 storage configuration
    pub fn r2(&self) -> Option<&R2Config> {
        self.r2.as_ref()
    }

    pub fn cf_account_id(&self) -> Option<&str> {
        self.r2.as_ref().map(|r| r.account_id())
    }

    pub fn cf_access_key_id(&self) -> Option<&str> {
        self.r2.as_ref().map(|r| r.access_key_id())
    }

    pub fn cf_secret_access_key(&self) -> Option<&str> {
        self.r2.as_ref().map(|r| r.secret_access_key())
    }

    pub fn cf_bucket(&self) -> Option<&str> {
        self.r2.as_ref().map(|r| r.bucket())
    }

    // Zero Trust configuration
    pub fn zero_trust(&self) -> Option<&ZeroTrustConfig> {
        self.zero_trust.as_ref()
    }

    pub fn cf_access_team_domain(&self) -> Option<&str> {
        self.zero_trust.as_ref().map(|z| z.team_domain())
    }

    pub fn cf_access_aud(&self) -> Option<&str> {
        self.zero_trust.as_ref().map(|z| z.audience())
    }

    /// Initializes configuration by reading from environment variables
    /// and applying environment-aware defaults.
    pub fn init() -> anyhow::Result<Self> {
        info!("Loading configuration from environment variables");

        let raw_config: RawConfig = serde_env::from_iter(vars())?;
        Self::from_raw(raw_config)
    }

    fn from_raw(mut raw: RawConfig) -> anyhow::Result<Self> {
        let env = raw.env.clone();

        // Apply default server_addr based on environment
        let server_addr = if let Some(addr) = raw.server_addr.take() {
            addr
        } else {
            let default = env.default_server_addr();
            info!("SERVER_ADDR not set, defaulting to {default} for {env} environment");
            default.to_string()
        };

        // Port: required for non-local, defaults to 8080 for local
        let port = match raw.port {
            Some(p) => p,
            None if env.is_local_or_test() => {
                info!("PORT not set, defaulting to 8080 for {env} environment");
                8080
            }
            None => anyhow::bail!("PORT must be set for {env} environment"),
        };

        // JWT secret: required for production environments
        let jwt_secret = match raw.jwt_secret.take() {
            Some(secret) => secret,
            None if !env.requires_jwt_secret() => {
                info!("JWT_SECRET not set, using default for {env} environment");
                "default-jwt-secret-for-local-development-only".to_string()
            }
            None => anyhow::bail!("JWT_SECRET must be set for {env} environment"),
        };

        // Build and validate R2 config
        let r2 = raw.try_r2_config().map_err(anyhow::Error::msg)?;
        if env.requires_r2() && r2.is_none() {
            anyhow::bail!(
                "R2 storage credentials (CF_ACCOUNT_ID, CF_ACCESS_KEY_ID, CF_SECRET_ACCESS_KEY, CF_BUCKET) \
                 must be set for {env} environment"
            );
        }
        if r2.is_some() {
            info!("R2 storage credentials validated for {env} environment");
        }

        // Build and validate Zero Trust config
        let zero_trust = raw.try_zero_trust_config().map_err(anyhow::Error::msg)?;
        if env.requires_zero_trust() && zero_trust.is_none() {
            anyhow::bail!(
                "Zero Trust credentials (CF_ACCESS_TEAM_DOMAIN, CF_ACCESS_AUD) \
                 must be set for {env} environment. Internal routes require Zero Trust authentication."
            );
        }

        Ok(Self {
            env,
            database_url: raw.database_url,
            server_addr,
            port,
            jwt_secret,
            r2,
            zero_trust,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_env::from_iter;

    fn make_raw(vars: Vec<(&str, &str)>) -> RawConfig {
        from_iter(vars).expect("RawConfig should deserialize")
    }

    #[test]
    fn default_server_addr_for_pr_is_public() {
        let raw = make_raw(vec![
            ("ENV", "pr"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let config = Config::from_raw(raw).expect("pr config should build");
        assert_eq!(config.server_addr(), "0.0.0.0");
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn r2_credentials_required_for_prod() {
        let raw = make_raw(vec![
            ("ENV", "prod"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("R2 storage"));
    }

    #[test]
    fn r2_credentials_required_for_local() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("R2 storage"));
    }

    #[test]
    fn r2_config_parsed_when_all_fields_present() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
            ("CF_ACCOUNT_ID", "my-account"),
            ("CF_ACCESS_KEY_ID", "my-key"),
            ("CF_SECRET_ACCESS_KEY", "my-secret"),
            ("CF_BUCKET", "my-bucket"),
        ]);

        let config = Config::from_raw(raw).expect("config should build with R2");
        let r2 = config.r2().expect("R2 config should be present");
        assert_eq!(r2.account_id(), "my-account");
        assert_eq!(r2.access_key_id(), "my-key");
        assert_eq!(r2.secret_access_key(), "my-secret");
        assert_eq!(r2.bucket(), "my-bucket");
    }

    #[test]
    fn r2_config_partial_fields_error() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
            ("CF_ACCOUNT_ID", "my-account"),
            // Missing other R2 fields
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Partial R2"));
    }

    #[test]
    fn default_server_addr_for_test_internal_is_public() {
        let raw = make_raw(vec![
            ("ENV", "test-internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "test-audience"),
        ]);

        let config = Config::from_raw(raw).expect("test-internal config should build");
        assert_eq!(config.server_addr(), "0.0.0.0");
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn internal_env_requires_zero_trust_config() {
        // Internal env requires both R2 and Zero Trust, so include R2 to test Zero Trust validation
        let raw = make_raw(vec![
            ("ENV", "internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Zero Trust"));
    }

    #[test]
    fn internal_env_succeeds_with_zero_trust_config() {
        // Internal env requires both R2 and Zero Trust
        let raw = make_raw(vec![
            ("ENV", "internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("JWT_SECRET", "test-jwt-secret"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "test-audience"),
        ]);

        let config = Config::from_raw(raw).expect("internal config should build with Zero Trust");
        let zt = config
            .zero_trust()
            .expect("Zero Trust config should be present");
        assert_eq!(zt.team_domain(), "myteam.cloudflareaccess.com");
        assert_eq!(zt.audience(), "test-audience");
    }

    #[test]
    fn zero_trust_partial_fields_error() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            // Missing CF_ACCESS_AUD
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Partial Zero Trust")
        );
    }

    #[test]
    fn test_internal_env_requires_zero_trust_config() {
        let raw = make_raw(vec![
            ("ENV", "test-internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Zero Trust"));
    }

    #[test]
    fn test_internal_env_succeeds_with_zero_trust_config() {
        let raw = make_raw(vec![
            ("ENV", "test-internal"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
            ("CF_ACCESS_TEAM_DOMAIN", "myteam.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "test-audience"),
        ]);

        let config =
            Config::from_raw(raw).expect("test-internal config should build with Zero Trust");
        assert_eq!(
            config.cf_access_team_domain(),
            Some("myteam.cloudflareaccess.com")
        );
        assert_eq!(config.cf_access_aud(), Some("test-audience"));
    }

    #[test]
    fn jwt_secret_required_for_prod() {
        let raw = make_raw(vec![
            ("ENV", "prod"),
            ("DATABASE_URL", "postgres://example"),
            ("PORT", "8080"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("JWT_SECRET"));
    }

    #[test]
    fn jwt_secret_optional_for_local() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let config = Config::from_raw(raw).expect("local config should build without JWT_SECRET");
        assert_eq!(
            config.jwt_secret(),
            "default-jwt-secret-for-local-development-only"
        );
    }

    #[test]
    fn port_required_for_non_local() {
        let raw = make_raw(vec![
            ("ENV", "prod"),
            ("DATABASE_URL", "postgres://example"),
            ("JWT_SECRET", "secret"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let result = Config::from_raw(raw);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("PORT"));
    }

    #[test]
    fn port_defaults_for_local() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
            ("CF_ACCOUNT_ID", "test-account"),
            ("CF_ACCESS_KEY_ID", "test-access-key"),
            ("CF_SECRET_ACCESS_KEY", "test-secret"),
            ("CF_BUCKET", "test-bucket"),
        ]);

        let config = Config::from_raw(raw).expect("local config should build");
        assert_eq!(config.port(), 8080);
    }

    #[test]
    fn env_helper_methods() {
        assert!(Env::Prod.requires_r2());
        assert!(Env::Internal.requires_r2());
        assert!(Env::Pr.requires_r2());
        assert!(Env::Nightly.requires_r2());
        assert!(Env::Local.requires_r2());
        assert!(Env::Test.requires_r2());
        assert!(Env::TestInternal.requires_r2());

        assert!(Env::Internal.requires_zero_trust());
        assert!(Env::TestInternal.requires_zero_trust());
        assert!(!Env::Prod.requires_zero_trust());
        assert!(!Env::Local.requires_zero_trust());

        assert!(Env::Prod.requires_jwt_secret());
        assert!(Env::Internal.requires_jwt_secret());
        assert!(!Env::Local.requires_jwt_secret());
        assert!(!Env::Test.requires_jwt_secret());
    }

    #[test]
    fn backward_compat_accessors() {
        let raw = make_raw(vec![
            ("ENV", "local"),
            ("DATABASE_URL", "postgres://example"),
            ("CF_ACCOUNT_ID", "my-account"),
            ("CF_ACCESS_KEY_ID", "my-key"),
            ("CF_SECRET_ACCESS_KEY", "my-secret"),
            ("CF_BUCKET", "my-bucket"),
            ("CF_ACCESS_TEAM_DOMAIN", "team.cloudflareaccess.com"),
            ("CF_ACCESS_AUD", "aud123"),
        ]);

        let config = Config::from_raw(raw).expect("config should build");

        // Test backward-compatible accessors
        assert_eq!(config.cf_account_id(), Some("my-account"));
        assert_eq!(config.cf_access_key_id(), Some("my-key"));
        assert_eq!(config.cf_secret_access_key(), Some("my-secret"));
        assert_eq!(config.cf_bucket(), Some("my-bucket"));
        assert_eq!(
            config.cf_access_team_domain(),
            Some("team.cloudflareaccess.com")
        );
        assert_eq!(config.cf_access_aud(), Some("aud123"));
    }
}
