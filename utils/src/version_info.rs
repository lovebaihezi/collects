//! Version information for the application, populated at build time.
//!
//! Environment display format:
//! - PR: `pr:{number}` (number passed via env var at build time)
//! - Prod (stable): `stable:{version}`
//! - Nightly: `nightly:{date}`
//! - Internal: `internal:{commit}`
//! - Main/Test: `main:{commit}`
//!
//! This module supports both compile-time feature-based environment detection (for UI)
//! and runtime environment detection (for services).

/// Runtime environment enum for services that determine environment at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeEnv {
    /// Local development
    Local,
    /// Production
    Prod,
    /// Internal testing
    Internal,
    /// Test environment
    Test,
    /// Test-internal environment
    TestInternal,
    /// Pull request preview
    Pr,
    /// Nightly build
    Nightly,
}

/// Get the build date in RFC3339 format
pub fn build_date() -> &'static str {
    env!("BUILD_DATE")
}

/// Get the git commit hash (short)
pub fn build_commit() -> &'static str {
    env!("BUILD_COMMIT")
}

/// Get the package version
pub fn build_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Returns the environment label and version/info string based on build features.
///
/// Format: `(env_name, info_string)`
/// - PR: ("pr", "number") - number from `PR_NUMBER` env var or "unknown"
/// - Prod: ("stable", "version")
/// - Nightly: ("nightly", "date")
/// - Internal: ("internal", "commit")
/// - Test/Main: ("main", "commit")
pub fn env_version_info() -> (&'static str, &'static str) {
    if cfg!(feature = "env_pr") {
        ("pr", option_env!("PR_NUMBER").unwrap_or("unknown"))
    } else if cfg!(feature = "env_nightly") {
        ("nightly", build_date())
    } else if cfg!(feature = "env_internal") {
        ("internal", build_commit())
    } else if cfg!(feature = "env_test_internal") {
        ("test-internal", build_commit())
    } else if cfg!(feature = "env_test") {
        ("main", build_commit())
    } else {
        // Production (stable)
        ("stable", build_version())
    }
}

/// Format the environment and version info as a display string.
pub fn format_env_version() -> String {
    let (env_name, info) = env_version_info();
    // For nightly, extract just the date portion
    if env_name == "nightly" && info.len() >= 10 {
        format!("{}:{}", env_name, &info[..10])
    } else {
        format!("{env_name}:{info}")
    }
}

/// Format version string for a runtime-determined environment.
///
/// This is used by services that determine their environment at runtime
/// rather than compile time. Uses build-time constants for commit/date/version.
///
/// Format: `{env}:{info}` where:
/// - PR: `pr:{pr_number}` (number from `PR_NUMBER` env var at build time)
/// - Nightly: `nightly:{date}` (first 10 chars of build date)
/// - Internal: `internal:{commit}`
/// - Test-Internal: `test-internal:{commit}`
/// - Test/Local: `main:{commit}`
/// - Prod: `stable:{version}`
pub fn format_version_for_runtime_env(env: RuntimeEnv) -> String {
    match env {
        RuntimeEnv::Pr => {
            let pr_number = option_env!("PR_NUMBER").unwrap_or("unknown");
            format!("pr:{pr_number}")
        }
        RuntimeEnv::Nightly => {
            let date = build_date();
            // Extract just the date portion (first 10 chars) from RFC3339 format
            // BUILD_DATE is RFC3339 formatted (e.g., "2026-01-03T12:00:00+00:00") which is ASCII
            let date_part = if date.len() >= 10 && date.is_ascii() {
                &date[..10]
            } else {
                date
            };
            format!("nightly:{date_part}")
        }
        RuntimeEnv::Internal => format!("internal:{}", build_commit()),
        RuntimeEnv::TestInternal => format!("test-internal:{}", build_commit()),
        RuntimeEnv::Test | RuntimeEnv::Local => format!("main:{}", build_commit()),
        RuntimeEnv::Prod => format!("stable:{}", build_version()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_date_not_empty() {
        assert!(!build_date().is_empty());
    }

    #[test]
    fn test_build_commit_not_empty() {
        assert!(!build_commit().is_empty());
    }

    #[test]
    fn test_build_version_not_empty() {
        assert!(!build_version().is_empty());
    }

    #[test]
    fn test_env_version_info_format() {
        let (env_name, info) = env_version_info();
        assert!(!env_name.is_empty());
        assert!(!info.is_empty());
    }

    #[test]
    fn test_format_env_version() {
        let formatted = format_env_version();
        assert!(formatted.contains(':'));
    }

    #[test]
    fn test_format_version_for_runtime_env_local() {
        let version = format_version_for_runtime_env(RuntimeEnv::Local);
        assert!(version.starts_with("main:"));
    }

    #[test]
    fn test_format_version_for_runtime_env_test() {
        let version = format_version_for_runtime_env(RuntimeEnv::Test);
        assert!(version.starts_with("main:"));
    }

    #[test]
    fn test_format_version_for_runtime_env_prod() {
        let version = format_version_for_runtime_env(RuntimeEnv::Prod);
        assert!(version.starts_with("stable:"));
    }

    #[test]
    fn test_format_version_for_runtime_env_internal() {
        let version = format_version_for_runtime_env(RuntimeEnv::Internal);
        assert!(version.starts_with("internal:"));
    }

    #[test]
    fn test_format_version_for_runtime_env_test_internal() {
        let version = format_version_for_runtime_env(RuntimeEnv::TestInternal);
        assert!(version.starts_with("test-internal:"));
    }

    #[test]
    fn test_format_version_for_runtime_env_nightly() {
        let version = format_version_for_runtime_env(RuntimeEnv::Nightly);
        assert!(version.starts_with("nightly:"));
    }

    #[test]
    fn test_format_version_for_runtime_env_pr() {
        let version = format_version_for_runtime_env(RuntimeEnv::Pr);
        assert!(version.starts_with("pr:"));
    }
}
