use collects_states::State;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub api_base_url: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_base_url: if cfg!(feature = "env_test") {
                "https://collects-test.lqxclqxc.com".to_string()
            } else if cfg!(feature = "env_internal") {
                "https://collects-internal.lqxclqxc.com".to_string()
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
