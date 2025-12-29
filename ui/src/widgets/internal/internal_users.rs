//! Internal users management widget.
//!
//! Displays a table of users with their usernames and OTP codes,
//! along with a button to create new users.

use chrono::{DateTime, Utc};
use collects_business::{
    CreateUserCommand, CreateUserCompute, CreateUserInput, CreateUserResult, InternalUserItem,
    ListUsersResponse,
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

        // Collect usernames to toggle (avoiding borrow issues)
        let mut username_to_toggle: Option<String> = None;

        // Users table
        ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("users_table")
                .num_columns(3)
                .striped(true)
                .spacing([40.0, 8.0])
                .show(ui, |ui| {
                    // Header row
                    ui.strong("Username");
                    ui.strong("OTP Code");
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

                        ui.end_row();
                    }
                });
        });

        // Apply toggle action after table iteration
        if let Some(username) = username_to_toggle {
            state.toggle_otp_visibility(&username);
        }
    });

    // Create user modal
    if state.create_modal_open {
        show_create_user_modal(state, state_ctx, ui);
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
                    if state.qr_texture.is_none() {
                        if let Some(qr_image) = generate_qr_image(&created.otpauth_url, 200) {
                            state.qr_texture = Some(ui.ctx().load_texture(
                                "qr_code",
                                qr_image,
                                egui::TextureOptions::NEAREST,
                            ));
                        }
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
}
