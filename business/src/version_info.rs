//! Version information for the application, populated at build time.
//!
//! Environment display format:
//! - PR: `pr:{number}` (number passed via env var at build time)
//! - Prod (stable): `stable:{version}`
//! - Nightly: `nightly:{date}`
//! - Internal: `internal:{commit}`
//! - Main/Test: `main:{commit}`

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
/// - PR: ("pr", "number") - number from PR_NUMBER env var or "unknown"
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
        format!("{}:{}", env_name, info)
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
}
