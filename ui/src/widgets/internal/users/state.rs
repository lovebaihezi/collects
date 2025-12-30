//! State management for internal users panel.

use chrono::{DateTime, Utc};
use collects_business::InternalUserItem;
use egui::TextureHandle;
use std::collections::HashMap;

/// Action type for user management.
#[derive(Debug, Clone, PartialEq)]
pub enum UserAction {
    /// No action.
    None,
    /// Show QR code for a user.
    ShowQrCode(String),
    /// Edit username.
    EditUsername(String),
    /// Delete user (with confirmation).
    DeleteUser(String),
    /// Revoke OTP for a user.
    RevokeOtp(String),
}

impl Default for UserAction {
    fn default() -> Self {
        Self::None
    }
}

/// State for the internal users panel.
#[derive(Default)]
pub struct InternalUsersState {
    /// List of users fetched from the API.
    pub(crate) users: Vec<InternalUserItem>,
    /// Map to track which users have their OTP revealed.
    pub(crate) revealed_otps: HashMap<String, bool>,
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
    /// Whether an action is in progress.
    pub(crate) action_in_progress: bool,
    /// Action error message.
    pub(crate) action_error: Option<String>,
    /// QR code data for display (otpauth URL).
    pub(crate) qr_code_data: Option<String>,
}

impl InternalUsersState {
    /// Create a new internal users state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle OTP visibility for a user.
    pub fn toggle_otp_visibility(&mut self, username: &str) {
        let revealed = self
            .revealed_otps
            .entry(username.to_string())
            .or_insert(false);
        *revealed = !*revealed;
    }

    /// Check if OTP is revealed for a user.
    pub fn is_otp_revealed(&self, username: &str) -> bool {
        self.revealed_otps.get(username).copied().unwrap_or(false)
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

        // Initialize edit username input if editing
        if let UserAction::EditUsername(username) = &action {
            self.edit_username_input = username.clone();
        }
    }

    /// Close the current action modal.
    pub fn close_action(&mut self) {
        self.current_action = UserAction::None;
        self.action_in_progress = false;
        self.action_error = None;
        self.edit_username_input.clear();
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
}
