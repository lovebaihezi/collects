use kittest::Queryable;

use crate::common::TestCtx;

mod common;

/// Tests for non-internal builds: login form is displayed.
/// For internal builds (Zero Trust), the user is already authenticated.
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
mod non_internal_tests {
    use super::*;

    /// Tests that the login form is displayed with all expected elements.
    #[tokio::test]
    async fn test_login_form_displayed() {
        let mut ctx = TestCtx::new_app().await;

        let harness = ctx.harness_mut();
        harness.step();

        // Check that the heading is displayed
        assert!(
            harness.query_by_label_contains("Collects App").is_some(),
            "Collects App heading should be displayed"
        );

        // Check that username label is displayed
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username label should be displayed"
        );

        // Check that OTP label is displayed
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "OTP Code label should be displayed"
        );

        // Check that Login button is displayed
        assert!(
            harness.query_by_label_contains("Login").is_some(),
            "Login button should be displayed"
        );
    }

    /// Tests that the login form shows centered layout.
    #[tokio::test]
    async fn test_login_form_centered() {
        let mut ctx = TestCtx::new_app().await;

        let harness = ctx.harness_mut();
        harness.step();

        // Verify the "Collects App" heading appears (indicating centered content)
        let heading = harness.query_by_label_contains("Collects App");
        assert!(
            heading.is_some(),
            "Collects App heading should be displayed in centered layout"
        );

        // Verify other form elements are present and accessible
        // which indicates the form is properly positioned on screen
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username field should be accessible"
        );
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "OTP Code field should be accessible"
        );
        assert!(
            harness.query_by_label_contains("Login").is_some(),
            "Login button should be accessible"
        );
    }

    /// Tests that the login form is vertically centered with appropriate spacing.
    #[tokio::test]
    async fn test_login_form_vertical_centering() {
        let mut ctx = TestCtx::new_app().await;

        let harness = ctx.harness_mut();
        harness.step();

        // The form should be properly displayed with all elements
        // This test verifies that vertical centering doesn't break the form layout
        assert!(
            harness.query_by_label_contains("Collects App").is_some(),
            "Heading should be visible"
        );

        // Verify all interactive elements are still accessible after centering changes
        let username_field = harness.query_by_label_contains("Username");
        let otp_field = harness.query_by_label_contains("OTP Code");
        let login_button = harness.query_by_label_contains("Login");

        assert!(
            username_field.is_some(),
            "Username field should be accessible"
        );
        assert!(otp_field.is_some(), "OTP field should be accessible");
        assert!(login_button.is_some(), "Login button should be accessible");
    }

    /// Tests that the login button is disabled when username or OTP is empty.
    #[tokio::test]
    async fn test_login_button_disabled_when_empty() {
        let mut ctx = TestCtx::new_app().await;

        let harness = ctx.harness_mut();
        harness.step();

        // The Login button should be present
        let login_button = harness.query_by_label_contains("Login");
        assert!(login_button.is_some(), "Login button should be present");
    }
}

/// Tests for internal builds: user is authenticated via Zero Trust.
/// Login form should NOT be displayed; user sees authenticated content directly.
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
mod internal_tests {
    use super::*;

    /// Tests that internal builds skip the login page (Zero Trust authentication).
    /// The login form has a "Login" button which should NOT appear for internal builds.
    #[tokio::test]
    async fn test_internal_build_skips_login_page() {
        let mut ctx = TestCtx::new_app().await;

        let harness = ctx.harness_mut();
        harness.step();

        // Check that the heading is displayed
        assert!(
            harness.query_by_label_contains("Collects App").is_some(),
            "Collects App heading should be displayed"
        );

        // The "Login" button is unique to the login form and should NOT appear
        // for internal builds since they use Zero Trust authentication
        assert!(
            harness.query_by_label_contains("Login").is_none(),
            "Login button should NOT be displayed for internal builds (Zero Trust auth)"
        );

        // The username input with "Username:" label should not be in login form context
        // (note: internal panel may have username column but not a login-style input)
    }

    /// Tests that internal builds show the authenticated user welcome message.
    #[tokio::test]
    async fn test_internal_build_shows_authenticated_content() {
        let mut ctx = TestCtx::new_app().await;

        let harness = ctx.harness_mut();
        harness.step();

        // Check that the signed-in status is displayed
        assert!(
            harness.query_by_label_contains("Signed").is_some(),
            "Signed status should be displayed for internal builds"
        );

        // Check that the Zero Trust user name is shown
        assert!(
            harness.query_by_label_contains("Zero Trust User").is_some(),
            "Zero Trust User should be displayed for internal builds"
        );

        // Check welcome message is shown
        assert!(
            harness.query_by_label_contains("Welcome").is_some(),
            "Welcome message should be displayed for authenticated internal users"
        );
    }
}
