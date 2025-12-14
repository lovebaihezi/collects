use collects_states::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvType {
    Prod,
    Test,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppType {
    Web,
    Native,
}

#[derive(Debug)]
pub struct AppEnv {
    env_type: EnvType,
    app_type: AppType,
}

impl AppEnv {
    pub fn native_prod() -> Self {
        Self {
            env_type: EnvType::Prod,
            app_type: AppType::Native,
        }
    }

    pub fn native_test() -> Self {
        Self {
            env_type: EnvType::Test,
            app_type: AppType::Native,
        }
    }

    pub fn native_local() -> Self {
        Self {
            env_type: EnvType::Local,
            app_type: AppType::Native,
        }
    }

    pub fn web_prod() -> Self {
        Self {
            env_type: EnvType::Prod,
            app_type: AppType::Web,
        }
    }

    pub fn web_test() -> Self {
        Self {
            env_type: EnvType::Test,
            app_type: AppType::Web,
        }
    }

    pub fn web_local() -> Self {
        Self {
            env_type: EnvType::Local,
            app_type: AppType::Web,
        }
    }

    pub fn env_type(&self) -> EnvType {
        self.env_type
    }

    pub fn app_type(&self) -> AppType {
        self.app_type
    }
}

impl State for AppEnv {}
