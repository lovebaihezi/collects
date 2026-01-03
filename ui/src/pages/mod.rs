//! Pages module for the application.
//!
//! This module contains the different pages that can be displayed based on the route:
//! - `login_page`: Login form for unauthenticated users
//! - `home_page`: Main content for authenticated users (non-internal builds)
//! - `internal_page`: Internal users management for authenticated users (internal builds)

mod home_page;
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
mod internal_page;
mod login_page;

pub use home_page::home_page;
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
pub use internal_page::internal_page;
pub use login_page::login_page;
