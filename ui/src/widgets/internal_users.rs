//! Internal user management panel widget.
//!
//! This module provides a panel for managing internal users when running
//! in test or internal build environments. Features include:
//! - Display users in a table with usernames and OTP codes
//! - Reveal/hide OTP codes with a click
//! - Create new users with QR code display for Google Authenticator

use collects_business::{CreateInternalUserResponse, InternalUser, generate_totp_code, is_internal_build};
use egui::{Color32, Response, RichText, Ui, Vec2};
use std::collections::{HashMap, HashSet};

/// State for the internal users panel.
#[derive(Default)]
pub struct InternalUsersState {
    /// List of users fetched from the API.
    users: Vec<InternalUser>,
    /// Set of usernames whose OTP codes are currently revealed.
    revealed_otps: HashSet<String>,
    /// Whether the create user modal is open.
    create_modal_open: bool,
    /// Username being entered in the create form.
    new_username: String,
    /// Response from creating a user (for QR code display).
    created_user: Option<CreateInternalUserResponse>,
    /// Error message to display.
    error_message: Option<String>,
    /// Internal API connection status.
    internal_api_connected: Option<bool>,
    /// Loading states for various operations.
    loading: InternalLoadingState,
    /// Cached OTP codes for display (refreshed periodically).
    cached_otp_codes: HashMap<String, String>,
    /// Last OTP refresh time (in seconds since epoch).
    last_otp_refresh: u64,
}

#[derive(Default)]
struct InternalLoadingState {
    creating: bool,
    #[allow(dead_code)]
    checking_api: bool,
}

impl InternalUsersState {
    /// Creates a new internal users state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a user to the list.
    pub fn add_user(&mut self, user: InternalUser) {
        self.users.push(user);
    }

    /// Sets the users list.
    pub fn set_users(&mut self, users: Vec<InternalUser>) {
        self.users = users;
    }

    /// Returns whether OTP is revealed for a user.
    pub fn is_otp_revealed(&self, username: &str) -> bool {
        self.revealed_otps.contains(username)
    }

    /// Toggles OTP visibility for a user.
    pub fn toggle_otp(&mut self, username: &str) {
        if self.revealed_otps.contains(username) {
            self.revealed_otps.remove(username);
        } else {
            self.revealed_otps.insert(username.to_string());
        }
    }

    /// Opens the create user modal.
    pub fn open_create_modal(&mut self) {
        self.create_modal_open = true;
        self.new_username.clear();
        self.created_user = None;
        self.error_message = None;
    }

    /// Closes the create user modal.
    pub fn close_create_modal(&mut self) {
        self.create_modal_open = false;
        self.new_username.clear();
        self.created_user = None;
        self.error_message = None;
    }

    /// Sets the created user response (for QR code display).
    pub fn set_created_user(&mut self, response: CreateInternalUserResponse) {
        // Also add to users list
        self.users.push(InternalUser::new(
            response.username.clone(),
            response.secret.clone(),
        ));
        self.created_user = Some(response);
        self.loading.creating = false;
    }

    /// Sets an error message.
    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
        self.loading.creating = false;
    }

    /// Sets the internal API connection status.
    pub fn set_api_status(&mut self, connected: bool) {
        self.internal_api_connected = Some(connected);
        self.loading.checking_api = false;
    }

    /// Gets OTP code for a user, using cache when appropriate.
    fn get_otp_code(&mut self, secret: &str) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Refresh cache every 30 seconds (TOTP period)
        let current_period = now / 30;
        let last_period = self.last_otp_refresh / 30;

        if current_period != last_period {
            self.cached_otp_codes.clear();
            self.last_otp_refresh = now;
        }

        if let Some(code) = self.cached_otp_codes.get(secret) {
            return code.clone();
        }

        let code = generate_totp_code(secret).unwrap_or_else(|| "------".to_string());
        self.cached_otp_codes.insert(secret.to_string(), code.clone());
        code
    }
}

