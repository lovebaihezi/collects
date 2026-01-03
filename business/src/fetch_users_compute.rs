//! Fetch internal users command + compute cache.
//!
//! ## Why this file exists
//! Fetching users is a side effect (network IO). Side effects must **not** live in derived
//! computes because computes can run implicitly (startup, dirty propagation, etc).
//!
//! Instead, we expose:
//! - `FetchInternalUsersCompute`: a compute-shaped cache that stores the latest users list
//! - `FetchInternalUsersCommand`: a manual-only command you explicitly dispatch, which performs
//!   the network request and updates `FetchInternalUsersCompute` via `Updater`.
//!
//! ## How to use
//! 1) Register state/compute/command once during app setup:
//!    - `ctx.record_compute(FetchInternalUsersCompute::default());`
//!    - `ctx.record_command(FetchInternalUsersCommand);`
//!
//! 2) Dispatch at app startup or on user action (refresh button):
//!    - `ctx.dispatch::<FetchInternalUsersCommand>();`
//!    - later in your update loop: `ctx.sync_computes();`
//!
//! The command updates `FetchInternalUsersCompute` via `Updater::set`, so the normal
//! `StateCtx::sync_computes()` path will apply it.

use std::any::Any;

use crate::BusinessConfig;
use crate::internal::{InternalUserItem, ListUsersResponse};

use collects_states::{Command, Compute, ComputeDeps, Dep, Updater, assign_impl};
use log::{error, info};

/// Result of fetching internal users.
#[derive(Debug, Clone, Default)]
pub enum FetchUsersResult {
    /// No fetch attempted yet.
    #[default]
    Idle,
    /// Fetch in progress.
    Pending,
    /// Users fetched successfully.
    Success(Vec<InternalUserItem>),
    /// Fetch failed with an error message.
    Error(String),
}

/// Compute-shaped cache for storing the fetched users.
///
/// This type is intentionally a `Compute` so it can be read via `ctx.cached::<FetchInternalUsersCompute>()`
/// and updated via `Updater::set(FetchInternalUsersCompute { ... })`.
///
/// Note: its `compute()` implementation is a deliberate no-op. Updates come from commands.
#[derive(Default, Debug)]
pub struct FetchInternalUsersCompute {
    /// The result of the last fetch attempt.
    pub result: FetchUsersResult,
}

impl FetchInternalUsersCompute {
    /// Returns true if users were fetched successfully.
    pub fn is_success(&self) -> bool {
        matches!(self.result, FetchUsersResult::Success(_))
    }

    /// Returns the list of users if fetch was successful.
    pub fn users(&self) -> Option<&Vec<InternalUserItem>> {
        if let FetchUsersResult::Success(ref users) = self.result {
            Some(users)
        } else {
            None
        }
    }

    /// Returns the error message if fetch failed.
    pub fn error_message(&self) -> Option<&str> {
        if let FetchUsersResult::Error(ref msg) = self.result {
            Some(msg)
        } else {
            None
        }
    }

    /// Returns true if fetch is in progress.
    pub fn is_pending(&self) -> bool {
        matches!(self.result, FetchUsersResult::Pending)
    }

    /// Returns true if no fetch has been attempted yet.
    pub fn is_idle(&self) -> bool {
        matches!(self.result, FetchUsersResult::Idle)
    }
}

impl Compute for FetchInternalUsersCompute {
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
        // The command `FetchInternalUsersCommand` updates this compute via `Updater`.
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

/// Manual-only command that fetches internal users.
///
/// Dispatch explicitly via `ctx.dispatch::<FetchInternalUsersCommand>()`.
#[derive(Default, Debug)]
pub struct FetchInternalUsersCommand;

impl Command for FetchInternalUsersCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let config = deps.get_state_ref::<BusinessConfig>();

        info!("FetchInternalUsersCommand: Fetching internal users");

        // Update cache to pending immediately.
        updater.set(FetchInternalUsersCompute {
            result: FetchUsersResult::Pending,
        });

        let url = format!("{}/internal/users", config.api_url().as_str());
        let request = ehttp::Request::get(&url);

        // Perform the IO side effect and update the compute cache with the result.
        ehttp::fetch(request, move |result| match result {
            Ok(response) => {
                if response.status == 200 {
                    match serde_json::from_slice::<ListUsersResponse>(&response.bytes) {
                        Ok(list_response) => {
                            info!(
                                "FetchInternalUsersCommand: Fetched {} users successfully",
                                list_response.users.len()
                            );
                            updater.set(FetchInternalUsersCompute {
                                result: FetchUsersResult::Success(list_response.users),
                            });
                        }
                        Err(e) => {
                            error!(
                                "FetchInternalUsersCommand: Failed to parse ListUsersResponse: {}",
                                e
                            );
                            updater.set(FetchInternalUsersCompute {
                                result: FetchUsersResult::Error(format!("Parse error: {e}")),
                            });
                        }
                    }
                } else {
                    let error_msg = format!("API returned status: {}", response.status);
                    error!("FetchInternalUsersCommand: {}", error_msg);
                    updater.set(FetchInternalUsersCompute {
                        result: FetchUsersResult::Error(error_msg),
                    });
                }
            }
            Err(err) => {
                let error_msg = err.to_string();
                error!("FetchInternalUsersCommand: Request failed: {}", error_msg);
                updater.set(FetchInternalUsersCompute {
                    result: FetchUsersResult::Error(error_msg),
                });
            }
        });
    }
}
