//! Route state for page navigation.
//!
//! This module defines the route enum that determines which page to display.

use collects_states::State;
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Represents the current page/route of the application.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Route {
    /// Login page - shown when user is not authenticated
    #[default]
    Login,
    /// Home page - shown when user is authenticated (non-internal builds)
    Home,
    /// Internal page - shown when user is authenticated (internal builds only)
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    Internal,
}

impl State for Route {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
