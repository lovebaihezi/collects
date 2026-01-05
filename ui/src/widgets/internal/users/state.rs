//! State management for internal users panel.

use chrono::{DateTime, Utc};
use collects_business::InternalUserItem;
use collects_states::State;
use egui::TextureHandle;
use std::any::Any;
use std::collections::HashMap;
use ustr::Ustr;

/// Action type for user management.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum UserAction {
    /// No action.
    #[default]
    None,
    /// Show QR code for a user.
    ShowQrCode(Ustr),
    /// Edit username.
    EditUsername(Ustr),
    /// Edit profile (nickname and avatar URL).
    EditProfile(Ustr),
    /// Delete user (with confirmation).
    DeleteUser(Ustr),
    /// Revoke OTP for a user.
    RevokeOtp(Ustr),
}

/// State for the internal users panel.
///
/// This state is stored in `StateCtx` and can be accessed via `state_mut::<InternalUsersState>()`.
#[derive(Default)]
pub struct InternalUsersState {
    /// List of users fetched from the API.
    pub(crate) users: Vec<InternalUserItem>,
    /// Map to track which users have their OTP revealed.
    pub(crate) revealed_otps: HashMap<Ustr, bool>,
    /// Whether currently fetching users.
    pub(crate) is_fetching: bool,
    /// Error message if fetch failed.
    pub(crate) error: Option<String>,
    /// Last fetch timestamp (using DateTime<Utc> for WASM compatibility and test mockability).
    pub(crate) last_fetch: Option<DateTime<Utc>>,
    /// Whether the create user modal is open.
    pub(crate) create_modal_open: bool,
    /// Username input for create modal.
    pub(crate) new_username: String,
    /// Cached QR code texture for the created user.
    pub(crate) qr_texture: Option<TextureHandle>,
    /// Current action being performed.
    pub(crate) current_action: UserAction,
    /// Edit username input.
    pub(crate) edit_username_input: String,
    /// Edit nickname input for profile editing.
    pub(crate) edit_nickname_input: String,
    /// Edit avatar URL input for profile editing.
    pub(crate) edit_avatar_url_input: String,
    /// Whether an action is in progress.
    pub(crate) action_in_progress: bool,
    /// Action error message.
    pub(crate) action_error: Option<String>,
    /// QR code data for display (otpauth URL).
    pub(crate) qr_code_data: Option<String>,
}

impl std::fmt::Debug for InternalUsersState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InternalUsersState")
            .field("users", &self.users)
            .field("revealed_otps", &self.revealed_otps)
            .field("is_fetching", &self.is_fetching)
            .field("error", &self.error)
            .field("last_fetch", &self.last_fetch)
            .field("create_modal_open", &self.create_modal_open)
            .field("new_username", &self.new_username)
            .field("qr_texture", &self.qr_texture.is_some())
            .field("current_action", &self.current_action)
            .field("edit_username_input", &self.edit_username_input)
            .field("edit_nickname_input", &self.edit_nickname_input)
            .field("edit_avatar_url_input", &self.edit_avatar_url_input)
            .field("action_in_progress", &self.action_in_progress)
            .field("action_error", &self.action_error)
            .field("qr_code_data", &self.qr_code_data)
            .finish()
    }
}

impl State for InternalUsersState {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl InternalUsersState {
    /// Create a new internal users state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle OTP visibility for a user.
    pub fn toggle_otp_visibility(&mut self, username: Ustr) {
        let revealed = self.revealed_otps.entry(username).or_insert(false);
        *revealed = !*revealed;
    }

    /// Check if OTP is revealed for a user.
    pub fn is_otp_revealed(&self, username: &str) -> bool {
        let key = Ustr::from(username);
        self.revealed_otps.get(&key).copied().unwrap_or(false)
    }

    /// Update users from API response.
    ///
    /// Takes `now` as a parameter to allow test mockability via the `Time` state.
    pub fn update_users(&mut self, users: Vec<InternalUserItem>, now: DateTime<Utc>) {
        self.users = users;
        self.is_fetching = false;
        self.error = None;
        self.last_fetch = Some(now);
    }