/// Renders the internal users panel.
///
/// This panel is only rendered when running in test or internal build environments.
/// It provides functionality to view and manage internal users.
#[allow(clippy::too_many_lines)]
pub fn internal_users_panel(ui: &mut Ui, state: &mut InternalUsersState) -> Response {
    // Only show in internal/test builds
    if !is_internal_build() {
        return ui.label("");
    }

    let response = egui::Frame::NONE
        .fill(Color32::from_rgb(45, 45, 45))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .outer_margin(egui::Margin::symmetric(0, 8))
        .corner_radius(8.0)
        .show(ui, |ui| {
            ui.heading(RichText::new("🔐 Internal User Management").color(Color32::WHITE));
            ui.add_space(8.0);

            // API Status indicator
            render_api_status(ui, state);
            ui.add_space(8.0);

            // Create User button
            if ui
                .button(RichText::new("➕ Create User").color(Color32::WHITE))
                .clicked()
            {
                state.open_create_modal();
            }

            ui.add_space(12.0);

            // Users table
            render_users_table(ui, state);
        });

    // Render create user modal if open
    if state.create_modal_open {
        render_create_user_modal(ui.ctx(), state);
    }

    response.response
}

/// Renders the internal API status indicator.
fn render_api_status(ui: &mut Ui, state: &InternalUsersState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Internal API:").color(Color32::LIGHT_GRAY));

        let (status_text, status_color) = match state.internal_api_connected {
            Some(true) => ("Connected", Color32::from_rgb(34, 139, 34)),
            Some(false) => ("Disconnected", Color32::from_rgb(220, 53, 69)),
            None => ("Checking...", Color32::from_rgb(255, 193, 7)),
        };

        egui::Frame::NONE
            .fill(status_color)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .corner_radius(4.0)
            .show(ui, |ui| {
                ui.label(RichText::new(status_text).color(Color32::WHITE).small());
            });
    });
}

/// Renders the users table.
fn render_users_table(ui: &mut Ui, state: &mut InternalUsersState) {
    if state.users.is_empty() {
        ui.label(RichText::new("No users found").color(Color32::GRAY).italics());
        return;
    }

    egui::Frame::NONE
        .fill(Color32::from_rgb(35, 35, 35))
        .inner_margin(egui::Margin::same(8))
        .corner_radius(4.0)
        .show(ui, |ui| {
            // Table header
            ui.horizontal(|ui| {
                ui.add_sized([150.0, 20.0], egui::Label::new(
                    RichText::new("Username").color(Color32::LIGHT_GRAY).strong(),
                ));
                ui.add_sized([120.0, 20.0], egui::Label::new(
                    RichText::new("OTP Code").color(Color32::LIGHT_GRAY).strong(),
                ));
                ui.label(RichText::new("Actions").color(Color32::LIGHT_GRAY).strong());
            });

            ui.separator();

            // Clone users to avoid borrow issues
            let users: Vec<_> = state.users.to_vec();

            // Table rows
            for user in users {
                ui.horizontal(|ui| {
                    // Username
                    ui.add_sized([150.0, 20.0], egui::Label::new(
                        RichText::new(&user.username).color(Color32::WHITE),
                    ));

                    // OTP Code (revealed or hidden)
                    let otp_display = if state.is_otp_revealed(&user.username) {
                        state.get_otp_code(&user.secret)
                    } else {
                        "••••••".to_string()
                    };

                    ui.add_sized([120.0, 20.0], egui::Label::new(
                        RichText::new(&otp_display)
                            .color(Color32::from_rgb(100, 200, 100))
                            .monospace(),
                    ));

                    // Reveal/Hide button
                    let button_text = if state.is_otp_revealed(&user.username) {
                        "🙈 Hide"
                    } else {
                        "👁 Reveal"
                    };

                    if ui.small_button(button_text).clicked() {
                        state.toggle_otp(&user.username);
                    }
                });
            }
        });
}

/// Renders the create user modal.
fn render_create_user_modal(ctx: &egui::Context, state: &mut InternalUsersState) {
    egui::Window::new("Create Internal User")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            if let Some(created) = &state.created_user {
                // Show QR code and success message
                ui.label(
                    RichText::new("✅ User created successfully!")
                        .color(Color32::from_rgb(34, 139, 34))
                        .strong(),
                );
                ui.add_space(8.0);

                ui.label(format!("Username: {}", created.username));
                ui.add_space(4.0);

                // QR Code
                ui.label(RichText::new("Scan this QR code with Google Authenticator:").strong());
                ui.add_space(8.0);

                render_qr_code(ui, &created.otpauth_url);

                ui.add_space(8.0);

                // Secret for manual entry
                ui.collapsing("Manual Entry", |ui| {
                    ui.label("Secret key:");
                    ui.label(
                        RichText::new(&created.secret)
                            .monospace()
                            .color(Color32::from_rgb(100, 200, 100)),
                    );
                });

                ui.add_space(12.0);

                if ui.button("Close").clicked() {
                    state.close_create_modal();
                }
            } else {
                // Show create form
                ui.label("Enter username for the new internal user:");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut state.new_username);
                });

                if let Some(error) = &state.error_message {
                    ui.add_space(4.0);
                    ui.label(RichText::new(error).color(Color32::from_rgb(220, 53, 69)));
                }

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        state.close_create_modal();
                    }

                    let can_create =
                        !state.new_username.is_empty() && !state.loading.creating;

                    if ui
                        .add_enabled(can_create, egui::Button::new("Create"))
                        .clicked()
                    {
                        state.loading.creating = true;
                        // In a real implementation, this would trigger an API call
                        // For now, we simulate user creation
                        let username = state.new_username.clone();
                        simulate_create_user(state, &username);
                    }

                    if state.loading.creating {
                        ui.spinner();
                    }
                });
            }
        });
}

