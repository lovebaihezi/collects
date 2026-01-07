//! Cloudflare Access token state + compute cache.
//!
//! ## Why this file exists
//! Internal endpoints (`/internal/*`) are protected by Cloudflare Zero Trust in internal envs.
//! Those endpoints require a token, commonly passed via the `cf-authorization` header.
//!
//! We want a single source of truth for that token that:
//! - is easy to set manually (copy/paste) today
//! - can be replaced by PKCE flow later, without changing every internal command
//! - avoids side effects in derives (same pattern as `create_user_compute.rs`)
//!
//! This file provides:
//! - `CFTokenInput`: editable state (manual input)
//! - `CFTokenCompute`: compute-shaped cache of the effective token (trimmed/validated)
//! - `SetCFTokenCommand`: manual-only command that updates the compute cache
//!
//! ## How to use
//! 1) Register once during app setup:
//!    - `ctx.add_state(CFTokenInput::default());`
//!    - `ctx.record_compute(CFTokenCompute::default());`
//!    - `ctx.record_command(SetCFTokenCommand::default());`
//!
//! 2) When user sets token:
//!    - `ctx.update::<CFTokenInput>(|s| s.token = Some("...".into()));`
//!    - `ctx.dispatch::<SetCFTokenCommand>();`
//!    - later: `ctx.sync_computes();`
//!
//! 3) When calling internal endpoints, read via:
//!    - `ctx.cached::<CFTokenCompute>()`
//!      or from `Dep` in a command: `deps.get_state_ref::<CFTokenCompute>()`
//!    - attach header `cf-authorization: <token>` if `Some`.

use std::any::Any;

use collects_states::{
    Command, CommandSnapshot, Compute, ComputeDeps, Dep, SnapshotClone, State, Updater, assign_impl,
    state_assign_impl,
};
use log::info;

/// State for manually editing the Cloudflare Access token.
///
/// This is the *input* to `SetCFTokenCommand`.
#[derive(Default, Debug, Clone)]
pub struct CFTokenInput {
    /// Token string pasted by the user.
    ///
    /// - `None` means "no change intended / unset".
    /// - `Some("")` (or whitespace) will be treated as "clear token" by the command.
    pub token: Option<String>,
}

impl SnapshotClone for CFTokenInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for CFTokenInput {
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

/// Result/status of token setup.
#[derive(Debug, Clone, Default)]
pub enum CFTokenResult {
    /// No token assigned yet.
    #[default]
    Idle,
    /// Token is set and non-empty.
    Set(String),
}

impl CFTokenResult {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            CFTokenResult::Set(s) => Some(s.as_str()),
            CFTokenResult::Idle => None,
        }
    }

    pub fn is_set(&self) -> bool {
        matches!(self, CFTokenResult::Set(_))
    }
}

/// Compute-shaped cache for the effective token.
///
/// This is intentionally a `Compute` with a no-op `compute()` so it can be read through
/// the normal caching path and updated via `Updater::set(...)` from a command.
#[derive(Default, Debug, Clone)]
pub struct CFTokenCompute {
    pub result: CFTokenResult,
}

impl SnapshotClone for CFTokenCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl CFTokenCompute {
    /// Returns the token if set, otherwise `None`.
    pub fn token(&self) -> Option<&str> {
        self.result.as_str()
    }

    /// Clear any stored token.
    pub fn clear(&mut self) {
        self.result = CFTokenResult::Idle;
    }
}

impl Compute for CFTokenCompute {
    fn deps(&self) -> ComputeDeps {
        // Cache updated by a command; no derived dependencies.
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // Intentionally no-op.
        //
        // Token updates are explicit user actions handled by `SetCFTokenCommand`.
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for CFTokenCompute {
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

/// Manual-only command that sanitizes and applies token changes.
///
/// Dispatch explicitly via `ctx.dispatch::<SetCFTokenCommand>()`.
#[derive(Default, Debug)]
pub struct SetCFTokenCommand;

impl Command for SetCFTokenCommand {
    fn run(&self, snap: CommandSnapshot, updater: Updater) {
        let input: &CFTokenInput = snap.state();

        let token = input
            .token
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string);

        match token {
            Some(token) => {
                info!("SetCFTokenCommand: token set ({} chars)", token.len());
                updater.set(CFTokenCompute {
                    result: CFTokenResult::Set(token),
                });
            }
            None => {
                info!("SetCFTokenCommand: token cleared");
                updater.set(CFTokenCompute {
                    result: CFTokenResult::Idle,
                });
            }
        }
    }
}