    /// Set error state.
    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.is_fetching = false;
    }

    /// Set fetching state.
    pub fn set_fetching(&mut self) {
        self.is_fetching = true;
        self.error = None;
    }

    /// Open create user modal.
    pub fn open_create_modal(&mut self) {
        self.create_modal_open = true;
        self.new_username.clear();
    }

    /// Close create user modal.
    pub fn close_create_modal(&mut self) {
        self.create_modal_open = false;
        self.new_username.clear();
        self.qr_texture = None;
    }

    /// Start an action.
    pub fn start_action(&mut self, action: UserAction) {
        self.current_action = action.clone();
        self.action_in_progress = false;
        self.action_error = None;
        self.qr_texture = None;
        self.qr_code_data = None;

        // Initialize inputs based on action type
        match &action {
            UserAction::EditUsername(username) => {
                self.edit_username_input = username.to_string();
            }
            UserAction::EditProfile(username) => {
                // Initialize with current values from the user
                if let Some(user) = self.users.iter().find(|u| u.username == username.as_str()) {
                    self.edit_nickname_input = user.nickname.clone().unwrap_or_default();
                    self.edit_avatar_url_input = user.avatar_url.clone().unwrap_or_default();
                } else {
                    self.edit_nickname_input.clear();
                    self.edit_avatar_url_input.clear();
                }
            }
            _ => {}
        }
    }

    /// Close the current action modal.
    pub fn close_action(&mut self) {
        self.current_action = UserAction::None;
        self.action_in_progress = false;
        self.action_error = None;
        self.edit_username_input.clear();
        self.edit_nickname_input.clear();
        self.edit_avatar_url_input.clear();
        self.qr_texture = None;
        self.qr_code_data = None;
    }

    /// Set action error.
    pub fn set_action_error(&mut self, error: String) {
        self.action_error = Some(error);
        self.action_in_progress = false;
    }

    /// Set action in progress.
    pub fn set_action_in_progress(&mut self) {
        self.action_in_progress = true;
        self.action_error = None;
    }

    /// Set QR code data for display.
    pub fn set_qr_code_data(&mut self, otpauth_url: String) {
        self.qr_code_data = Some(otpauth_url);
        self.action_in_progress = false;
    }

    // =====================
    // Getter methods for testing
    // =====================

    /// Get the current action.
    pub fn current_action(&self) -> &UserAction {
        &self.current_action
    }

    /// Get the edit nickname input.
    pub fn edit_nickname_input(&self) -> &str {
        &self.edit_nickname_input
    }

    /// Get the edit avatar URL input.
    pub fn edit_avatar_url_input(&self) -> &str {
        &self.edit_avatar_url_input
    }

    /// Get whether action is in progress.
    pub fn is_action_in_progress(&self) -> bool {
        self.action_in_progress
    }

    /// Get the action error.
    pub fn action_error(&self) -> Option<&str> {
        self.action_error.as_deref()
    }

    /// Get the users list.
    pub fn users(&self) -> &[InternalUserItem] {
        &self.users
    }

    /// Get mutable reference to users list for testing.
    pub fn users_mut(&mut self) -> &mut Vec<InternalUserItem> {
        &mut self.users
    }

    /// Get whether currently fetching.
    pub fn is_fetching(&self) -> bool {
        self.is_fetching
    }

