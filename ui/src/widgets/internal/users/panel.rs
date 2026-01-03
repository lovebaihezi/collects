//! Main panel for internal users management.
//!
//! Uses a Typora-like table style with clean borders and minimal styling.

use collects_business::{CreateUserCommand, CreateUserCompute, CreateUserInput, InternalUserItem};
use collects_states::{StateCtx, Time};
use egui::{Color32, Frame, InnerResponse, Margin, Response, RichText, ScrollArea, Stroke, Ui};
use std::any::TypeId;

use super::api::fetch_users;
use super::modals::{
    show_create_user_modal, show_delete_user_modal, show_edit_username_modal, show_qr_code_modal,
    show_revoke_otp_modal,
};
use super::state::{InternalUsersState, UserAction};

/// Border color for Typora-like table style (subtle gray)
const TABLE_BORDER_COLOR: Color32 = Color32::from_rgb(200, 200, 200);

/// Header background color for Typora-like table style (light gray)
const HEADER_BG_COLOR: Color32 = Color32::from_rgb(245, 245, 245);

/// Helper to create a Typora-style header cell with background.
fn header_cell<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    Frame::NONE
        .fill(HEADER_BG_COLOR)
        .inner_margin(Margin::symmetric(8, 8))
        .show(ui, add_contents)
}

/// Helper to create a Typora-style data cell with padding.
fn data_cell<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> InnerResponse<R> {
    Frame::NONE
        .inner_margin(Margin::symmetric(8, 6))
        .show(ui, add_contents)
}

/// Displays the internal users panel with a Typora-like table style.
pub fn internal_users_panel(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) -> Response {
    let response = ui.vertical(|ui| {
        // Get state from StateCtx
        let state = state_ctx.state_mut::<InternalUsersState>();

        // Toolbar row: Refresh and Create buttons (compact, no heading)
        let should_open_create = ui.horizontal(|ui| {
            if ui.button("🔄 Refresh").clicked() && !state.is_fetching {
                state.set_fetching();
                fetch_users(api_base_url, ui.ctx().clone());
            }

            let clicked = ui.button("➕ Create User").clicked();
            if state.is_fetching {
                ui.spinner();
                ui.label("Loading...");
            }
            clicked
        }).inner;

        // Error display
        let state = state_ctx.state_mut::<InternalUsersState>();
        if let Some(error) = &state.error {
            ui.colored_label(Color32::RED, format!("Error: {error}"));
        }

        ui.add_space(8.0);

        // Collect actions (avoiding borrow issues)
        let mut username_to_toggle: Option<String> = None;
        let mut action_to_start: Option<UserAction> = None;

        // Get current time for calculating real-time OTP time remaining
        let now = *state_ctx.state_mut::<Time>().as_ref();

        // Typora-like table with frame border
        let state = state_ctx.state_mut::<InternalUsersState>();
        Frame::NONE
            .stroke(Stroke::new(1.0, TABLE_BORDER_COLOR))
            .inner_margin(Margin::ZERO)
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("users_table")
                        .num_columns(5)
                        .striped(true)
                        .spacing([16.0, 0.0])
                        .min_col_width(60.0)
                        .show(ui, |ui| {
                            // Header row with background
                            header_cell(ui, |ui| { ui.strong("Username"); });
                            header_cell(ui, |ui| { ui.strong("OTP Code"); });
                            header_cell(ui, |ui| { ui.strong("Time Left"); });
                            header_cell(ui, |ui| { ui.strong("OTP"); });
                            header_cell(ui, |ui| { ui.strong("Actions"); });
                            ui.end_row();

                            // User rows with cell padding
                            for user in &state.users {
                                data_cell(ui, |ui| {
                                    ui.label(&user.username);
                                });

                                // OTP code with reveal/hide
                                data_cell(ui, |ui| {
                                    if state.is_otp_revealed(&user.username) {
                                        ui.label(RichText::new(&user.current_otp).monospace());
                                    } else {
                                        ui.label(RichText::new("••••••").monospace());
                                    }
                                });

                                // Calculate real-time time remaining based on elapsed time since fetch
                                let time_remaining =
                                    state.calculate_time_remaining(user.time_remaining, now);

                                // Time remaining indicator with color coding
                                data_cell(ui, |ui| {
                                    let time_color = if time_remaining <= 5 {
                                        Color32::RED // Critical: 5 seconds or less
                                    } else if time_remaining <= 10 {
                                        Color32::from_rgb(255, 165, 0) // Warning: 10 seconds or less
                                    } else {
                                        Color32::from_rgb(34, 139, 34) // Safe: more than 10 seconds
                                    };
                                    ui.label(
                                        RichText::new(format!("{}s", time_remaining))
                                            .monospace()
                                            .color(time_color),
                                    );
                                });

                                // Reveal/hide button
                                data_cell(ui, |ui| {
                                    let button_text = if state.is_otp_revealed(&user.username) {
                                        "Hide"
                                    } else {
                                        "Reveal"
                                    };
                                    if ui.button(button_text).clicked() {
                                        username_to_toggle = Some(user.username.clone());
                                    }
                                });

                                // Action buttons
                                data_cell(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.button("📱 QR").on_hover_text("Show QR Code").clicked() {
                                            action_to_start =
                                                Some(UserAction::ShowQrCode(user.username.clone()));
                                        }
                                        if ui.button("✏️").on_hover_text("Edit Username").clicked() {
                                            action_to_start =
                                                Some(UserAction::EditUsername(user.username.clone()));
                                        }
                                        if ui.button("🔄").on_hover_text("Revoke OTP").clicked() {
                                            action_to_start =
                                                Some(UserAction::RevokeOtp(user.username.clone()));
                                        }
                                        if ui.button("🗑️").on_hover_text("Delete User").clicked() {
                                            action_to_start =
                                                Some(UserAction::DeleteUser(user.username.clone()));
                                        }
                                    });
                                });

                                ui.end_row();
                            }
                        });
                });
            });

        // Apply toggle action after table iteration
        if let Some(username) = username_to_toggle {
            state.toggle_otp_visibility(&username);
        }

        // Start action if requested
        if let Some(action) = action_to_start {
            state.start_action(action);
        }

        // Handle create modal open (after borrowing issues resolved)
        if should_open_create {
            // Reset the compute state when opening modal
            reset_create_user_compute(state_ctx);
            state_ctx
                .state_mut::<InternalUsersState>()
                .open_create_modal();
        }
    });

    // Create user modal
    let state = state_ctx.state_mut::<InternalUsersState>();
    if state.create_modal_open {
        show_create_user_modal(state_ctx, ui);
    }

    // Action modals
    let state = state_ctx.state_mut::<InternalUsersState>();
    match &state.current_action.clone() {
        UserAction::ShowQrCode(username) => {
            show_qr_code_modal(state_ctx, api_base_url, username.clone(), ui);
        }
        UserAction::EditUsername(username) => {
            show_edit_username_modal(state_ctx, api_base_url, username.clone(), ui);
        }
        UserAction::DeleteUser(username) => {
            show_delete_user_modal(state_ctx, api_base_url, username.clone(), ui);
        }
        UserAction::RevokeOtp(username) => {
            show_revoke_otp_modal(state_ctx, api_base_url, username.clone(), ui);
        }
        UserAction::None => {}
    }

    response.response
}

