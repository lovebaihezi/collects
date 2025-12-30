//! Internal users management widget.
//!
//! Displays a table of users with their usernames and OTP codes,
//! along with buttons to create, edit, delete users and manage OTP.

use chrono::{DateTime, Utc};
use collects_business::{
    CreateUserCommand, CreateUserCompute, CreateUserInput, CreateUserResult, DeleteUserResponse,
    GetUserResponse, InternalUserItem, ListUsersResponse, RevokeOtpResponse,
    UpdateUsernameResponse,
};
use collects_states::{StateCtx, Time};
use egui::{Color32, ColorImage, Response, RichText, ScrollArea, TextureHandle, Ui, Window};
use std::any::TypeId;
use std::collections::HashMap;

/// Generate a QR code image from data.
///
/// Returns a `ColorImage` that can be loaded as a texture in egui.
fn generate_qr_image(data: &str, size: usize) -> Option<ColorImage> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    let qr_width = code.width();

    // Calculate scale factor to fit the desired size (minimum scale of 1)
    let scale = (size / qr_width).max(1);
    let actual_size = qr_width * scale;

    // Create pixel buffer
    let mut pixels = vec![Color32::WHITE; actual_size * actual_size];

    for (y, row) in code.to_colors().chunks(qr_width).enumerate() {
        for (x, color) in row.iter().enumerate() {
            let pixel_color = match color {
                qrcode::Color::Dark => Color32::BLACK,
                qrcode::Color::Light => Color32::WHITE,
            };

            // Fill scaled pixels
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x * scale + dx;
                    let py = y * scale + dy;
                    if px < actual_size && py < actual_size {
                        pixels[py * actual_size + px] = pixel_color;
                    }
                }
            }
        }
    }

    Some(ColorImage::new([actual_size, actual_size], pixels))
}

