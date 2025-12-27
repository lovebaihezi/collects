mod api_status;
pub mod config;
pub mod version_info;

// Internal API module - only available for internal builds
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
pub mod internal_api;

pub use api_status::{APIAvailability, ApiStatus};
pub use config::BusinessConfig;

// Re-export internal API types for internal builds
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
pub use internal_api::{
    CreateUserRequest, CreateUserResponse, InternalAPIAvailability, InternalApiStatus,
    InternalUser, InternalUsers, ListUsersResponse, create_user,
};
