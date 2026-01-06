use collects_business::ApiStatus;
use collects_business::BusinessConfig;
use collects_business::Route;
use collects_business::ToggleApiStatusCommand;
use collects_business::{
    AuthCompute, LoginCommand, LoginInput, LogoutCommand, PendingTokenValidation,
    ValidateTokenCommand,
};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{
    CFTokenCompute, CFTokenInput, CreateUserCommand, CreateUserCompute, CreateUserInput,
    InternalApiStatus, SetCFTokenCommand,
};
use collects_states::{StateCtx, Time};
use serde::{Deserialize, Serialize};

use crate::widgets::ImagePreviewState;
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use crate::widgets::InternalUsersState;

/// Key for storing the auth token in egui storage.
pub const AUTH_TOKEN_STORAGE_KEY: &str = "collects_auth_token";

#[derive(Deserialize, Serialize)]
pub struct State {
    // We need to store the persistent state
    #[serde(skip)]
    pub ctx: StateCtx,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.add_state(Route::default());
        ctx.record_compute(ApiStatus::default());
        ctx.record_command(ToggleApiStatusCommand);

        // Add login states and commands
        ctx.add_state(LoginInput::default());
        ctx.add_state(PendingTokenValidation::default());

        // For internal builds, use Zero Trust authentication (skip login page)
        // For other builds, use default (not authenticated)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        ctx.record_compute(AuthCompute::zero_trust_authenticated());
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        ctx.record_compute(AuthCompute::default());

        ctx.record_command(LoginCommand);
        ctx.record_command(LogoutCommand);
        ctx.record_command(ValidateTokenCommand);

        // Add image preview state for clipboard/drop image handling
        ctx.add_state(ImagePreviewState::new());

        // Add internal states and computes for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            // Cloudflare Access token (manual input) + compute cache
            ctx.add_state(CFTokenInput::default());
            ctx.record_compute(CFTokenCompute::default());
            ctx.record_command(SetCFTokenCommand);

            // Create user flow
            ctx.add_state(CreateUserInput::default());
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(CreateUserCompute::default());
            ctx.record_command(CreateUserCommand);

            // Internal users state
            ctx.add_state(InternalUsersState::new());

            // Internal users: refresh/list users compute + input + command
            //
            // This replaces the previous pattern where UI used egui `ctx.memory_mut`
            // as an async message bus for list-users refresh results.
            ctx.add_state(collects_business::InternalUsersListUsersInput::default());
            ctx.record_compute(collects_business::InternalUsersListUsersCompute::default());
            ctx.record_command(collects_business::RefreshInternalUsersCommand);
        }

        Self { ctx }
    }
}

impl State {
    pub fn test(base_url: String) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::new(base_url));
        ctx.add_state(Route::default());
        ctx.record_compute(ApiStatus::default());
        ctx.record_command(ToggleApiStatusCommand);

        // Add login states and commands
        ctx.add_state(LoginInput::default());
        ctx.add_state(PendingTokenValidation::default());

        // For internal builds, use Zero Trust authentication (skip login page)
        // For other builds, use default (not authenticated)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        ctx.record_compute(AuthCompute::zero_trust_authenticated());
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        ctx.record_compute(AuthCompute::default());

        ctx.record_command(LoginCommand);
        ctx.record_command(LogoutCommand);
        ctx.record_command(ValidateTokenCommand);

        // Add image preview state for clipboard/drop image handling
        ctx.add_state(ImagePreviewState::new());

        // Add internal states and computes for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            // Cloudflare Access token (manual input) + compute cache
            ctx.add_state(CFTokenInput::default());
            ctx.record_compute(CFTokenCompute::default());
            ctx.record_command(SetCFTokenCommand);

            // Create user flow
            ctx.add_state(CreateUserInput::default());
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(CreateUserCompute::default());
            ctx.record_command(CreateUserCommand);

            // Internal users state
            ctx.add_state(InternalUsersState::new());

            // Internal users: refresh/list users compute + input + command
            //
            // This replaces the previous pattern where UI used egui `ctx.memory_mut`
            // as an async message bus for list-users refresh results.
            ctx.add_state(collects_business::InternalUsersListUsersInput::default());
            ctx.record_compute(collects_business::InternalUsersListUsersCompute::default());
            ctx.record_command(collects_business::RefreshInternalUsersCommand);
        }

        Self { ctx }
    }
}