/// Poll for async responses and update state.
/// Call this in the update loop.
pub fn poll_internal_users_responses(state_ctx: &mut StateCtx, ctx: &egui::Context) {
    // Check for users list response
    if let Some(users) = ctx.memory(|mem| {
        mem.data
            .get_temp::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"))
    }) {
        // Get current time from Time state for mockability
        let now = *state_ctx.state_mut::<Time>().as_ref();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(users, now);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"));
        });
    }

    // Check for users list error
    if let Some(error) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("internal_users_error"))
    }) {
        state_ctx.state_mut::<InternalUsersState>().set_error(error);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("internal_users_error"));
        });
    }

    // Check for action error
    if let Some(error) =
        ctx.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("action_error")))
    {
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_action_error(error);
        ctx.memory_mut(|mem| {
            mem.data.remove::<String>(egui::Id::new("action_error"));
        });
    }

    // Check for action success (triggers refresh)
    if let Some(action) =
        ctx.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("action_success")))
    {
        ctx.memory_mut(|mem| {
            mem.data.remove::<String>(egui::Id::new("action_success"));
        });
        // Close action modal and mark for refresh
        let state = state_ctx.state_mut::<InternalUsersState>();
        state.close_action();
        if action == "user_deleted" || action == "username_updated" {
            // Mark as needing fetch - the actual fetch will happen on next panel render
            // when internal_users_panel() is called with api_base_url
            state.set_fetching();
        }
    }

    // Check for QR code response
    if let Some(otpauth_url) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("user_qr_code_response"))
    }) {
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_qr_code_data(otpauth_url);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("user_qr_code_response"));
        });
    }

    // Check for revoke OTP response
    if let Some(otpauth_url) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("revoke_otp_response"))
    }) {
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_qr_code_data(otpauth_url);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("revoke_otp_response"));
        });
    }
}

