use collects_states::{State, state_assign_impl};
use std::any::Any;
use ustr::Ustr;

#[derive(Debug, Clone)]
pub struct BusinessConfig {
    pub api_base_url: String,
    /// Optional Cloudflare Access token used for internal endpoints.
    ///
    /// When present, internal API calls should send it via the `cf-authorization` header.
    pub cf_authorization: Option<String>,
}

impl BusinessConfig {
    pub fn new(base_url: String) -> Self {
        Self {
            api_base_url: base_url,
            cf_authorization: None,
        }
    }

    pub fn api_url(&self) -> Ustr {
        if self.api_base_url.is_empty() {
            Ustr::from("/api")
        } else {
            Ustr::from(&format!("{}/api", self.api_base_url))
        }
    }

    pub fn cf_authorization(&self) -> Option<&str> {
        self.cf_authorization.as_deref()
    }
}

impl Default for BusinessConfig {
    fn default() -> Self {
        Self {
            api_base_url: if cfg!(target_arch = "wasm32") {
                "".to_string()
            } else if cfg!(feature = "env_test") {
                "https://collects-test.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_test_internal") {
                "https://collects-test-internal.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_pr") {
                "https://collects-pr.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_internal") {
                "https://collects-internal.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_nightly") {
                "https://collects-nightly.lqxclqxc.com".to_string()
            } else {
                "https://collects.lqxclqxc.com".to_string()
            },
            cf_authorization: None,
        }
    }
}

impl State for BusinessConfig {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn snapshot(&self) -> Option<Box<dyn Any + Send + 'static>> {
        Some(Box::new(self.clone()))
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_urls() {
        let config = BusinessConfig::default();

        if cfg!(target_arch = "wasm32") {
            assert_eq!(config.api_base_url, "");
            assert_eq!(config.api_url(), Ustr::from("/api"));
        } else if cfg!(feature = "env_test") {
            assert_eq!(config.api_base_url, "https://collects-test.lqxclqxc.com");
            assert_eq!(
                config.api_url(),
                Ustr::from("https://collects-test.lqxclqxc.com/api")
            );
        } else if cfg!(feature = "env_internal") {
            assert_eq!(
                config.api_base_url,
                "https://collects-internal.lqxclqxc.com"
            );
            assert_eq!(
                config.api_url(),
                Ustr::from("https://collects-internal.lqxclqxc.com/api")
            );
        } else if cfg!(feature = "env_nightly") {
            assert_eq!(config.api_base_url, "https://collects-nightly.lqxclqxc.com");
            assert_eq!(
                config.api_url(),
                Ustr::from("https://collects-nightly.lqxclqxc.com/api")
            );
        } else {
            // Default production
            assert_eq!(config.api_base_url, "https://collects.lqxclqxc.com");
            assert_eq!(
                config.api_url(),
                Ustr::from("https://collects.lqxclqxc.com/api")
            );
        }
    }
}
