//! Integration tests for profile update feature.
//!
//! These tests verify the complete flow for updating user profile
//! through the UI by using kittest to control the egui interface
//! and wiremock to mock the API responses.
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use collects_business::InternalUserItem;
use collects_ui::state::State;
use collects_ui::widgets::{InternalUsersState, UserAction};
use egui_kittest::Harness;
use ustr::Ustr;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for profile update integration tests.
struct ProfileTestCtx<'a> {
    #[allow(dead_code)]
    mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> ProfileTestCtx<'a> {
    /// Get mutable reference to the harness.
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }
}

/// Setup test state with mock server and UI harness.
async fn setup_profile_test<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> ProfileTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock the health check endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock the internal users endpoint
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": []
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);
    let harness = Harness::new_ui_state(app, state);

    ProfileTestCtx {
        mock_server,
        harness,
    }
}

/// Create a test user item for use in tests.
fn create_test_user(username: &str) -> InternalUserItem {
    InternalUserItem {
        username: username.to_string(),
        current_otp: "123456".to_string(),
        time_remaining: 25,
        nickname: Some("Test Nickname".to_string()),
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

// ===========================================
// Tests for EditProfile action state management
// ===========================================

/// Test that starting EditProfile action initializes state correctly.
#[tokio::test]
async fn test_edit_profile_action_initializes_state() {
    let mut ctx = setup_profile_test(|ui, state| {
        // Display the current action state
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        ui.label(format!("Action: {:?}", users_state.current_action()));
        ui.label(format!("Nickname: {}", users_state.edit_nickname_input()));
        ui.label(format!(
            "Avatar URL: {}",
            users_state.edit_avatar_url_input()
        ));
    })
    .await;

    let harness = ctx.harness_mut();

    // Add a test user and start the EditProfile action
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.users_mut().push(create_test_user("testuser"));
        users_state.start_action(UserAction::EditProfile(Ustr::from("testuser")));
    }

    harness.step();

    // Verify the state is initialized with user's current values
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert_eq!(
        *users_state.current_action(),
        UserAction::EditProfile(Ustr::from("testuser"))
    );
    assert_eq!(users_state.edit_nickname_input(), "Test Nickname");
    assert_eq!(
        users_state.edit_avatar_url_input(),
        "https://example.com/avatar.png"
    );
}

/// Test that EditProfile action with non-existent user clears inputs.
#[tokio::test]
async fn test_edit_profile_action_nonexistent_user() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        ui.label(format!("Nickname: {}", users_state.edit_nickname_input()));
        ui.label(format!(
            "Avatar URL: {}",
            users_state.edit_avatar_url_input()
        ));
    })
    .await;

    let harness = ctx.harness_mut();

    // Start EditProfile action for non-existent user
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.start_action(UserAction::EditProfile(Ustr::from("nonexistent")));
    }

    harness.step();

    // Verify inputs are cleared
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert_eq!(users_state.edit_nickname_input(), "");
    assert_eq!(users_state.edit_avatar_url_input(), "");
}

/// Test that close_action clears the profile editing state.
#[tokio::test]
async fn test_close_action_clears_profile_state() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        ui.label(format!("Action: {:?}", users_state.current_action()));
    })
    .await;

    let harness = ctx.harness_mut();

    // Set up state and then close
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.users_mut().push(create_test_user("testuser"));
        users_state.start_action(UserAction::EditProfile(Ustr::from("testuser")));
    }

    harness.step();

    // Close the action
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.close_action();
    }

    harness.step();

    // Verify state is cleared
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert_eq!(*users_state.current_action(), UserAction::None);
    assert_eq!(users_state.edit_nickname_input(), "");
    assert_eq!(users_state.edit_avatar_url_input(), "");
}

// ===========================================
// Tests for profile update UI flow
// ===========================================

/// Test that update profile action is set correctly when triggered.
#[tokio::test]
async fn test_update_profile_sets_action_in_progress() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        ui.label(format!(
            "In Progress: {}",
            users_state.is_action_in_progress()
        ));
    })
    .await;

    let harness = ctx.harness_mut();

    // Set up state and mark action in progress
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.users_mut().push(create_test_user("testuser"));
        users_state.start_action(UserAction::EditProfile(Ustr::from("testuser")));
        users_state.set_action_in_progress();
    }

    harness.step();

    // Verify action is in progress
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert!(users_state.is_action_in_progress());
}