/// Action type for user management.
#[derive(Debug, Clone, PartialEq)]
enum UserAction {
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
    users: Vec<InternalUserItem>,
    /// Map to track which users have their OTP revealed.
    revealed_otps: HashMap<String, bool>,
    /// Whether currently fetching users.
    is_fetching: bool,
    /// Error message if fetch failed.
    error: Option<String>,
    /// Last fetch timestamp (using DateTime<Utc> for WASM compatibility and test mockability).
    last_fetch: Option<DateTime<Utc>>,
    /// Whether the create user modal is open.
    create_modal_open: bool,
    /// Username input for create modal.
    new_username: String,
    /// Cached QR code texture for the created user.
    qr_texture: Option<TextureHandle>,
    /// Current action being performed.
    current_action: UserAction,
    /// Edit username input.
    edit_username_input: String,
    /// Whether an action is in progress.
    action_in_progress: bool,
    /// Action error message.
    action_error: Option<String>,
    /// QR code data for display (otpauth URL).
    qr_code_data: Option<String>,
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
    fn start_action(&mut self, action: UserAction) {
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
    fn close_action(&mut self) {
        self.current_action = UserAction::None;
        self.action_in_progress = false;
        self.action_error = None;
        self.edit_username_input.clear();
        self.qr_texture = None;
        self.qr_code_data = None;
    }

    /// Set action error.
    fn set_action_error(&mut self, error: String) {
        self.action_error = Some(error);
        self.action_in_progress = false;
    }

    /// Set action in progress.
    fn set_action_in_progress(&mut self) {
        self.action_in_progress = true;
        self.action_error = None;
    }

    /// Set QR code data for display.
    fn set_qr_code_data(&mut self, otpauth_url: String) {
        self.qr_code_data = Some(otpauth_url);
        self.action_in_progress = false;
    }
}

/// Displays the internal users panel with a table and create button.
pub fn internal_users_panel(
    state: &mut InternalUsersState,
    state_ctx: &mut StateCtx,
    api_base_url: &str,
    ui: &mut Ui,
) -> Response {
    let response = ui.vertical(|ui| {
        ui.heading("Internal Users");
        ui.separator();

        // Controls row: Refresh and Create buttons
        ui.horizontal(|ui| {
            if ui.button("🔄 Refresh").clicked() && !state.is_fetching {
                state.set_fetching();
                fetch_users(api_base_url, ui.ctx().clone());
            }

            if ui.button("➕ Create User").clicked() {
                // Reset the compute state when opening modal
                reset_create_user_compute(state_ctx);
                state.open_create_modal();
            }

            if state.is_fetching {
                ui.spinner();
                ui.label("Loading...");
            }
        });

        // Error display
        if let Some(error) = &state.error {
            ui.colored_label(Color32::RED, format!("Error: {error}"));
        }

        ui.add_space(8.0);

        // Collect actions (avoiding borrow issues)
        let mut username_to_toggle: Option<String> = None;
        let mut action_to_start: Option<UserAction> = None;

        // Users table
        ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("users_table")
                .num_columns(4)
                .striped(true)
                .spacing([20.0, 8.0])
                .show(ui, |ui| {
                    // Header row
                    ui.strong("Username");
                    ui.strong("OTP Code");
                    ui.strong("OTP");
                    ui.strong("Actions");
                    ui.end_row();

                    // User rows
                    for user in &state.users {
                        ui.label(&user.username);

                        // OTP code with reveal/hide
                        if state.is_otp_revealed(&user.username) {
                            ui.label(RichText::new(&user.current_otp).monospace());
                        } else {
                            ui.label(RichText::new("••••••").monospace());
                        }

                        // Reveal/hide button
                        let button_text = if state.is_otp_revealed(&user.username) {
                            "Hide"
                        } else {
                            "Reveal"
                        };
                        if ui.button(button_text).clicked() {
                            username_to_toggle = Some(user.username.clone());
                        }

                        // Action buttons
                        ui.horizontal(|ui| {
                            if ui.button("📱 QR").on_hover_text("Show QR Code").clicked() {
                                action_to_start = Some(UserAction::ShowQrCode(user.username.clone()));
                            }
                            if ui.button("✏️").on_hover_text("Edit Username").clicked() {
                                action_to_start = Some(UserAction::EditUsername(user.username.clone()));
                            }
                            if ui.button("🔄").on_hover_text("Revoke OTP").clicked() {
                                action_to_start = Some(UserAction::RevokeOtp(user.username.clone()));
                            }
                            if ui.button("🗑️").on_hover_text("Delete User").clicked() {
                                action_to_start = Some(UserAction::DeleteUser(user.username.clone()));
                            }
                        });

                        ui.end_row();
                    }
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
    });

    // Create user modal
    if state.create_modal_open {
        show_create_user_modal(state, state_ctx, ui);
    }

    // Action modals
    match &state.current_action {
        UserAction::ShowQrCode(username) => {
            show_qr_code_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::EditUsername(username) => {
            show_edit_username_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::DeleteUser(username) => {
            show_delete_user_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::RevokeOtp(username) => {
            show_revoke_otp_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::None => {}
    }

    response.response
}

/// Reset the CreateUserCompute to idle state.
fn reset_create_user_compute(state_ctx: &mut StateCtx) {
    // Clear the input
    let input = state_ctx.state_mut::<CreateUserInput>();
    input.username = None;
    // Mark compute as clean so it doesn't auto-run
    state_ctx.mark_clean(&TypeId::of::<CreateUserCompute>());
}

/// Shows the create user modal window.
fn show_create_user_modal(state: &mut InternalUsersState, state_ctx: &mut StateCtx, ui: &mut Ui) {
    let mut open = state.create_modal_open;

    // Get the compute result
    let compute_result = state_ctx
        .cached::<CreateUserCompute>()
        .map(|c| c.result.clone())
        .unwrap_or(CreateUserResult::Idle);

    Window::new("Create User")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            match &compute_result {
                CreateUserResult::Success(created) => {
                    // Show success with QR code info
                    ui.colored_label(
                        Color32::from_rgb(34, 139, 34),
                        "✓ User created successfully!",
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.strong("Username:");
                        ui.label(&created.username);
                    });

                    ui.add_space(8.0);
                    ui.label("Scan this QR code with Google Authenticator:");
                    ui.add_space(4.0);

                    // Generate QR code texture if not cached
                    if state.qr_texture.is_none()
                        && let Some(qr_image) = generate_qr_image(&created.otpauth_url, 200)
                    {
                        state.qr_texture = Some(ui.ctx().load_texture(
                            "qr_code",
                            qr_image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }

                    // Display QR code as an image
                    egui::Frame::NONE
                        .fill(Color32::WHITE)
                        .inner_margin(egui::Margin::same(8))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            if let Some(texture) = &state.qr_texture {
                                ui.image(texture);
                            } else {
                                // Fallback: display the URL as text if QR generation fails
                                ui.label(RichText::new(&created.otpauth_url).monospace().small());
                            }
                        });

                    ui.add_space(8.0);

                    // Show secret for manual entry
                    ui.collapsing("Show secret (for manual entry)", |ui| {
                        ui.label(RichText::new(&created.secret).monospace());
                    });

                    ui.add_space(16.0);
                    if ui.button("Close").clicked() {
                        reset_create_user_compute(state_ctx);
                        state.close_create_modal();
                    }
                }
                CreateUserResult::Error(error) => {
                    // Show error with form to retry
                    ui.colored_label(Color32::RED, format!("Error: {error}"));
                    ui.add_space(8.0);

                    ui.label("Enter a username for the new user:");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut state.new_username);
                    });

                    ui.add_space(16.0);

                    ui.horizontal(|ui| {
                        let can_create = !state.new_username.is_empty();

                        if ui
                            .add_enabled(can_create, egui::Button::new("Retry"))
                            .clicked()
                        {
                            trigger_create_user(state_ctx, &state.new_username);
                        }

                        if ui.button("Cancel").clicked() {
                            reset_create_user_compute(state_ctx);
                            state.close_create_modal();
                        }
                    });
                }
                CreateUserResult::Pending => {
                    // Show loading state
                    ui.label("Creating user...");
                    ui.add_space(8.0);
                    ui.spinner();

                    ui.add_space(16.0);
                    if ui.button("Cancel").clicked() {
                        reset_create_user_compute(state_ctx);
                        state.close_create_modal();
                    }
                }
                CreateUserResult::Idle => {
                    // Show create form
                    ui.label("Enter a username for the new user:");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut state.new_username);
                    });

                    ui.add_space(16.0);

                    ui.horizontal(|ui| {
                        let can_create = !state.new_username.is_empty();

                        if ui
                            .add_enabled(can_create, egui::Button::new("Create"))
                            .clicked()
                        {
                            trigger_create_user(state_ctx, &state.new_username);
                        }

                        if ui.button("Cancel").clicked() {
                            state.close_create_modal();
                        }
                    });
                }
            }
        });

    if !open {
        reset_create_user_compute(state_ctx);
        state.close_create_modal();
    }
}

