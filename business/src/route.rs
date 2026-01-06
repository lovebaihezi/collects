//! Route state for page navigation.
//!
//! This module defines the route enum that determines which page to display.

use collects_states::{State, state_assign_impl};
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

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_default_is_login() {
        let route = Route::default();
        assert_eq!(route, Route::Login);
    }

    #[test]
    fn test_route_clone() {
        let route = Route::Home;
        let cloned = route.clone();
        assert_eq!(cloned, Route::Home);
    }

    #[test]
    fn test_route_equality() {
        assert_eq!(Route::Login, Route::Login);
        assert_eq!(Route::Home, Route::Home);
        assert_ne!(Route::Login, Route::Home);
    }

    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    #[test]
    fn test_internal_route() {
        let route = Route::Internal;
        assert_eq!(route, Route::Internal);
        assert_ne!(route, Route::Home);
        assert_ne!(route, Route::Login);
    }
}