/// Test that action error is set correctly.
#[tokio::test]
async fn test_update_profile_handles_error() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        if let Some(error) = users_state.action_error() {
            ui.label(format!("Error: {}", error));
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Set up state and set error
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.start_action(UserAction::EditProfile(Ustr::from("testuser")));
        users_state.set_action_error("User not found".to_string());
    }

    harness.step();

    // Verify error is set and action is no longer in progress
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert_eq!(users_state.action_error(), Some("User not found"));
    assert!(!users_state.is_action_in_progress());
}

// ===========================================
// Tests for InternalUserItem with profile fields
// ===========================================

/// Test that InternalUserItem stores profile fields correctly.
#[tokio::test]
async fn test_internal_user_item_profile_fields() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        for user in users_state.users() {
            ui.label(format!("Username: {}", user.username));
            ui.label(format!("Nickname: {:?}", user.nickname));
            ui.label(format!("Avatar URL: {:?}", user.avatar_url));
            ui.label(format!("Created At: {}", user.created_at));
            ui.label(format!("Updated At: {}", user.updated_at));
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Add a user with profile fields
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.users_mut().push(InternalUserItem {
            username: "profileuser".to_string(),
            current_otp: "654321".to_string(),
            time_remaining: 15,
            nickname: Some("Profile User".to_string()),
            avatar_url: Some("https://example.com/profile.jpg".to_string()),
            created_at: "2026-01-05T10:00:00Z".to_string(),
            updated_at: "2026-01-05T11:30:00Z".to_string(),
        });
    }

    harness.step();

    // Verify user fields
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert_eq!(users_state.users().len(), 1);

    let user = &users_state.users()[0];
    assert_eq!(user.username, "profileuser");
    assert_eq!(user.nickname, Some("Profile User".to_string()));
    assert_eq!(
        user.avatar_url,
        Some("https://example.com/profile.jpg".to_string())
    );
    assert_eq!(user.created_at, "2026-01-05T10:00:00Z");
    assert_eq!(user.updated_at, "2026-01-05T11:30:00Z");
}

/// Test that user without profile fields has None values.
#[tokio::test]
async fn test_internal_user_item_no_profile() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        for user in users_state.users() {
            ui.label(format!("Nickname: {:?}", user.nickname));
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Add a user without profile fields
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.users_mut().push(InternalUserItem {
            username: "simpleuser".to_string(),
            current_otp: "111111".to_string(),
            time_remaining: 20,
            nickname: None,
            avatar_url: None,
            created_at: "2026-01-05T10:00:00Z".to_string(),
            updated_at: "2026-01-05T10:00:00Z".to_string(),
        });
    }

    harness.step();

    // Verify None values
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    let user = &users_state.users()[0];
    assert_eq!(user.nickname, None);
    assert_eq!(user.avatar_url, None);
}

/// Test that EditProfile action for user with no profile initializes empty fields.
#[tokio::test]
async fn test_edit_profile_action_empty_user_profile() {
    let mut ctx = setup_profile_test(|ui, state| {
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        ui.label(format!("Nickname: '{}'", users_state.edit_nickname_input()));
        ui.label(format!("Avatar: '{}'", users_state.edit_avatar_url_input()));
    })
    .await;

    let harness = ctx.harness_mut();

    // Add a user without profile fields and start edit
    {
        let state = harness.state_mut();
        let users_state = state.ctx.state_mut::<InternalUsersState>();
        users_state.users_mut().push(InternalUserItem {
            username: "emptyprofile".to_string(),
            current_otp: "000000".to_string(),
            time_remaining: 30,
            nickname: None,
            avatar_url: None,
            created_at: "2026-01-05T10:00:00Z".to_string(),
            updated_at: "2026-01-05T10:00:00Z".to_string(),
        });
        users_state.start_action(UserAction::EditProfile(Ustr::from("emptyprofile")));
    }

    harness.step();

    // Verify inputs are empty strings (not None)
    let state = harness.state();
    let users_state = state.ctx.state_mut::<InternalUsersState>();
    assert_eq!(users_state.edit_nickname_input(), "");
    assert_eq!(users_state.edit_avatar_url_input(), "");
}
