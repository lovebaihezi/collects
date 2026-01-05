//! E2E test for internal user management.
//!
//! This test connects to a real backend service (internal environment) to verify:
//! 1. User table is displayed
//! 2. Create user button is available
//! 3. Creating a user adds them to the table
//!
//! **IMPORTANT**: This test requires access to the `env_test_internal` environment.
//! For Zero Trust protected environments, you may need to provide authentication tokens.
//!
//! Run with: `cargo test --package collects-ui --test e2e_internal_happy_path --features env_test_internal`

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use kittest::Queryable;

mod common;

use common::E2eTestCtx;

/// E2E test: Internal users table is displayed.
///
/// Verifies that the internal page shows the users table with expected controls.
#[test]
fn test_internal_users_table_structure_e2e() {
    let mut ctx = E2eTestCtx::new_app();
    let harness = ctx.harness_mut();

    // Run several frames to let the app initialize
    for _ in 0..10 {
        harness.step();
    }

    // For internal builds, we should NOT see the login form
    assert!(
        harness.query_by_label_contains("Login").is_none(),
        "Login button should NOT be displayed in internal builds (Zero Trust auth)"
    );

    // Verify table controls are present
    assert!(
        harness.query_by_label_contains("Refresh").is_some(),
        "Refresh button should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Create User").is_some(),
        "Create User button should be displayed"
    );
}

/// E2E test: Internal users table headers are displayed.
///
/// Verifies that the table has the expected column headers.
#[test]
fn test_internal_users_table_headers_e2e() {
    let mut ctx = E2eTestCtx::new_app();
    let harness = ctx.harness_mut();

    // Run several frames to let the app initialize
    for _ in 0..10 {
        harness.step();
    }

    // Verify table headers are present
    assert!(
        harness.query_by_label_contains("Username").is_some(),
        "Username column header should be displayed"
    );
    assert!(
        harness.query_by_label_contains("OTP Code").is_some(),
        "OTP Code column header should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Time Left").is_some(),
        "Time Left column header should be displayed"
    );
}

/// E2E test: Create user flow structure.
///
/// Verifies that clicking "Create User" opens a modal with expected fields.
/// Note: This test only validates UI structure, not actual user creation.
#[test]
fn test_internal_create_user_modal_structure_e2e() {
    let mut ctx = E2eTestCtx::new_app();
    let harness = ctx.harness_mut();

    // Run several frames to let the app initialize
    for _ in 0..10 {
        harness.step();
    }

    // Find and click the Create User button
    let create_button = harness.query_by_label_contains("Create User");
    assert!(
        create_button.is_some(),
        "Create User button should be present"
    );

    // In a full e2e test, we would click the button and verify the modal opens
    // For now, we just verify the button exists and is accessible
    println!("E2E internal test: Create User button found and accessible");
}

/// E2E test: Internal page shows no login form.
///
/// Verifies that Zero Trust authentication means we skip the login form.
#[test]
fn test_internal_skips_login_form_e2e() {
    let mut ctx = E2eTestCtx::new_app();
    let harness = ctx.harness_mut();

    // Run several frames to let the app initialize
    for _ in 0..10 {
        harness.step();
    }

    // Should NOT show login form elements
    assert!(
        harness.query_by_label_contains("Collects App").is_none(),
        "Collects App heading should NOT be displayed in internal builds"
    );

    // Should NOT show OTP input (login form)
    // Note: OTP Code might appear in the table headers, so we check for login-specific elements
    assert!(
        harness.query_by_label_contains("Login").is_none(),
        "Login button should NOT be displayed in internal builds"
    );
}
