//! Integration tests for profile update feature.
//!
//! These tests verify the complete flow for updating user profile
//! through the UI by using kittest to control the egui interface
//! and wiremock to mock the API responses.
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use crate::common::TestCtx;
use kittest::Queryable;

mod common;

// ===========================================
// Integration tests using real CollectsApp
// ===========================================

/// Test that the profile columns are displayed in the users table.
#[tokio::test]
async fn test_profile_columns_displayed_in_table() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Verify profile-related column headers are displayed
    assert!(
        harness.query_by_label_contains("Nickname").is_some(),
        "Nickname column header should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Avatar").is_some(),
        "Avatar column header should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Created").is_some(),
        "Created At column header should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Updated").is_some(),
        "Updated At column header should be displayed"
    );
}

/// Test that the table has all expected column headers including profile fields.
#[tokio::test]
async fn test_table_has_all_column_headers() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Verify all column headers from the users table
    assert!(
        harness.query_by_label_contains("Username").is_some(),
        "Username column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("OTP Code").is_some(),
        "OTP Code column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Time Left").is_some(),
        "Time Left column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Nickname").is_some(),
        "Nickname column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Avatar").is_some(),
        "Avatar column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Created").is_some(),
        "Created At column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Updated").is_some(),
        "Updated At column should be displayed"
    );
    assert!(
        harness.query_by_label_contains("Actions").is_some(),
        "Actions column should be displayed"
    );
}

/// Test that refresh button is present for internal builds.
#[tokio::test]
async fn test_refresh_button_present() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    assert!(
        harness.query_by_label_contains("Refresh").is_some(),
        "Refresh button should be present"
    );
}

/// Test that mocked user data with profile fields is displayed in the table.
#[tokio::test]
async fn test_user_data_with_profile_fields_displayed() {
    let mut ctx = TestCtx::new_app_with_users().await;
    let harness = ctx.harness_mut();

    // Manually populate the users state with mocked data
    // This simulates what would happen after a successful API fetch
    {
        use collects_business::InternalUserItem;
        use collects_states::Time;
        use collects_ui::widgets::InternalUsersState;

        let users = vec![
            InternalUserItem {
                username: "alice".to_string(),
                current_otp: "123456".to_string(),
                time_remaining: 25,
                nickname: Some("Alice Wonderland".to_string()),
                avatar_url: Some("https://example.com/avatar/alice.png".to_string()),
                created_at: "2026-01-01T10:00:00Z".to_string(),
                updated_at: "2026-01-05T15:30:00Z".to_string(),
            },
            InternalUserItem {
                username: "bob".to_string(),
                current_otp: "654321".to_string(),
                time_remaining: 15,
                nickname: None,
                avatar_url: None,
                created_at: "2026-01-02T12:00:00Z".to_string(),
                updated_at: "2026-01-02T12:00:00Z".to_string(),
            },
        ];

        let now = *harness.state().state.ctx.state_mut::<Time>().as_ref();
        harness
            .state_mut()
            .state
            .ctx
            .state_mut::<InternalUsersState>()
            .update_users(users, now);
    }

    // Render to display the data
    for _ in 0..5 {
        harness.step();
    }

    // Verify mocked user data is displayed - check for username
    assert!(
        harness.query_by_label_contains("alice").is_some(),
        "Username 'alice' should be displayed in table"
    );

    // Verify profile fields are displayed
    assert!(
        harness
            .query_by_label_contains("Alice Wonderland")
            .is_some(),
        "Nickname 'Alice Wonderland' should be displayed"
    );

    assert!(
        harness.query_by_label_contains("bob").is_some(),
        "Username 'bob' should be displayed in table"
    );

    // Verify timestamps are displayed (they should be formatted)
    // Check for the formatted timestamp from the created_at field
    assert!(
        harness
            .query_by_label_contains("2026-01-01 10:00")
            .is_some(),
        "Created timestamp '2026-01-01 10:00' should be displayed in formatted form"
    );
}

/// Test that create user button is present for internal builds.
#[tokio::test]
async fn test_create_user_button_present() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    assert!(
        harness.query_by_label_contains("Create User").is_some(),
        "Create User button should be present"
    );
}

/// Test that internal builds show the users table (data-centric view).
#[tokio::test]
async fn test_internal_build_shows_users_table() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Internal builds should NOT show the login form
    assert!(
        harness.query_by_label_contains("Login").is_none(),
        "Login button should NOT be displayed for internal builds"
    );

    // Should show the users table with column headers
    assert!(
        harness.query_by_label_contains("Username").is_some(),
        "Username column should be displayed in internal builds"
    );
}
