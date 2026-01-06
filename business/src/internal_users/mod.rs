//! Internal users domain module.
//!
//! This module is the single home for:
//! - State stored in `StateCtx` for internal users UI (panel state, modal state, etc.)
//! - Computes that cache derived/async results (if any)
//! - Business-layer API helpers for `/internal/*` endpoints
//!
//! UI code under `ui/src/widgets/**` should not define domain `State`/`Compute`/`Command`.
//! It should only read via `ctx.cached::<T>()` and trigger changes via `ctx.dispatch::<Cmd>()`.

pub mod action_compute;
pub mod api;
pub mod list_users_compute;
pub mod state;

pub use action_compute::{
    DeleteUserCommand, GetUserQrCommand, InternalUsersActionCompute, InternalUsersActionInput,
    InternalUsersActionKind, InternalUsersActionState, RevokeOtpCommand, UpdateProfileCommand,
    UpdateUsernameCommand,
};

pub use list_users_compute::{
    InternalUsersListUsersCompute, InternalUsersListUsersInput, InternalUsersListUsersResult,
    RefreshInternalUsersCommand,
};
