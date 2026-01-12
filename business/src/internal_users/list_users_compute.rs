//! Internal-users "list users" compute + refresh command.
//!
//! This follows the repo pattern used by `CreateUserCompute`:
//! - A compute-shaped cache (`InternalUsersListUsersCompute`) stores the latest status/result.
//! - A manual-only command (`RefreshInternalUsersCommand`) performs network IO and updates the
//!   compute via `Updater::set()`.
//!
//! UI should read the compute via `ctx.cached::<InternalUsersListUsersCompute>()` and dispatch
//! the command via `ctx.dispatch::<RefreshInternalUsersCommand>()`.
//!
//! NOTE: This file is intentionally self-contained so the UI can stop using `egui::Context`
//! memory (`ctx.memory_mut`) as an async message bus for the refresh flow.

use std::any::Any;

use collects_states::{
    Command, CommandSnapshot, Compute, ComputeDeps, Dep, LatestOnlyUpdater, SnapshotClone, State,
    Updater, assign_impl, state_assign_impl,
};
use ustr::Ustr;

use crate::BusinessConfig;
use crate::cf_token_compute::CFTokenCompute;
use crate::internal::InternalUserItem;
use crate::internal_users::api as internal_users_api;

/// Status/result of the internal-users list call.
#[derive(Debug, Clone, Default)]
pub enum InternalUsersListUsersResult {
    /// No request has been made yet (or the cache was reset).
    #[default]
    Idle,

    /// A refresh is currently in-flight.
    Loading,

    /// The last refresh succeeded with these users.
    Loaded(Vec<InternalUserItem>),

    /// The last refresh failed with this error message.
    Error(String),
}

/// Compute-shaped cache for listing internal users.
///
/// This is a `Compute` so UI can read it via `ctx.cached::<InternalUsersListUsersCompute>()`,
/// and a `State` so it can be recorded in `StateCtx` similarly to other compute-shaped caches.
#[derive(Debug, Clone, Default)]
pub struct InternalUsersListUsersCompute {
    pub result: InternalUsersListUsersResult,
}

impl SnapshotClone for InternalUsersListUsersCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl InternalUsersListUsersCompute {
    pub fn is_loading(&self) -> bool {
        matches!(self.result, InternalUsersListUsersResult::Loading)
    }

    pub fn error_message(&self) -> Option<&str> {
        match &self.result {
            InternalUsersListUsersResult::Error(msg) => Some(msg.as_str()),
            _ => None,
        }
    }

    pub fn users(&self) -> Option<&[InternalUserItem]> {
        match &self.result {
            InternalUsersListUsersResult::Loaded(users) => Some(users.as_slice()),
            _ => None,
        }
    }
}

impl Compute for InternalUsersListUsersCompute {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn deps(&self) -> ComputeDeps {
        // Cache updated by a command; no derived dependencies.
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // Intentionally no-op.
        //
        // Side effects (network) must not run inside a Compute due to implicit execution.
        // Dispatch `RefreshInternalUsersCommand` to update this compute via `Updater::set()`.
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for InternalUsersListUsersCompute {
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

/// Input state for refresh.
///
/// This allows the UI to set the base URL (or any future parameters) without
/// passing strings directly into the command callsite.
///
/// Pattern mirrors `CreateUserInput`.
#[derive(Default, Debug, Clone)]
pub struct InternalUsersListUsersInput {
    /// Base URL for the API (e.g. "https://example.com/api").
    ///
    /// Use `Ustr` to avoid repeated allocations/clones; this is frequently reused.
    pub api_base_url: Option<Ustr>,
}

impl SnapshotClone for InternalUsersListUsersInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for InternalUsersListUsersInput {
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

/// Manual-only command that refreshes the list of internal users.
///
/// Dispatch explicitly via `ctx.dispatch::<RefreshInternalUsersCommand>()`.
#[derive(Default, Debug)]
pub struct RefreshInternalUsersCommand;

impl Command for RefreshInternalUsersCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        // Read inputs/config.
        let input: InternalUsersListUsersInput =
            snap.state::<InternalUsersListUsersInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            // Determine base URL:
            // - Prefer explicit input when set (UI/tests can override).
            // - Fall back to `BusinessConfig::api_url()` (the canonical base for `/api`).
            let api_base_url: String = match input.api_base_url.as_ref() {
                Some(u) => u.as_str().to_string(),
                None => config.api_url().as_str().to_string(),
            };

            if api_base_url.trim().is_empty() {
                updater.set(InternalUsersListUsersCompute {
                    result: InternalUsersListUsersResult::Error(
                        "RefreshInternalUsersCommand: missing api_base_url (set InternalUsersListUsersInput.api_base_url or BusinessConfig.api_base_url)".to_string(),
                    ),
                });
                return;
            }

            // Set loading immediately.
            updater.set(InternalUsersListUsersCompute {
                result: InternalUsersListUsersResult::Loading,
            });

            // Kick off async request; update compute on completion.
            match internal_users_api::list_users(&api_base_url, &cf_token).await {
                Ok(users) => {
                    updater.set(InternalUsersListUsersCompute {
                        result: InternalUsersListUsersResult::Loaded(users),
                    });
                }
                Err(err) => {
                    updater.set(InternalUsersListUsersCompute {
                        result: InternalUsersListUsersResult::Error(err.to_string()),
                    });
                }
            }
        })
    }
}