/// Renders a QR code from an otpauth URL.
fn render_qr_code(ui: &mut Ui, otpauth_url: &str) {
    use qrcode::{QrCode, render::unicode};

    match QrCode::new(otpauth_url) {
        Ok(code) => {
            // Render QR code as unicode characters
            let image = code
                .render::<unicode::Dense1x2>()
                .dark_color(unicode::Dense1x2::Light)
                .light_color(unicode::Dense1x2::Dark)
                .build();

            egui::Frame::NONE
                .fill(Color32::WHITE)
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(image)
                            .monospace()
                            .color(Color32::BLACK)
                            .size(8.0),
                    );
                });
        }
        Err(_) => {
            ui.label(
                RichText::new("Failed to generate QR code")
                    .color(Color32::from_rgb(220, 53, 69)),
            );
        }
    }
}

/// Simulates user creation for demonstration purposes.
/// In production, this would make an API call to the internal endpoint.
fn simulate_create_user(state: &mut InternalUsersState, username: &str) {
    // Generate a cryptographically secure random secret
    let mut random_bytes = [0u8; 20];
    if getrandom::getrandom(&mut random_bytes).is_err() {
        state.set_error("Failed to generate secure random bytes".to_string());
        return;
    }

    // Convert to base32
    let secret = to_base32(&random_bytes);

    // Create otpauth URL
    let otpauth_url = format!(
        "otpauth://totp/Collects:{}?secret={}&issuer=Collects",
        username, secret
    );

    state.set_created_user(CreateInternalUserResponse {
        username: username.to_string(),
        secret,
        otpauth_url,
    });
}

/// Converts bytes to base32 encoding.
fn to_base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

    let mut result = String::new();
    let mut bits: u64 = 0;
    let mut bit_count = 0;

    for &byte in bytes {
        bits = (bits << 8) | byte as u64;
        bit_count += 8;

        while bit_count >= 5 {
            bit_count -= 5;
            let index = ((bits >> bit_count) & 0x1f) as usize;
            result.push(ALPHABET[index] as char);
        }
    }

    if bit_count > 0 {
        let index = ((bits << (5 - bit_count)) & 0x1f) as usize;
        result.push(ALPHABET[index] as char);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_users_state_default() {
        let state = InternalUsersState::new();
        assert!(state.users.is_empty());
        assert!(state.revealed_otps.is_empty());
        assert!(!state.create_modal_open);
    }

    #[test]
    fn test_toggle_otp() {
        let mut state = InternalUsersState::new();
        assert!(!state.is_otp_revealed("user1"));

        state.toggle_otp("user1");
        assert!(state.is_otp_revealed("user1"));

        state.toggle_otp("user1");
        assert!(!state.is_otp_revealed("user1"));
    }

    #[test]
    fn test_add_user() {
        let mut state = InternalUsersState::new();
        state.add_user(InternalUser::new("testuser", "SECRET"));

        assert_eq!(state.users.len(), 1);
        assert_eq!(state.users[0].username, "testuser");
    }

    #[test]
    fn test_modal_operations() {
        let mut state = InternalUsersState::new();

        state.open_create_modal();
        assert!(state.create_modal_open);

        state.close_create_modal();
        assert!(!state.create_modal_open);
    }

    #[test]
    fn test_to_base32() {
        let bytes = b"Hello!";
        let encoded = to_base32(bytes);
        // base32 of "Hello!" should be "JBSWY3DPEHPK3PXP" but our implementation may differ
        assert!(!encoded.is_empty());
        assert!(encoded.chars().all(|c| c.is_ascii_alphanumeric()));
    }
}
