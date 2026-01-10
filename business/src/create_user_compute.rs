//! Create user command + compute cache.
//!
//! ## Why this file exists
//! Creating a user is a side effect (network IO). Side effects must **not** live in derived
//! computes because computes can run implicitly (startup, dirty propagation, etc).
//!
//! Instead, we expose:
//! - `CreateUserCompute`: a compute-shaped cache that stores the latest status/result
//! - `CreateUserCommand`: a manual-only command you explicitly dispatch, which performs
//!   the network request and updates `CreateUserCompute` via `Updater`.
//!
//! ## Auth (internal env / Zero Trust)
//! In internal environments protected by Cloudflare Zero Trust, `/internal/*` endpoints require
//! a token. This command will attach a `cf-authorization` header **if** a token is configured.
//!
//! ## How to use
//! 1) Register state/compute/command once during app setup:
//!    - `ctx.add_state(CreateUserInput::default());`
//!    - `ctx.record_compute(CreateUserCompute::default());`
//!    - `ctx.record_command(CreateUserCommand::default());`
//!
//! 2) When user clicks "Create":
//!    - `ctx.update::<CreateUserInput>(|s| s.username = Some("alice".into()));`
//!    - `ctx.dispatch::<CreateUserCommand>();`
//!    - later in your update loop: `ctx.sync_computes();`
//!
//! The command updates `CreateUserCompute` via `Updater::set`, so the normal
//! `StateCtx::sync_computes()` path will apply it.

use std::any::Any;

use crate::BusinessConfig;
use crate::cf_token_compute::CFTokenCompute;
use crate::internal::{CreateUserRequest, CreateUserResponse};

use collects_states::{
    Command, CommandSnapshot, Compute, ComputeDeps, Dep, SnapshotClone, State, Updater,
    assign_impl, state_assign_impl,
};
use log::{error, info};

/// State to hold inputs for user creation.
/// This is the "input" to the command.
///
/// Set this before dispatching `CreateUserCommand`.
#[derive(Default, Debug, Clone)]
pub struct CreateUserInput {
    /// The username to create. None means "no request intended".
    pub username: Option<String>,
}

impl SnapshotClone for CreateUserInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for CreateUserInput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Result of a user creation operation.
#[derive(Debug, Clone, Default)]
pub enum CreateUserResult {
    /// No creation attempted yet.
    #[default]
    Idle,
    /// Creation in progress.
    Pending,
    /// User created successfully.
    Success(CreateUserResponse),
    /// Creation failed with an error message.
    Error(String),
}

/// Compute-shaped cache for storing the latest create-user status/result.
///
/// This type is intentionally a `Compute` so it can be read via `ctx.cached::<CreateUserCompute>()`
/// and updated via `Updater::set(CreateUserCompute { ... })`.
///
/// Note: its `compute()` implementation is a deliberate no-op. Updates come from commands.
#[derive(Default, Debug, Clone)]
pub struct CreateUserCompute {
    /// The result of the last creation attempt.
    pub result: CreateUserResult,
}

impl SnapshotClone for CreateUserCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl CreateUserCompute {
    pub fn is_success(&self) -> bool {
        matches!(self.result, CreateUserResult::Success(_))
    }

    pub fn success_response(&self) -> Option<&CreateUserResponse> {
        if let CreateUserResult::Success(ref response) = self.result {
            Some(response)
        } else {
            None
        }
    }

    pub fn error_message(&self) -> Option<&str> {
        if let CreateUserResult::Error(ref msg) = self.result {
            Some(msg)
        } else {
            None
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.result, CreateUserResult::Pending)
    }

    pub fn reset(&mut self) {
        self.result = CreateUserResult::Idle;
    }
}

impl Compute for CreateUserCompute {
    fn deps(&self) -> ComputeDeps {
        // This is a cache updated by a command; it has no derived dependencies.
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // Intentionally no-op.
        //
        // Side effects (network) must not run inside a Compute due to implicit execution.
        // The command `CreateUserCommand` updates this compute via `Updater`.
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for CreateUserCompute {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Manual-only command that performs the create-user side effect.
///
/// Dispatch explicitly via `ctx.dispatch::<CreateUserCommand>()`.
#[derive(Default, Debug)]
pub struct CreateUserCommand;

impl Command for CreateUserCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: Updater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: CreateUserInput = snap.state::<CreateUserInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            let username = match &input.username {
                Some(name) if !name.trim().is_empty() => name.trim().to_string(),
                _ => {
                    info!("CreateUserCommand: No username set, skipping");
                    return;
                }
            };

            info!("CreateUserCommand: Creating user '{}'", username);

            // Update cache to pending immediately.
            updater.set(CreateUserCompute {
                result: CreateUserResult::Pending,
            });

            let url = format!("{}/internal/users", config.api_url().as_str());
            let body = match serde_json::to_vec(&CreateUserRequest {
                username: username.clone(),
            }) {
                Ok(body) => body,
                Err(e) => {
                    error!(
                        "CreateUserCommand: Failed to serialize CreateUserRequest: {}",
                        e
                    );
                    updater.set(CreateUserCompute {
                        result: CreateUserResult::Error(format!("Serialization error: {e}")),
                    });
                    return;
                }
            };

            let client = reqwest::Client::new();
            let mut request_builder = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body(body);

            // Cloudflare Zero Trust token (internal env):
            // If configured, attach it as `cf-authorization` so `/internal/*` routes pass middleware.
            if let Some(token) = cf_token.token() {
                request_builder = request_builder.header("cf-authorization", token);
            }

            match request_builder.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.as_u16() == 201 {
                        match response.bytes().await {
                            Ok(bytes) => {
                                match serde_json::from_slice::<CreateUserResponse>(&bytes) {
                                    Ok(create_response) => {
                                        info!(
                                            "CreateUserCommand: User '{}' created successfully",
                                            username
                                        );
                                        updater.set(CreateUserCompute {
                                            result: CreateUserResult::Success(create_response),
                                        });
                                    }
                                    Err(e) => {
                                        error!(
                                            "CreateUserCommand: Failed to parse CreateUserResponse: {}",
                                            e
                                        );
                                        updater.set(CreateUserCompute {
                                            result: CreateUserResult::Error(format!(
                                                "Parse error: {e}"
                                            )),
                                        });
                                    }
                                }
                            }
                            Err(e) => {
                                error!("CreateUserCommand: Failed to read response body: {}", e);
                                updater.set(CreateUserCompute {
                                    result: CreateUserResult::Error(format!(
                                        "Failed to read response: {e}"
                                    )),
                                });
                            }
                        }
                    } else {
                        let error_msg = format!("API returned status: {}", status);
                        error!("CreateUserCommand: {}", error_msg);
                        updater.set(CreateUserCompute {
                            result: CreateUserResult::Error(error_msg),
                        });
                    }
                }
                Err(err) => {
                    let error_msg = err.to_string();
                    error!("CreateUserCommand: Request failed: {}", error_msg);
                    updater.set(CreateUserCompute {
                        result: CreateUserResult::Error(error_msg),
                    });
                }
            }
        })
    }
}
