//! Create user compute.
//!
//! This module provides a compute for creating internal users.
//! It only runs when triggered (marked dirty) and stores the result.

use std::any::{Any, TypeId};

use crate::BusinessConfig;
use crate::internal::{CreateUserRequest, CreateUserResponse};
use collects_states::{Compute, ComputeDeps, Dep, State, Updater, assign_impl};
use log::{error, info};

/// State to hold the username for user creation.
/// Set this before marking CreateUserCompute as dirty.
#[derive(Default, Debug, Clone)]
pub struct CreateUserInput {
    /// The username to create. None means no pending creation.
    pub username: Option<String>,
}

impl State for CreateUserInput {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
    /// Creation failed with error message.
    Error(String),
}

/// Compute for creating internal users.
///
/// This compute does not auto-run. To trigger user creation:
/// 1. Set the username in `CreateUserInput` state
/// 2. Mark this compute as dirty using `ctx.mark_dirty(&TypeId::of::<CreateUserCompute>())`
///
/// The result will be stored in this compute and can be accessed via `cached::<CreateUserCompute>()`.
#[derive(Default, Debug)]
pub struct CreateUserCompute {
    /// The result of the last creation attempt.
    pub result: CreateUserResult,
}

impl CreateUserCompute {
    /// Check if the result is a success.
    pub fn is_success(&self) -> bool {
        matches!(self.result, CreateUserResult::Success(_))
    }

    /// Get the success response if available.
    pub fn success_response(&self) -> Option<&CreateUserResponse> {
        if let CreateUserResult::Success(ref response) = self.result {
            Some(response)
        } else {
            None
        }
    }

    /// Get the error message if available.
    pub fn error_message(&self) -> Option<&str> {
        if let CreateUserResult::Error(ref msg) = self.result {
            Some(msg)
        } else {
            None
        }
    }

    /// Check if creation is pending.
    pub fn is_pending(&self) -> bool {
        matches!(self.result, CreateUserResult::Pending)
    }

    /// Reset to idle state.
    pub fn reset(&mut self) {
        self.result = CreateUserResult::Idle;
    }
}

impl Compute for CreateUserCompute {
    fn deps(&self) -> ComputeDeps {
        const IDS: [TypeId; 2] = [
            TypeId::of::<CreateUserInput>(),
            TypeId::of::<BusinessConfig>(),
        ];
        (&IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<CreateUserInput>();
        let config = deps.get_state_ref::<BusinessConfig>();

        // Only proceed if there's a username to create
        let username = match &input.username {
            Some(name) if !name.is_empty() => name.clone(),
            _ => {
                info!("CreateUserCompute: No username set, skipping");
                return;
            }
        };

        info!("CreateUserCompute: Creating user '{}'", username);

        // Set to pending immediately
        updater.set(CreateUserCompute {
            result: CreateUserResult::Pending,
        });

        let url = format!("{}/internal/users", config.api_url().as_str());
        let body = match serde_json::to_vec(&CreateUserRequest {
            username: username.clone(),
        }) {
            Ok(body) => body,
            Err(e) => {
                error!("Failed to serialize CreateUserRequest: {}", e);
                updater.set(CreateUserCompute {
                    result: CreateUserResult::Error(format!("Serialization error: {e}")),
                });
                return;
            }
        };

        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");

        ehttp::fetch(request, move |result| match result {
            Ok(response) => {
                if response.status == 201 {
                    match serde_json::from_slice::<CreateUserResponse>(&response.bytes) {
                        Ok(create_response) => {
                            info!(
                                "CreateUserCompute: User '{}' created successfully",
                                username
                            );
                            updater.set(CreateUserCompute {
                                result: CreateUserResult::Success(create_response),
                            });
                        }
                        Err(e) => {
                            error!("Failed to parse CreateUserResponse: {}", e);
                            updater.set(CreateUserCompute {
                                result: CreateUserResult::Error(format!("Parse error: {e}")),
                            });
                        }
                    }
                } else {
                    let error_msg = format!("API returned status: {}", response.status);
                    error!("CreateUserCompute: {}", error_msg);
                    updater.set(CreateUserCompute {
                        result: CreateUserResult::Error(error_msg),
                    });
                }
            }
            Err(err) => {
                let error_msg = err.to_string();
                error!("CreateUserCompute: Request failed: {}", error_msg);
                updater.set(CreateUserCompute {
                    result: CreateUserResult::Error(error_msg),
                });
            }
        });
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for CreateUserCompute {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