/// Trigger the create-user side effect by setting input and dispatching the command.
///
/// The command will update `CreateUserCompute` via `Updater`, and the normal
/// `StateCtx::sync_computes()` path will apply the result.
fn trigger_create_user(state_ctx: &mut StateCtx, username: &str) {
    // Update command input state
    state_ctx.update::<CreateUserInput>(|input| {
        input.username = Some(username.to_string());
    });

    // Explicitly dispatch the command (manual-only; never runs implicitly)
    state_ctx.dispatch::<CreateUserCommand>();
}

/// Shows the QR code modal for an existing user.
fn show_qr_code_modal(state: &mut InternalUsersState, api_base_url: &str, username: String, ui: &mut Ui) {
    let mut open = true;

    Window::new(format!("QR Code - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    state.close_action();
                }
                return;
            }

            if state.action_in_progress {
                ui.label("Loading QR code...");
                ui.spinner();
                return;
            }

            if let Some(otpauth_url) = &state.qr_code_data {
                ui.label("Scan this QR code with Google Authenticator:");
                ui.add_space(4.0);

                // Generate QR code texture if not cached
                if state.qr_texture.is_none()
                    && let Some(qr_image) = generate_qr_image(otpauth_url, 200)
                {
                    state.qr_texture = Some(ui.ctx().load_texture(
                        "qr_code_display",
                        qr_image,
                        egui::TextureOptions::NEAREST,
                    ));
                }

                // Display QR code as an image
                egui::Frame::NONE
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        if let Some(texture) = &state.qr_texture {
                            ui.image(texture);
                        } else {
                            ui.label(RichText::new(otpauth_url).monospace().small());
                        }
                    });

                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    state.close_action();
                }
            } else {
                // Fetch user data to get QR code
                state.set_action_in_progress();
                fetch_user_qr_code(api_base_url, &username, ui.ctx().clone());
            }
        });

    if !open {
        state.close_action();
    }
}

