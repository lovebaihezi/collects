use collects_business::ApiStatus;
use collects_business::BusinessConfig;
use collects_business::FetchApiStatusCommand;
use collects_business::ImageDiagState;
use collects_business::Route;
use collects_business::ToggleApiStatusCommand;
use collects_business::{
    AuthCompute, LoginCommand, LoginInput, LogoutCommand, PendingTokenValidation,
    ValidateTokenCommand,
};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{
    CFTokenCompute, CFTokenInput, CreateUserCommand, CreateUserCompute, CreateUserInput,
    DeleteUserCommand, FetchInternalApiStatusCommand, GetUserOtpCommand, GetUserQrCommand,
    InternalApiStatus, InternalUsersActionCompute, InternalUsersActionInput,
    InternalUsersListUsersCompute, InternalUsersListUsersInput, RefreshInternalUsersCommand,
    ResetInternalUsersActionCommand, RevokeOtpCommand, SetCFTokenCommand, UpdateProfileCommand,
    UpdateUsernameCommand,
};
use collects_states::{ClipboardImageState, StateCtx, Time};
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
        Self::build(BusinessConfig::default())
    }
}

impl State {
    /// Create a test state with a custom base URL (for mock servers).
    pub fn test(base_url: String) -> Self {
        Self::build(BusinessConfig::new(base_url))
    }

    /// Internal builder that registers all state, compute, and commands.
    ///
    /// This ensures both `default()` and `test()` share identical setup,
    /// differing only in the `BusinessConfig` (environment URLs vs mock server URL).
    fn build(config: BusinessConfig) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(config);
        ctx.add_state(Route::default());
        ctx.record_compute(ApiStatus::default());
        ctx.record_command(ToggleApiStatusCommand);
        ctx.record_command(FetchApiStatusCommand);

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

        // Add clipboard payload state (original encoded bytes + synthesized fallback marker).
        // UI can decode/downconvert this into `ImagePreviewState` for rendering.
        ctx.add_state(ClipboardImageState::new());

        // Add image preview state for clipboard/drop image handling (UI texture + RGBA preview)
        ctx.add_state(ImagePreviewState::new());

        // Add image diagnostic state (for debugging paste/drop across environments)
        // Uses direct state update pattern via update::<ImageDiagState>() for simplicity
        ctx.add_state(ImageDiagState::new());

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
            ctx.record_command(FetchInternalApiStatusCommand);

            // Internal users state
            ctx.add_state(InternalUsersState::new());

            // Internal users: refresh/list users compute + input + command
            //
            // This replaces the previous pattern where UI used egui `ctx.memory_mut`
            // as an async message bus for list-users refresh results.
            ctx.add_state(InternalUsersListUsersInput::default());
            ctx.record_compute(InternalUsersListUsersCompute::default());
            ctx.record_command(RefreshInternalUsersCommand);

            // Internal users: action compute + input + commands
            // Used for QR code display, username update, profile update, delete, revoke OTP
            ctx.add_state(InternalUsersActionInput::default());
            ctx.record_compute(InternalUsersActionCompute::default());
            ctx.record_command(GetUserQrCommand);
            ctx.record_command(GetUserOtpCommand);
            ctx.record_command(UpdateUsernameCommand);
            ctx.record_command(UpdateProfileCommand);
            ctx.record_command(DeleteUserCommand);
            ctx.record_command(RevokeOtpCommand);
            ctx.record_command(ResetInternalUsersActionCommand);
        }

        Self { ctx }
    }
}
