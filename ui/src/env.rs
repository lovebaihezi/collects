//! Build-time environment configuration using Cargo features.
//!
//! This module provides compile-time configuration based on Cargo features:
//! - `prod` - Production environment
//! - `test` - Testing environment
//! - `dev` - Development environment (default)
//!
//! URLs are configured at compile time with zero runtime overhead.

// ============================================================================
// Environment Detection Macros
// ============================================================================

/// Check if building for production environment
#[macro_export]
macro_rules! env_prod {
    () => {
        cfg!(feature = "prod")
    };
}

/// Check if building for test environment
#[macro_export]
macro_rules! env_test {
    () => {
        cfg!(feature = "test")
    };
}

/// Check if building for development environment
#[macro_export]
macro_rules! env_dev {
    () => {
        !cfg!(feature = "prod") && !cfg!(feature = "test")
    };
}

/// Check if building for WASM target
#[macro_export]
macro_rules! target_wasm {
    () => {
        cfg!(target_arch = "wasm32")
    };
}

/// Check if building for native target
#[macro_export]
macro_rules! target_native {
    () => {
        cfg!(not(target_arch = "wasm32"))
    };
}

// ============================================================================
// URL Configuration
// ============================================================================

// Production URLs
#[cfg(feature = "prod")]
pub const BASE_URL: &str = "https://collects.lqxclqxc.com";

#[cfg(feature = "prod")]
pub const REAL_URL: &str = "https://collects-api-145756646168.us-east1.run.app";

// Test URLs
#[cfg(feature = "test")]
pub const BASE_URL: &str = "https://collects-internal.lqxclqxc.com";

#[cfg(feature = "test")]
pub const REAL_URL: &str = "https://collects-api-145756646168.us-east1.run.app";

// Development URLs (default)
#[cfg(not(any(feature = "prod", feature = "test")))]
pub const BASE_URL: &str = "http://localhost:7788";

#[cfg(not(any(feature = "prod", feature = "test")))]
pub const REAL_URL: &str = "http://localhost:7788";

// ============================================================================
// Environment Name
// ============================================================================

#[cfg(feature = "prod")]
pub const ENVIRONMENT: &str = "prod";

#[cfg(feature = "test")]
pub const ENVIRONMENT: &str = "test";

#[cfg(not(any(feature = "prod", feature = "test")))]
pub const ENVIRONMENT: &str = "dev";

// ============================================================================
// Build Type
// ============================================================================

#[cfg(target_arch = "wasm32")]
pub const BUILD_TYPE: &str = "wasm";

#[cfg(not(target_arch = "wasm32"))]
pub const BUILD_TYPE: &str = "native";

// ============================================================================
// Helper Functions
// ============================================================================

/// Returns true if this is a production build
#[inline]
pub const fn is_prod() -> bool {
    cfg!(feature = "prod")
}

/// Returns true if this is a test build
#[inline]
pub const fn is_test() -> bool {
    cfg!(feature = "test")
}

/// Returns true if this is a development build
#[inline]
pub const fn is_dev() -> bool {
    !cfg!(feature = "prod") && !cfg!(feature = "test")
}

/// Returns true if this is a WASM build
#[inline]
pub const fn is_wasm() -> bool {
    cfg!(target_arch = "wasm32")
}

/// Returns true if this is a native build
#[inline]
pub const fn is_native() -> bool {
    !cfg!(target_arch = "wasm32")
}

/// Creates an `AppEnv` instance based on the current build configuration.
///
/// # Examples
///
/// ```rust,ignore
/// use collects_ui::env::create_app_env;
///
/// let app_env = create_app_env();
/// ```
pub fn create_app_env() -> collects_business::AppEnv {
    use collects_business::AppEnv;

    #[cfg(all(target_arch = "wasm32", feature = "prod"))]
    return AppEnv::web_prod();

    #[cfg(all(target_arch = "wasm32", feature = "test"))]
    return AppEnv::web_test();

    #[cfg(all(target_arch = "wasm32", not(any(feature = "prod", feature = "test"))))]
    return AppEnv::web_local();

    #[cfg(all(not(target_arch = "wasm32"), feature = "prod"))]
    return AppEnv::native_prod();

    #[cfg(all(not(target_arch = "wasm32"), feature = "test"))]
    return AppEnv::native_test();

    #[cfg(all(
        not(target_arch = "wasm32"),
        not(any(feature = "prod", feature = "test"))
    ))]
    return AppEnv::native_local();
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_is_valid() {
        assert!(
            ENVIRONMENT == "prod" || ENVIRONMENT == "test" || ENVIRONMENT == "dev",
            "ENVIRONMENT must be 'prod', 'test', or 'dev'"
        );
    }

    #[test]
    fn test_build_type_is_valid() {
        assert!(
            BUILD_TYPE == "wasm" || BUILD_TYPE == "native",
            "BUILD_TYPE must be 'wasm' or 'native'"
        );
    }

    #[test]
    fn test_urls_not_empty() {
        assert!(!BASE_URL.is_empty(), "BASE_URL should not be empty");
        assert!(!REAL_URL.is_empty(), "REAL_URL should not be empty");
    }

    #[test]
    fn test_helper_functions() {
        // Only one environment should be true
        let env_count = [is_prod(), is_test(), is_dev()]
            .iter()
            .filter(|&&x| x)
            .count();
        assert_eq!(env_count, 1, "Exactly one environment should be active");

        // Only one build type should be true
        assert_ne!(is_wasm(), is_native(), "Build type should be exclusive");
    }

    #[test]
    fn test_create_app_env() {
        use collects_business::{AppType, EnvType};

        let app_env = create_app_env();

        // Verify app type matches build type
        if is_wasm() {
            assert_eq!(app_env.app_type(), AppType::Web);
        } else {
            assert_eq!(app_env.app_type(), AppType::Native);
        }

        // Verify env type matches environment
        if is_prod() {
            assert_eq!(app_env.env_type(), EnvType::Prod);
        } else if is_test() {
            assert_eq!(app_env.env_type(), EnvType::Test);
        } else {
            assert_eq!(app_env.env_type(), EnvType::Local);
        }
    }

    #[test]
    fn test_macros() {
        // Test that macros compile and return bool
        let _: bool = env_prod!();
        let _: bool = env_test!();
        let _: bool = env_dev!();
        let _: bool = target_wasm!();
        let _: bool = target_native!();
    }
}