/// Shows the edit username modal.
fn show_edit_username_modal(state: &mut InternalUsersState, api_base_url: &str, username: String, ui: &mut Ui) {
    let mut open = true;

    Window::new(format!("Edit Username - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Updating username...");
                ui.spinner();
                return;
            }

            ui.label("Enter the new username:");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("New Username:");
                ui.text_edit_singleline(&mut state.edit_username_input);
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                let can_update = !state.edit_username_input.is_empty() 
                    && state.edit_username_input != username;

                if ui
                    .add_enabled(can_update, egui::Button::new("Update"))
                    .clicked()
                {
                    state.set_action_in_progress();
                    update_username(api_base_url, &username, &state.edit_username_input, ui.ctx().clone());
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state.close_action();
    }
}

/// Shows the delete user confirmation modal.
fn show_delete_user_modal(state: &mut InternalUsersState, api_base_url: &str, username: String, ui: &mut Ui) {
    let mut open = true;

    Window::new(format!("Delete User - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Deleting user...");
                ui.spinner();
                return;
            }

            ui.colored_label(Color32::from_rgb(255, 165, 0), "⚠️ Warning");
            ui.add_space(4.0);
            ui.label(format!("Are you sure you want to delete user '{}'?", username));
            ui.label("This action cannot be undone.");

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Delete").color(Color32::RED))
                    .clicked()
                {
                    state.set_action_in_progress();
                    delete_user(api_base_url, &username, ui.ctx().clone());
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state.close_action();
    }
}

/// Shows the revoke OTP modal.
fn show_revoke_otp_modal(state: &mut InternalUsersState, api_base_url: &str, username: String, ui: &mut Ui) {
    let mut open = true;

    Window::new(format!("Revoke OTP - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Revoking OTP...");
                ui.spinner();
                return;
            }

            // Check if we have new QR code data (after revoke)
            if let Some(otpauth_url) = &state.qr_code_data {
                ui.colored_label(
                    Color32::from_rgb(34, 139, 34),
                    "✓ OTP revoked successfully!",
                );
                ui.add_space(8.0);
                ui.label("The user must scan this new QR code:");
                ui.add_space(4.0);

                // Generate QR code texture if not cached
                if state.qr_texture.is_none()
                    && let Some(qr_image) = generate_qr_image(otpauth_url, 200)
                {
                    state.qr_texture = Some(ui.ctx().load_texture(
                        "qr_code_revoke",
                        qr_image,
                        egui::TextureOptions::NEAREST,
                    ));
                }

                // Display QR code as an image
                egui::Frame::NONE
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        if let Some(texture) = &state.qr_texture {
                            ui.image(texture);
                        } else {
                            ui.label(RichText::new(otpauth_url).monospace().small());
                        }
                    });

                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    state.close_action();
                }
            } else {
                ui.colored_label(Color32::from_rgb(255, 165, 0), "⚠️ Warning");
                ui.add_space(4.0);
                ui.label(format!("Are you sure you want to revoke OTP for user '{}'?", username));
                ui.label("The user will need to re-scan a new QR code.");

                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("Revoke").color(Color32::from_rgb(255, 165, 0)))
                        .clicked()
                    {
                        state.set_action_in_progress();
                        revoke_otp(api_base_url, &username, ui.ctx().clone());
                    }

                    if ui.button("Cancel").clicked() {
                        state.close_action();
                    }
                });
            }
        });

    if !open {
        state.close_action();
    }
}