/// Reset the CreateUserCompute to idle state.
pub(crate) fn reset_create_user_compute(state_ctx: &mut StateCtx) {
    // Clear the input
    let input = state_ctx.state_mut::<CreateUserInput>();
    input.username = None;
    // Mark compute as clean so it doesn't auto-run
    state_ctx.mark_clean(&TypeId::of::<CreateUserCompute>());
}

/// Trigger the create-user side effect by setting input and dispatching the command.
///
/// The command will update `CreateUserCompute` via `Updater`, and the normal
/// `StateCtx::sync_computes()` path will apply the result.
pub(crate) fn trigger_create_user(state_ctx: &mut StateCtx, username: &str) {
    // Update command input state
    state_ctx.update::<CreateUserInput>(|input| {
        input.username = Some(username.to_string());
    });

    // Explicitly dispatch the command (manual-only; never runs implicitly)
    state_ctx.dispatch::<CreateUserCommand>();
}

#[cfg(test)]
mod internal_users_panel_tests {
    use chrono::Utc;
    use collects_business::InternalUserItem;
    use egui_kittest::Harness;
    use kittest::Queryable;

    use super::*;

    /// Helper to create a StateCtx for testing internal users panel.
    fn create_test_state_ctx() -> StateCtx {
        let mut ctx = StateCtx::new();
        ctx.add_state(CreateUserInput::default());
        ctx.record_compute(CreateUserCompute::default());
        ctx.add_state(InternalUsersState::new());
        ctx.add_state(Time::default());
        ctx
    }

    /// Helper to create test users data.
    fn create_test_users() -> Vec<InternalUserItem> {
        vec![
            InternalUserItem {
                username: "alice".to_string(),
                current_otp: "123456".to_string(),
                time_remaining: 25,
            },
            InternalUserItem {
                username: "bob".to_string(),
                current_otp: "654321".to_string(),
                time_remaining: 8,
            },
            InternalUserItem {
                username: "charlie".to_string(),
                current_otp: "111222".to_string(),
                time_remaining: 3,
            },
        ]
    }

    // Element Existence Tests

