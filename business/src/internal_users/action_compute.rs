//! Internal-users "actions" compute + commands.
//
// This file migrates internal user actions off `egui::Context` temp-memory message passing
// and into the canonical business-layer pattern:
//
// - UI dispatches a Command (manual-only; allowed to do network IO)
// - Command updates a Compute via `Updater::set()`
// - UI reads via `ctx.cached::<InternalUsersActionCompute>()`
//
// Actions covered:
// - Update username
// - Update profile
// - Delete user
// - Revoke OTP (returns new otpauth URL)
// - Get user QR (fetches user -> otpauth URL)

use std::any::Any;

use collects_states::{Command, Compute, ComputeDeps, Dep, State, Updater, assign_impl};
use ustr::Ustr;

use crate::BusinessConfig;
use crate::CFTokenCompute;
use crate::internal_users::api as internal_users_api;
use crate::{
    DeleteUserResponse, GetUserResponse, RevokeOtpResponse, UpdateProfileResponse,
    UpdateUsernameResponse,
};

/// Strongly-typed action kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InternalUsersActionKind {
    UpdateUsername,
    UpdateProfile,
    DeleteUser,
    RevokeOtp,
    GetUserQr,
}

/// Strongly-typed action state.
/// This is intentionally UI-friendly and testable (no stringly-typed magic IDs).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InternalUsersActionState {
    /// No active action.
    #[default]
    Idle,

    /// An action is currently running.
    InFlight {
        kind: InternalUsersActionKind,
        user: Ustr,
    },

    /// An action succeeded.
    ///
    /// Some operations also produce an otpauth URL (QR data), which is stored in `data`.
    Success {
        kind: InternalUsersActionKind,
        user: Ustr,
        data: Option<String>,
    },

    /// An action failed.
    Error {
        kind: InternalUsersActionKind,
        user: Ustr,
        message: String,
    },
}

// Default is derived on `InternalUsersActionState` (Idle).

/// Compute-shaped cache for internal users actions.
#[derive(Debug, Clone, Default)]
pub struct InternalUsersActionCompute {
    pub state: InternalUsersActionState,
}

impl InternalUsersActionCompute {
    pub fn is_in_flight(&self) -> bool {
        matches!(self.state, InternalUsersActionState::InFlight { .. })
    }

    pub fn state(&self) -> &InternalUsersActionState {
        &self.state
    }
}

impl Compute for InternalUsersActionCompute {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn deps(&self) -> ComputeDeps {
        // Updated explicitly by commands; no derived dependencies.
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // Intentionally no-op.
        //
        // Side effects (network) must not run inside a Compute due to implicit execution.
        // Dispatch one of the action commands to update this compute via `Updater::set()`.
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for InternalUsersActionCompute {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Input state for internal users actions.
///
/// UI sets these fields before dispatching the corresponding command.
/// This mirrors existing patterns like `CreateUserInput` and `InternalUsersListUsersInput`.
#[derive(Debug, Clone, Default)]
pub struct InternalUsersActionInput {
    /// Optional override of API base URL (e.g. "https://example.com/api").
    /// Falls back to `BusinessConfig::api_url()` when unset.
    pub api_base_url: Option<Ustr>,

    /// Target username for the action.
    pub username: Option<Ustr>,

    /// New username (for update-username).
    pub new_username: Option<Ustr>,

    /// Nickname (for update-profile). `None` means "clear".
    pub nickname: Option<String>,

    /// Avatar URL (for update-profile). `None` means "clear".
    pub avatar_url: Option<String>,
}

impl State for InternalUsersActionInput {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn resolve_api_base_url(input: &InternalUsersActionInput, config: &BusinessConfig) -> String {
    match input.api_base_url.as_ref() {
        Some(u) => u.as_str().to_string(),
        None => config.api_url().as_str().to_string(),
    }
}

fn missing(field: &str, cmd: &str) -> String {
    format!("{cmd}: missing required input field `{field}`")
}

#[derive(Default, Debug)]
pub struct UpdateUsernameCommand;

impl Command for UpdateUsernameCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<InternalUsersActionInput>();
        let config = deps.get_state_ref::<BusinessConfig>();
        let cf_token = deps.get_state_ref::<CFTokenCompute>();

        let api_base_url = resolve_api_base_url(input, config);
        if api_base_url.trim().is_empty() {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::UpdateUsername,
                    user: input.username.unwrap_or_else(|| Ustr::from("")),
                    message:
                        "UpdateUsernameCommand: missing api_base_url (set InternalUsersActionInput.api_base_url or BusinessConfig.api_base_url)"
                            .to_string(),
                },
            });
            return;
        }

