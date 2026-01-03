use collects_business::ApiStatus;
use collects_business::BusinessConfig;
use collects_business::Route;
use collects_business::{AuthCompute, LoginCommand, LoginInput, LogoutCommand};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{
    CFTokenCompute, CFTokenInput, CreateUserCommand, CreateUserCompute, CreateUserInput,
    InternalApiStatus, SetCFTokenCommand,
};
use collects_states::{StateCtx, Time};
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use crate::widgets::InternalUsersState;

#[derive(Deserialize, Serialize)]
pub struct State {
    // We need to store the persistent state
    #[serde(skip)]
    pub ctx: StateCtx,
    /// Internal users state (only for internal builds)
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    #[serde(skip)]
    pub internal_users: InternalUsersState,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.add_state(Route::default());
        ctx.record_compute(ApiStatus::default());

        // Add login states and commands
        ctx.add_state(LoginInput::default());

        // For internal builds, use Zero Trust authentication (skip login page)
        // For other builds, use default (not authenticated)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        ctx.record_compute(AuthCompute::zero_trust_authenticated());
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        ctx.record_compute(AuthCompute::default());

        ctx.record_command(LoginCommand);
        ctx.record_command(LogoutCommand);

        // Add internal states and computes for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            // Cloudflare Access token (manual input) + compute cache
            ctx.add_state(CFTokenInput::default());
            ctx.record_compute(CFTokenCompute::default());
            ctx.record_command(SetCFTokenCommand::default());

            // Create user flow
            ctx.add_state(CreateUserInput::default());
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(CreateUserCompute::default());
            ctx.record_command(CreateUserCommand::default());
        }

        Self {
            ctx,
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            internal_users: InternalUsersState::new(),
        }
    }
}

impl State {
    pub fn test(base_url: String) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::new(base_url));
        ctx.add_state(Route::default());
        ctx.record_compute(ApiStatus::default());

        // Add login states and commands
        ctx.add_state(LoginInput::default());

        // For internal builds, use Zero Trust authentication (skip login page)
        // For other builds, use default (not authenticated)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        ctx.record_compute(AuthCompute::zero_trust_authenticated());
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        ctx.record_compute(AuthCompute::default());

        ctx.record_command(LoginCommand);
        ctx.record_command(LogoutCommand);

        // Add internal states and computes for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            // Cloudflare Access token (manual input) + compute cache
            ctx.add_state(CFTokenInput::default());
            ctx.record_compute(CFTokenCompute::default());
            ctx.record_command(SetCFTokenCommand::default());

            // Create user flow
            ctx.add_state(CreateUserInput::default());
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(CreateUserCompute::default());
            ctx.record_command(CreateUserCommand::default());
        }

        Self {
            ctx,
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            internal_users: InternalUsersState::new(),
        }
    }
}