    /// Calculate real-time time remaining for a user's OTP code.
    ///
    /// OTP codes operate on a 30-second cycle. This method calculates the actual
    /// seconds remaining based on:
    /// - The original `time_remaining` value from when data was fetched
    /// - The elapsed time since the last fetch
    ///
    /// # Arguments
    ///
    /// * `original_time_remaining` - The time remaining value from the fetched user data (1-30)
    /// * `now` - The current time (from Time state for mockability)
    ///
    /// # Returns
    ///
    /// The real-time seconds remaining (1-30), automatically wrapping through OTP cycles.
    pub fn calculate_time_remaining(&self, original_time_remaining: u8, now: DateTime<Utc>) -> u8 {
        const OTP_CYCLE_SECONDS: i64 = 30;

        let Some(last_fetch) = self.last_fetch else {
            // If no fetch time recorded, return original value
            return original_time_remaining;
        };

        let elapsed_seconds = now.signed_duration_since(last_fetch).num_seconds();

        if elapsed_seconds < 0 {
            // Time went backwards (clock skew), return original value
            return original_time_remaining;
        }

        // Calculate the new time remaining
        // original_time_remaining was the seconds until code change at last_fetch
        // After elapsed_seconds, we need to compute new position in the 30-second cycle
        let original = original_time_remaining as i64;
        let remaining = original - (elapsed_seconds % OTP_CYCLE_SECONDS);

        // Handle wrap-around: if remaining <= 0, we've passed into new cycle(s)
        let adjusted = if remaining <= 0 {
            remaining + OTP_CYCLE_SECONDS
        } else {
            remaining
        };

        // Clamp to valid range (1-30)
        adjusted.clamp(1, OTP_CYCLE_SECONDS) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    /// Creates a state with last_fetch set to the given time.
    fn state_with_last_fetch(last_fetch: DateTime<Utc>) -> InternalUsersState {
        let mut state = InternalUsersState::new();
        state.last_fetch = Some(last_fetch);
        state
    }

    #[test]
    fn test_calculate_time_remaining_no_elapsed_time() {
        let now = Utc::now();
        let state = state_with_last_fetch(now);

        // If no time has elapsed, time remaining should be unchanged
        assert_eq!(state.calculate_time_remaining(30, now), 30);
        assert_eq!(state.calculate_time_remaining(15, now), 15);
        assert_eq!(state.calculate_time_remaining(1, now), 1);
    }

    #[test]
    fn test_calculate_time_remaining_5_seconds_elapsed() {
        let now = Utc::now();
        let fetch_time = now - Duration::seconds(5);
        let state = state_with_last_fetch(fetch_time);

        // 30 - 5 = 25
        assert_eq!(state.calculate_time_remaining(30, now), 25);
        // 15 - 5 = 10
        assert_eq!(state.calculate_time_remaining(15, now), 10);
        // 10 - 5 = 5
        assert_eq!(state.calculate_time_remaining(10, now), 5);
    }

    #[test]
    fn test_calculate_time_remaining_wrap_around() {
        let now = Utc::now();
        let fetch_time = now - Duration::seconds(10);
        let state = state_with_last_fetch(fetch_time);

        // 5 - 10 = -5, which wraps to 25 (30 + (-5))
        assert_eq!(state.calculate_time_remaining(5, now), 25);

        // 1 - 10 = -9, which wraps to 21 (30 + (-9))
        assert_eq!(state.calculate_time_remaining(1, now), 21);
    }

    #[test]
    fn test_calculate_time_remaining_full_cycle() {
        let now = Utc::now();
        let fetch_time = now - Duration::seconds(30);
        let state = state_with_last_fetch(fetch_time);

        // After exactly one full cycle (30 seconds), time remaining should be same
        assert_eq!(state.calculate_time_remaining(30, now), 30);
        assert_eq!(state.calculate_time_remaining(15, now), 15);
    }

    #[test]
    fn test_calculate_time_remaining_multiple_cycles() {
        let now = Utc::now();
        let fetch_time = now - Duration::seconds(65); // 2 full cycles + 5 seconds
        let state = state_with_last_fetch(fetch_time);

        // 65 % 30 = 5 seconds elapsed in current cycle
        // 30 - 5 = 25
        assert_eq!(state.calculate_time_remaining(30, now), 25);
        // 15 - 5 = 10
        assert_eq!(state.calculate_time_remaining(15, now), 10);
    }

    #[test]
    fn test_calculate_time_remaining_no_last_fetch() {
        let state = InternalUsersState::new();
        let now = Utc::now();

        // Without last_fetch, should return original value
        assert_eq!(state.calculate_time_remaining(30, now), 30);
        assert_eq!(state.calculate_time_remaining(15, now), 15);
    }

    #[test]
    fn test_calculate_time_remaining_clock_skew() {
        let now = Utc::now();
        let future_fetch_time = now + Duration::seconds(10);
        let state = state_with_last_fetch(future_fetch_time);

        // If last_fetch is in the future (clock skew), should return original value
        assert_eq!(state.calculate_time_remaining(30, now), 30);
        assert_eq!(state.calculate_time_remaining(15, now), 15);
    }

    #[test]
    fn test_calculate_time_remaining_exactly_at_boundary() {
        let now = Utc::now();

        // Test when original is 30 and we're at exact boundary
        let fetch_time = now - Duration::seconds(30);
        let state = state_with_last_fetch(fetch_time);
        // 30 - (30 % 30) = 30 - 0 = 30
        assert_eq!(state.calculate_time_remaining(30, now), 30);

        // Test when original is 1 and 1 second has passed
        let fetch_time = now - Duration::seconds(1);
        let state = state_with_last_fetch(fetch_time);
        // 1 - 1 = 0, which should wrap to 30
        assert_eq!(state.calculate_time_remaining(1, now), 30);
    }

    #[test]
    fn test_start_action_edit_profile_initializes_fields() {
        let mut state = InternalUsersState::new();

        // Add a user with nickname and avatar
        state.users.push(InternalUserItem {
            username: "testuser".to_string(),
            current_otp: "123456".to_string(),
            time_remaining: 25,
            nickname: Some("Test Nickname".to_string()),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
            created_at: "2026-01-04T08:00:00Z".to_string(),
            updated_at: "2026-01-04T08:15:00Z".to_string(),
        });

        // Start edit profile action
        state.start_action(UserAction::EditProfile(Ustr::from("testuser")));

        // Verify profile fields are initialized from user
        assert_eq!(state.edit_nickname_input, "Test Nickname");
        assert_eq!(
            state.edit_avatar_url_input,
            "https://example.com/avatar.png"
        );
        assert_eq!(
            state.current_action,
            UserAction::EditProfile(Ustr::from("testuser"))
        );
    }

    #[test]
    fn test_start_action_edit_profile_empty_fields() {
        let mut state = InternalUsersState::new();

        // Add a user without nickname and avatar
        state.users.push(InternalUserItem {
            username: "testuser".to_string(),
            current_otp: "123456".to_string(),
            time_remaining: 25,
            nickname: None,
            avatar_url: None,
            created_at: "2026-01-04T08:00:00Z".to_string(),
            updated_at: "2026-01-04T08:15:00Z".to_string(),
        });

        // Start edit profile action
        state.start_action(UserAction::EditProfile(Ustr::from("testuser")));

        // Verify profile fields are empty when user has no values
        assert_eq!(state.edit_nickname_input, "");
        assert_eq!(state.edit_avatar_url_input, "");
    }

    #[test]
    fn test_start_action_edit_profile_user_not_found() {
        let mut state = InternalUsersState::new();

        // Start edit profile action for non-existent user
        state.start_action(UserAction::EditProfile(Ustr::from("nonexistent")));

        // Verify profile fields are empty when user not found
        assert_eq!(state.edit_nickname_input, "");
        assert_eq!(state.edit_avatar_url_input, "");
    }

    #[test]
    fn test_close_action_clears_profile_fields() {
        let mut state = InternalUsersState::new();

        // Set up some profile fields
        state.edit_nickname_input = "Test Nickname".to_string();
        state.edit_avatar_url_input = "https://example.com/avatar.png".to_string();
        state.current_action = UserAction::EditProfile(Ustr::from("testuser"));

        // Close the action
        state.close_action();

        // Verify all fields are cleared
        assert_eq!(state.edit_nickname_input, "");
        assert_eq!(state.edit_avatar_url_input, "");
        assert_eq!(state.current_action, UserAction::None);
    }

    #[test]
    fn test_user_action_edit_profile_variant() {
        let action = UserAction::EditProfile(Ustr::from("testuser"));

        // Verify the action variant
        assert!(matches!(action, UserAction::EditProfile(_)));
        if let UserAction::EditProfile(username) = action {
            assert_eq!(username.as_str(), "testuser");
        }
    }
}
