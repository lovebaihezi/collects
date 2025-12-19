use collects_states::State;
use std::any::Any;
use ustr::Ustr;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    api_base_url: String,
}

impl ApiConfig {
    pub fn new(base_url: String) -> Self {
        Self {
            api_base_url: base_url,
        }
    }

    pub fn api_url(&self) -> Ustr {
        Ustr::from(&format!("{}/api", self.api_base_url))
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_base_url: if cfg!(feature = "env_test") {
                "https://collects-test.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_internal") {
                "https://collects-internal.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_nightly") {
                "https://collects-nightly.lqxclqxc.com".to_string()
            } else {
                "https://collects.lqxclqxc.com".to_string()
            },
        }
    }
}

impl State for ApiConfig {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