    #[test]
    fn test_table_header_elements_exist() {
        let mut state_ctx = create_test_state_ctx();

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Verify header columns exist
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username header should exist"
        );
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "OTP Code header should exist"
        );
        assert!(
            harness.query_by_label_contains("Time Left").is_some(),
            "Time Left header should exist"
        );
        assert!(
            harness.query_by_label_contains("Actions").is_some(),
            "Actions header should exist"
        );
    }

    #[test]
    fn test_toolbar_buttons_exist() {
        let mut state_ctx = create_test_state_ctx();

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Verify toolbar buttons exist
        assert!(
            harness.query_by_label_contains("Refresh").is_some(),
            "Refresh button should exist"
        );
        assert!(
            harness.query_by_label_contains("Create User").is_some(),
            "Create User button should exist"
        );
    }

    #[test]
    fn test_user_rows_display_with_data() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Verify user rows display usernames
        assert!(
            harness.query_by_label_contains("alice").is_some(),
            "Username 'alice' should be displayed"
        );
        assert!(
            harness.query_by_label_contains("bob").is_some(),
            "Username 'bob' should be displayed"
        );
        assert!(
            harness.query_by_label_contains("charlie").is_some(),
            "Username 'charlie' should be displayed"
        );
    }

    // Content Correctness Tests

    #[test]
    fn test_otp_is_hidden_by_default() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // OTP should be hidden (shown as dots) - one per user
        let hidden_otp_count = harness.query_all_by_label_contains("••••••").count();
        assert_eq!(
            hidden_otp_count, 3,
            "All 3 OTPs should be hidden with dots by default"
        );
        // The actual OTP should not be visible
        assert!(
            harness.query_by_label_contains("123456").is_none(),
            "OTP '123456' should NOT be visible by default"
        );
    }

    #[test]
    fn test_time_remaining_displays_correctly() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users with different time remaining values
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Time remaining should be displayed with "s" suffix
        assert!(
            harness.query_by_label_contains("25s").is_some(),
            "Time remaining '25s' should be displayed"
        );
        assert!(
            harness.query_by_label_contains("8s").is_some(),
            "Time remaining '8s' should be displayed"
        );
        assert!(
            harness.query_by_label_contains("3s").is_some(),
            "Time remaining '3s' should be displayed"
        );
    }

    #[test]
    fn test_reveal_hide_buttons_exist_for_each_user() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Count "Reveal" buttons - should have one per user
        let reveal_count = harness.query_all_by_label("Reveal").count();
        assert_eq!(
            reveal_count, 3,
            "Should have 3 Reveal buttons (one per user)"
        );
    }

    #[test]
    fn test_action_buttons_exist_for_each_user() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Count QR buttons - should have one per user
        let qr_count = harness.query_all_by_label_contains("QR").count();
        assert_eq!(qr_count, 3, "Should have 3 QR buttons (one per user)");

        // Verify edit, revoke, and delete buttons exist
        let edit_count = harness.query_all_by_label("✏️").count();
        assert_eq!(edit_count, 3, "Should have 3 Edit buttons (one per user)");

        // Delete buttons
        let delete_count = harness.query_all_by_label("🗑️").count();
        assert_eq!(
            delete_count, 3,
            "Should have 3 Delete buttons (one per user)"
        );
    }

    // User Interaction Tests

    #[test]
    fn test_reveal_button_toggles_otp_visibility() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);

        let mut harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        harness.step();

        // Verify OTP is hidden initially (via state)
        assert!(
            !harness.state().state_mut::<InternalUsersState>().is_otp_revealed("alice"),
            "OTP should not be revealed initially"
        );

        // Click the first "Reveal" button
        if let Some(reveal_button) = harness.query_all_by_label("Reveal").next() {
            reveal_button.click();
        }
        harness.step();

        // Verify the state has been updated
        assert!(
            harness.state().state_mut::<InternalUsersState>().is_otp_revealed("alice"),
            "OTP should be revealed after clicking Reveal button"
        );

        // Run another step to re-render UI with new state
        harness.step();

        // Verify "Hide" button now appears
        assert!(
            harness.query_by_label("Hide").is_some(),
            "Hide button should appear after revealing OTP"
        );
    }

    #[test]
    fn test_hide_button_toggles_otp_visibility() {
        let mut state_ctx = create_test_state_ctx();

        // Add test users and reveal OTP for alice
        let now = Utc::now();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(create_test_users(), now);
        state_ctx
            .state_mut::<InternalUsersState>()
            .toggle_otp_visibility("alice");

        let mut harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        harness.step();

        // Verify OTP is revealed initially (via state)
        assert!(
            harness.state().state_mut::<InternalUsersState>().is_otp_revealed("alice"),
            "OTP should be revealed initially"
        );

        // "Hide" button should exist
        assert!(
            harness.query_by_label("Hide").is_some(),
            "Hide button should exist for revealed OTP"
        );

        // Click the "Hide" button
        if let Some(hide_button) = harness.query_by_label("Hide") {
            hide_button.click();
        }
        harness.step();

        // Verify the state has been updated
        assert!(
            !harness.state().state_mut::<InternalUsersState>().is_otp_revealed("alice"),
            "OTP should be hidden after clicking Hide button"
        );

        // Run another step to re-render UI with new state
        harness.step();

        // All buttons should show "Reveal" now
        let reveal_count = harness.query_all_by_label("Reveal").count();
        assert_eq!(
            reveal_count, 3,
            "All buttons should show 'Reveal' after hiding"
        );
    }

    #[test]
    fn test_create_user_button_opens_modal() {
        let mut state_ctx = create_test_state_ctx();

        let mut harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        harness.step();

        // Modal should not be open initially
        assert!(
            !harness.state().state_mut::<InternalUsersState>().create_modal_open,
            "Create modal should be closed initially"
        );

        // Click the "Create User" button
        if let Some(create_button) = harness.query_by_label_contains("Create User") {
            create_button.click();
        }
        harness.step();

        // Modal should now be open
        assert!(
            harness.state().state_mut::<InternalUsersState>().create_modal_open,
            "Create modal should be open after clicking button"
        );
    }

    #[test]
    fn test_loading_state_shows_spinner() {
        let mut state_ctx = create_test_state_ctx();

        // Set fetching state
        state_ctx.state_mut::<InternalUsersState>().set_fetching();

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // "Loading..." text should be visible
        assert!(
            harness.query_by_label_contains("Loading").is_some(),
            "Loading indicator should be visible when fetching"
        );
    }

    #[test]
    fn test_error_state_shows_message() {
        let mut state_ctx = create_test_state_ctx();

        // Set error state
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_error("Network connection failed".to_string());

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Error message should be visible
        assert!(
            harness.query_by_label_contains("Network connection failed").is_some(),
            "Error message should be displayed"
        );
    }

    #[test]
    fn test_empty_state_shows_headers_only() {
        let mut state_ctx = create_test_state_ctx();

        // Test with empty user list - no users added to state

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                internal_users_panel(state_ctx, "http://test", ui);
            },
            &mut state_ctx,
        );

        // Headers should still exist
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username header should exist even with no data"
        );

        // No reveal buttons (no users)
        let reveal_count = harness.query_all_by_label("Reveal").count();
        assert_eq!(reveal_count, 0, "No Reveal buttons when no users");
    }
}
