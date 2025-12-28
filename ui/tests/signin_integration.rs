use kittest::Queryable;

use crate::common::TestCtx;

mod common;

#[tokio::test]
async fn test_signin_button_visible_in_app() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();
    harness.step();

    assert!(
        harness.query_by_label("Sign In").is_some(),
        "'Sign In' button should be visible in the app"
    );
}

#[tokio::test]
async fn test_signin_message_visible_when_logged_out() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();
    harness.step();

    assert!(
        harness.query_by_label("Sign in to access your collections.").is_some(),
        "'Sign in to access your collections.' message should be visible"
    );
}

#[tokio::test]
async fn test_signin_flow_with_valid_credentials() {
    let mut ctx = TestCtx::new_app_with_auth("testuser", "123456").await;

    let harness = ctx.harness_mut();
    harness.step();

    // Verify Sign In button exists
    assert!(
        harness.query_by_label("Sign In").is_some(),
        "'Sign In' button should be visible initially"
    );

    // Simulate opening dialog and logging in
    {
        let state = harness.state_mut().state_mut();
        state.login_dialog_state.open();
        state.login_dialog_state.form_data.username = "testuser".to_string();
        state.login_dialog_state.form_data.otp_code = "123456".to_string();
        state.auth_state.start_login();
    }

    // Trigger login - need to get sender first
    let sender = harness.state().state().login_result_sender.clone();
    let form_data = harness.state().state().login_dialog_state.form_data.clone();
    collects_ui::widgets::perform_login(&harness.state().state().ctx, &form_data, sender);

    // Wait for the mock server response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Poll for login result - clone receiver first
    let receiver = harness.state().state().login_result_receiver.clone();
    {
        let state = harness.state_mut().state_mut();
        collects_ui::widgets::poll_login_result(
            &receiver,
            &mut state.auth_state,
            &mut state.login_dialog_state,
        );
    }

    harness.step();

    // Verify user is now logged in
    assert!(
        harness.state().state().auth_state.is_logged_in(),
        "User should be logged in after successful authentication"
    );
    assert_eq!(
        harness.state().state().auth_state.username,
        Some("testuser".to_string()),
        "Username should be set correctly"
    );
}

#[tokio::test]
async fn test_signin_flow_with_invalid_credentials() {
    let mut ctx = TestCtx::new_app_with_auth("testuser", "123456").await;

    let harness = ctx.harness_mut();
    harness.step();

    // Simulate opening dialog and logging in with wrong credentials
    {
        let state = harness.state_mut().state_mut();
        state.login_dialog_state.open();
        state.login_dialog_state.form_data.username = "wronguser".to_string();
        state.login_dialog_state.form_data.otp_code = "000000".to_string();
        state.auth_state.start_login();
    }

    // Trigger login
    let sender = harness.state().state().login_result_sender.clone();
    let form_data = harness.state().state().login_dialog_state.form_data.clone();
    collects_ui::widgets::perform_login(&harness.state().state().ctx, &form_data, sender);

    // Wait for the mock server response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Poll for login result
    let receiver = harness.state().state().login_result_receiver.clone();
    {
        let state = harness.state_mut().state_mut();
        collects_ui::widgets::poll_login_result(
            &receiver,
            &mut state.auth_state,
            &mut state.login_dialog_state,
        );
    }

    harness.step();

    // Verify user is NOT logged in
    assert!(
        !harness.state().state().auth_state.is_logged_in(),
        "User should not be logged in after failed authentication"
    );
    assert!(
        harness.state().state().auth_state.error.is_some(),
        "Error message should be set after failed login"
    );
    assert!(
        harness.state().state().login_dialog_state.is_open,
        "Dialog should remain open after failed login"
    );
}

#[tokio::test]
async fn test_welcome_message_after_login() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Simulate successful login
    harness
        .state_mut()
        .state_mut()
        .auth_state
        .login_success("welcomeuser".to_string());
    harness.step();

    assert!(
        harness.query_by_label("Welcome, welcomeuser!").is_some(),
        "'Welcome, welcomeuser!' message should be visible after login"
    );
}

#[tokio::test]
async fn test_signout_button_visible_when_logged_in() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Simulate successful login
    harness
        .state_mut()
        .state_mut()
        .auth_state
        .login_success("testuser".to_string());
    harness.step();

    assert!(
        harness.query_by_label("Sign Out").is_some(),
        "'Sign Out' button should be visible when logged in"
    );
}

#[tokio::test]
async fn test_username_displayed_in_header_when_logged_in() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Simulate successful login
    harness
        .state_mut()
        .state_mut()
        .auth_state
        .login_success("headeruser".to_string());
    harness.step();

    // Check for the username in the header (with user icon prefix)
    assert!(
        harness.query_by_label("👤 headeruser").is_some(),
        "Username should be displayed in header when logged in"
    );
}