/// Fetch users from the internal API.
fn fetch_users(api_base_url: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users");
    let request = ehttp::Request::get(&url);

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(list_response) =
                        serde_json::from_slice::<ListUsersResponse>(&response.bytes)
                    {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("internal_users_response"),
                                list_response.users,
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("internal_users_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("internal_users_error"), err.to_string());
                });
            }
        }
    });
}

/// Fetch user QR code from the internal API.
fn fetch_user_qr_code(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{username}");
    let request = ehttp::Request::get(&url);

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(user_response) =
                        serde_json::from_slice::<GetUserResponse>(&response.bytes)
                    {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("user_qr_code_response"),
                                user_response.otpauth_url,
                            );
                        });
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Update username via the internal API.
fn update_username(api_base_url: &str, old_username: &str, new_username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{old_username}");
    let body = serde_json::json!({ "new_username": new_username }).to_string();
    let request = ehttp::Request {
        method: "PUT".to_string(),
        url,
        body: body.into_bytes(),
        headers: ehttp::Headers::new(&[("Content-Type", "application/json")]),
    };

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if serde_json::from_slice::<UpdateUsernameResponse>(&response.bytes).is_ok() {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_success"),
                                "username_updated".to_string(),
                            );
                        });
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Delete user via the internal API.
fn delete_user(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{username}");
    let request = ehttp::Request {
        method: "DELETE".to_string(),
        url,
        body: Vec::new(),
        headers: ehttp::Headers::default(),
    };

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(delete_response) =
                        serde_json::from_slice::<DeleteUserResponse>(&response.bytes)
                    {
                        if delete_response.deleted {
                            ctx.memory_mut(|mem| {
                                mem.data.insert_temp(
                                    egui::Id::new("action_success"),
                                    "user_deleted".to_string(),
                                );
                            });
                        } else {
                            ctx.memory_mut(|mem| {
                                mem.data.insert_temp(
                                    egui::Id::new("action_error"),
                                    "User not found".to_string(),
                                );
                            });
                        }
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Revoke OTP via the internal API.
fn revoke_otp(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users/{username}/revoke");
    let request = ehttp::Request {
        method: "POST".to_string(),
        url,
        body: Vec::new(),
        headers: ehttp::Headers::default(),
    };

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 200 {
                    if let Ok(revoke_response) =
                        serde_json::from_slice::<RevokeOtpResponse>(&response.bytes)
                    {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("revoke_otp_response"),
                                revoke_response.otpauth_url,
                            );
                        });
                    } else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Failed to parse response".to_string(),
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("action_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data
                        .insert_temp(egui::Id::new("action_error"), err.to_string());
                });
            }
        }
    });
}

/// Poll for async responses and update state.
/// Call this in the update loop.
pub fn poll_internal_users_responses(
    state: &mut InternalUsersState,
    state_ctx: &StateCtx,
    ctx: &egui::Context,
) {
    // Check for users list response
    if let Some(users) = ctx.memory(|mem| {
        mem.data
            .get_temp::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"))
    }) {
        // Get current time from Time state for mockability
        let now = *state_ctx.state_mut::<Time>().as_ref();
        state.update_users(users, now);
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
        state.set_error(error);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("internal_users_error"));
        });
    }

    // Check for action error
    if let Some(error) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("action_error"))
    }) {
        state.set_action_error(error);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("action_error"));
        });
    }

    // Check for action success (triggers refresh)
    if let Some(action) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("action_success"))
    }) {
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("action_success"));
        });
        // Close action modal and refresh users list
        state.close_action();
        if action == "user_deleted" || action == "username_updated" {
            state.set_fetching();
            // Note: We need the api_base_url here, but we don't have it in poll.
            // For now, we'll just close the action. The user can manually refresh.
            // A better approach would be to trigger refresh from the modal itself.
        }
    }

    // Check for QR code response
    if let Some(otpauth_url) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("user_qr_code_response"))
    }) {
        state.set_qr_code_data(otpauth_url);
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
        state.set_qr_code_data(otpauth_url);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("revoke_otp_response"));
        });
    }
}