        let Some(user) = input.username else {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Idle,
            });
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::UpdateUsername,
                    user: Ustr::from(""),
                    message: missing("username", "UpdateUsernameCommand"),
                },
            });
            return;
        };

        let Some(new_username) = input.new_username else {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::UpdateUsername,
                    user,
                    message: missing("new_username", "UpdateUsernameCommand"),
                },
            });
            return;
        };

        updater.set(InternalUsersActionCompute {
            state: InternalUsersActionState::InFlight {
                kind: InternalUsersActionKind::UpdateUsername,
                user,
            },
        });

        let user_str = user.as_str().to_string();
        let new_username_str = new_username.as_str().to_string();

        internal_users_api::update_username(
            &api_base_url,
            cf_token,
            &user_str,
            &new_username_str,
            move |result: internal_users_api::ApiResult<UpdateUsernameResponse>| match result {
                Ok(_resp) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Success {
                            kind: InternalUsersActionKind::UpdateUsername,
                            user,
                            data: None,
                        },
                    });
                }
                Err(err) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Error {
                            kind: InternalUsersActionKind::UpdateUsername,
                            user,
                            message: err.to_string(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Default, Debug)]
pub struct UpdateProfileCommand;

impl Command for UpdateProfileCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<InternalUsersActionInput>();
        let config = deps.get_state_ref::<BusinessConfig>();
        let cf_token = deps.get_state_ref::<CFTokenCompute>();

        let api_base_url = resolve_api_base_url(input, config);
        if api_base_url.trim().is_empty() {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::UpdateProfile,
                    user: input.username.unwrap_or_else(|| Ustr::from("")),
                    message:
                        "UpdateProfileCommand: missing api_base_url (set InternalUsersActionInput.api_base_url or BusinessConfig.api_base_url)"
                            .to_string(),
                },
            });
            return;
        }

        let Some(user) = input.username else {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::UpdateProfile,
                    user: Ustr::from(""),
                    message: missing("username", "UpdateProfileCommand"),
                },
            });
            return;
        };

        updater.set(InternalUsersActionCompute {
            state: InternalUsersActionState::InFlight {
                kind: InternalUsersActionKind::UpdateProfile,
                user,
            },
        });

        let user_str = user.as_str().to_string();
        let nickname = input.nickname.clone();
        let avatar_url = input.avatar_url.clone();

        internal_users_api::update_profile(
            &api_base_url,
            cf_token,
            &user_str,
            nickname,
            avatar_url,
            move |result: internal_users_api::ApiResult<UpdateProfileResponse>| match result {
                Ok(_resp) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Success {
                            kind: InternalUsersActionKind::UpdateProfile,
                            user,
                            data: None,
                        },
                    });
                }
                Err(err) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Error {
                            kind: InternalUsersActionKind::UpdateProfile,
                            user,
                            message: err.to_string(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Default, Debug)]
pub struct DeleteUserCommand;

