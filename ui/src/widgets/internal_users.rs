//! Internal users management widget.
//!
//! This widget displays a table of internal users with their OTP codes,
//! and provides functionality to create new users.
//! Only available for internal/test-internal builds.

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use std::collections::HashSet;

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{BusinessConfig, CreateUserResponse, InternalUsers, create_user};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_states::StateCtx;
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use egui::{Color32, RichText, Ui};

/// State for the internal users panel.
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
#[derive(Default)]
pub struct InternalUsersState {
    /// Set of usernames whose OTP codes are revealed.
    revealed_otps: HashSet<String>,
    /// Whether the create user modal is open.
    show_create_modal: bool,
    /// New user input field.
    new_username: String,
    /// Created user response (for QR code display).
    created_user: Option<CreateUserResponse>,
    /// Error message from creation.
    create_error: Option<String>,
    /// Whether we're in the process of creating a user.
    is_creating: bool,
}

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
impl InternalUsersState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Renders the internal users panel.
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
pub fn internal_users_panel(
    state_ctx: &StateCtx,
    ui: &mut Ui,
    panel_state: &mut InternalUsersState,
) {
    ui.heading("Internal Users Management");
    ui.separator();

    // Create user button
    if ui.button("➕ Create New User").clicked() {
        panel_state.show_create_modal = true;
        panel_state.new_username.clear();
        panel_state.created_user = None;
        panel_state.create_error = None;
    }

    ui.separator();

    // Users table
    let users = state_ctx.cached::<InternalUsers>();

    match users {
        Some(internal_users) => {
            if internal_users.is_fetching {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Loading users...");
                });
            } else if let Some(ref error) = internal_users.last_error {
                ui.colored_label(Color32::RED, format!("Error: {}", error));
            } else if internal_users.users.is_empty() {
                ui.label("No users found.");
            } else {
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        egui::Grid::new("internal_users_grid")
                            .num_columns(3)
                            .spacing([20.0, 8.0])
                            .striped(true)
                            .show(ui, |ui| {
                                // Header
                                ui.label(RichText::new("Username").strong());
                                ui.label(RichText::new("OTP Code").strong());
                                ui.label(RichText::new("Actions").strong());
                                ui.end_row();

                                // Rows
                                for user in &internal_users.users {
                                    ui.label(&user.username);

                                    // OTP code (masked or revealed)
                                    let is_revealed =
                                        panel_state.revealed_otps.contains(&user.username);
                                    if is_revealed {
                                        ui.label(
                                            RichText::new(&user.current_otp)
                                                .monospace()
                                                .color(Color32::from_rgb(34, 139, 34)),
                                        );
                                    } else {
                                        ui.label(
                                            RichText::new("••••••")
                                                .monospace()
                                                .color(Color32::GRAY),
                                        );
                                    }

                                    // Reveal/Hide button
                                    let button_text = if is_revealed { "Hide" } else { "Reveal" };
                                    if ui.button(button_text).clicked() {
                                        if is_revealed {
                                            panel_state.revealed_otps.remove(&user.username);
                                        } else {
                                            panel_state.revealed_otps.insert(user.username.clone());
                                        }
                                    }

                                    ui.end_row();
                                }
                            });
                    });

                // Show last update time
                if let Some(time) = internal_users.last_update_time {
                    ui.separator();
                    ui.label(format!(
                        "Last updated: {}",
                        time.format("%Y-%m-%d %H:%M:%S UTC")
                    ));
                }
            }
        }
        None => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Initializing...");
            });
        }
    }

    // Create user modal
    if panel_state.show_create_modal {
        egui::Window::new("Create New User")
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                // If we have a created user, show the QR code
                if let Some(ref created) = panel_state.created_user {
                    ui.heading("User Created Successfully!");
                    ui.separator();

                    ui.label(format!("Username: {}", created.username));
                    ui.separator();

                    ui.label("Scan this QR code with Google Authenticator:");

                    // Generate QR code for the otpauth URL
                    render_qr_code(ui, &created.otpauth_url);

                    ui.separator();
                    ui.label("Or manually enter this secret:");
                    ui.horizontal(|ui| {
                        ui.monospace(&created.secret);
                        if ui.button("📋 Copy").clicked() {
                            ui.ctx().copy_text(created.secret.clone());
                        }
                    });

                    ui.separator();
                    if ui.button("Close").clicked() {
                        panel_state.show_create_modal = false;
                        panel_state.created_user = None;
                        // Note: The InternalUsers state will auto-refresh on next compute cycle
                    }
                } else {
                    // Input form
                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut panel_state.new_username);
                    });

                    if let Some(ref error) = panel_state.create_error {
                        ui.colored_label(Color32::RED, error);
                    }

                    ui.horizontal(|ui| {
                        let create_enabled =
                            !panel_state.new_username.is_empty() && !panel_state.is_creating;

                        if panel_state.is_creating {
                            ui.spinner();
                            ui.label("Creating...");
                        } else {
                            if ui
                                .add_enabled(create_enabled, egui::Button::new("Create"))
                                .clicked()
                            {
                                let username = panel_state.new_username.clone();

                                // Get the API base URL from config state
                                let config = state_ctx.state_mut::<BusinessConfig>();
                                let api_base = config.api_url().to_string();

                                panel_state.is_creating = true;
                                panel_state.create_error = None;

                                // Note: In a real implementation, we'd use a channel to communicate
                                // the result back. For now, we'll store the callback result.
                                create_user(&api_base, &username, move |result| {
                                    // This callback runs asynchronously
                                    // The result handling will need to be improved
                                    // to properly update the UI state
                                    match result {
                                        Ok(_response) => {
                                            log::info!("User created successfully");
                                        }
                                        Err(err) => {
                                            log::error!("Failed to create user: {}", err);
                                        }
                                    }
                                });
                            }

                            if ui.button("Cancel").clicked() {
                                panel_state.show_create_modal = false;
                            }
                        }
                    });
                }
            });
    }
}

/// Renders a QR code for the given text.
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
fn render_qr_code(ui: &mut Ui, text: &str) {
    // For now, display the URL as text with instructions
    // A full QR code implementation would require adding a QR code generation library
    ui.vertical(|ui| {
        egui::Frame::NONE
            .fill(Color32::from_gray(240))
            .inner_margin(egui::Margin::same(16))
            .corner_radius(8.0)
            .show(ui, |ui| {
                ui.label("📱 QR Code Display");
                ui.separator();
                ui.label(RichText::new("OTPAuth URL:").small());
                ui.horizontal_wrapped(|ui| {
                    ui.monospace(RichText::new(text).small());
                });
                ui.separator();
                ui.label(
                    RichText::new(
                        "Copy this URL and use a QR code generator\nto create a scannable code.",
                    )
                    .small()
                    .italics(),
                );
            });

        if ui.button("📋 Copy URL").clicked() {
            ui.ctx().copy_text(text.to_string());
        }
    });
}
