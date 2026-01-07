//! Configuration file handling for the CLI.
//!
//! Stores user credentials in `$XDG_CONFIG_HOME/collects/config.toml` following
//! the XDG Base Directory Specification.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// CLI configuration stored on disk
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Authentication section
    #[serde(default)]
    pub auth: AuthConfig,
}

/// Authentication configuration
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Saved JWT session token
    pub token: Option<String>,
    /// Username associated with the token
    pub username: Option<String>,
}

impl Config {
    /// Get the configuration file path.
    ///
    /// Returns `$XDG_CONFIG_HOME/collects/config.toml` on Linux,
    /// appropriate paths on other platforms.
    pub fn config_path() -> Result<PathBuf> {
        let project_dirs = ProjectDirs::from("com", "lqxc", "collects")
            .context("Failed to determine config directory")?;

        let config_dir = project_dirs.config_dir();
        Ok(config_dir.join("config.toml"))
    }

    /// Load configuration from disk.
    ///
    /// Returns default configuration if file doesn't exist.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    /// Save configuration to disk.
    ///
    /// Creates the config directory if it doesn't exist.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let content = toml::to_string_pretty(self).context("Failed to serialize configuration")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    /// Check if a token is saved.
    pub fn has_token(&self) -> bool {
        self.auth.token.is_some()
    }

    /// Get the saved token.
    pub fn get_token(&self) -> Option<&str> {
        self.auth.token.as_deref()
    }

    /// Get the saved username.
    pub fn get_username(&self) -> Option<&str> {
        self.auth.username.as_deref()
    }

    /// Set the authentication token and username.
    pub fn set_auth(&mut self, username: &str, token: &str) {
        self.auth.username = Some(username.to_string());
        self.auth.token = Some(token.to_string());
    }

    /// Clear authentication data.
    pub fn clear_auth(&mut self) {
        self.auth.username = None;
        self.auth.token = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.auth.token.is_none());
        assert!(config.auth.username.is_none());
        assert!(!config.has_token());
    }

    #[test]
    fn test_config_set_auth() {
        let mut config = Config::default();
        config.set_auth("testuser", "testtoken");

        assert_eq!(config.get_username(), Some("testuser"));
        assert_eq!(config.get_token(), Some("testtoken"));
        assert!(config.has_token());
    }

    #[test]
    fn test_config_clear_auth() {
        let mut config = Config::default();
        config.set_auth("testuser", "testtoken");
        config.clear_auth();

        assert!(config.get_username().is_none());
        assert!(config.get_token().is_none());
        assert!(!config.has_token());
    }

    #[test]
    fn test_config_serialization() {
        let mut config = Config::default();
        config.set_auth("testuser", "testtoken");

        let toml_str = toml::to_string_pretty(&config).expect("Should serialize");
        assert!(toml_str.contains("testuser"));
        assert!(toml_str.contains("testtoken"));

        let parsed: Config = toml::from_str(&toml_str).expect("Should deserialize");
        assert_eq!(parsed.get_username(), Some("testuser"));
        assert_eq!(parsed.get_token(), Some("testtoken"));
    }
}