impl Command for DeleteUserCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<InternalUsersActionInput>();
        let config = deps.get_state_ref::<BusinessConfig>();
        let cf_token = deps.get_state_ref::<CFTokenCompute>();

        let api_base_url = resolve_api_base_url(input, config);
        if api_base_url.trim().is_empty() {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::DeleteUser,
                    user: input.username.unwrap_or_else(|| Ustr::from("")),
                    message:
                        "DeleteUserCommand: missing api_base_url (set InternalUsersActionInput.api_base_url or BusinessConfig.api_base_url)"
                            .to_string(),
                },
            });
            return;
        }

        let Some(user) = input.username else {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::DeleteUser,
                    user: Ustr::from(""),
                    message: missing("username", "DeleteUserCommand"),
                },
            });
            return;
        };

        updater.set(InternalUsersActionCompute {
            state: InternalUsersActionState::InFlight {
                kind: InternalUsersActionKind::DeleteUser,
                user,
            },
        });

        let user_str = user.as_str().to_string();

        internal_users_api::delete_user(
            &api_base_url,
            cf_token,
            &user_str,
            move |result: internal_users_api::ApiResult<DeleteUserResponse>| match result {
                Ok(resp) => {
                    if resp.deleted {
                        updater.set(InternalUsersActionCompute {
                            state: InternalUsersActionState::Success {
                                kind: InternalUsersActionKind::DeleteUser,
                                user,
                                data: None,
                            },
                        });
                    } else {
                        updater.set(InternalUsersActionCompute {
                            state: InternalUsersActionState::Error {
                                kind: InternalUsersActionKind::DeleteUser,
                                user,
                                message: "User not found".to_string(),
                            },
                        });
                    }
                }
                Err(err) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Error {
                            kind: InternalUsersActionKind::DeleteUser,
                            user,
                            message: err.to_string(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Default, Debug)]
pub struct RevokeOtpCommand;

impl Command for RevokeOtpCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<InternalUsersActionInput>();
        let config = deps.get_state_ref::<BusinessConfig>();
        let cf_token = deps.get_state_ref::<CFTokenCompute>();

        let api_base_url = resolve_api_base_url(input, config);
        if api_base_url.trim().is_empty() {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::RevokeOtp,
                    user: input.username.unwrap_or_else(|| Ustr::from("")),
                    message:
                        "RevokeOtpCommand: missing api_base_url (set InternalUsersActionInput.api_base_url or BusinessConfig.api_base_url)"
                            .to_string(),
                },
            });
            return;
        }

        let Some(user) = input.username else {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::RevokeOtp,
                    user: Ustr::from(""),
                    message: missing("username", "RevokeOtpCommand"),
                },
            });
            return;
        };

        updater.set(InternalUsersActionCompute {
            state: InternalUsersActionState::InFlight {
                kind: InternalUsersActionKind::RevokeOtp,
                user,
            },
        });

        let user_str = user.as_str().to_string();

        internal_users_api::revoke_otp(
            &api_base_url,
            cf_token,
            &user_str,
            move |result: internal_users_api::ApiResult<RevokeOtpResponse>| match result {
                Ok(resp) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Success {
                            kind: InternalUsersActionKind::RevokeOtp,
                            user,
                            data: Some(resp.otpauth_url),
                        },
                    });
                }
                Err(err) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Error {
                            kind: InternalUsersActionKind::RevokeOtp,
                            user,
                            message: err.to_string(),
                        },
                    });
                }
            },
        );
    }
}

#[derive(Default, Debug)]
pub struct GetUserQrCommand;

impl Command for GetUserQrCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<InternalUsersActionInput>();
        let config = deps.get_state_ref::<BusinessConfig>();
        let cf_token = deps.get_state_ref::<CFTokenCompute>();

        let api_base_url = resolve_api_base_url(input, config);
        if api_base_url.trim().is_empty() {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::GetUserQr,
                    user: input.username.unwrap_or_else(|| Ustr::from("")),
                    message:
                        "GetUserQrCommand: missing api_base_url (set InternalUsersActionInput.api_base_url or BusinessConfig.api_base_url)"
                            .to_string(),
                },
            });
            return;
        }

        let Some(user) = input.username else {
            updater.set(InternalUsersActionCompute {
                state: InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::GetUserQr,
                    user: Ustr::from(""),
                    message: missing("username", "GetUserQrCommand"),
                },
            });
            return;
        };

        updater.set(InternalUsersActionCompute {
            state: InternalUsersActionState::InFlight {
                kind: InternalUsersActionKind::GetUserQr,
                user,
            },
        });

        let user_str = user.as_str().to_string();

        internal_users_api::get_user(
            &api_base_url,
            cf_token,
            &user_str,
            move |result: internal_users_api::ApiResult<GetUserResponse>| match result {
                Ok(resp) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Success {
                            kind: InternalUsersActionKind::GetUserQr,
                            user,
                            data: Some(resp.otpauth_url),
                        },
                    });
                }
                Err(err) => {
                    updater.set(InternalUsersActionCompute {
                        state: InternalUsersActionState::Error {
                            kind: InternalUsersActionKind::GetUserQr,
                            user,
                            message: err.to_string(),
                        },
                    });
                }
            },
        );
    }
}
