//! Internal users management widget.
//!
//! Displays a table of users with their usernames and OTP codes,
//! along with a button to create new users.

use collects_business::{CreateUserRequest, CreateUserResponse, InternalUserItem, ListUsersResponse};
use egui::{Color32, Response, RichText, ScrollArea, Ui, Window};
use std::collections::HashMap;

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
    /// Last fetch timestamp.
    last_fetch: Option<std::time::Instant>,
    /// Whether the create user modal is open.
    create_modal_open: bool,
    /// Username input for create modal.
    new_username: String,
    /// Whether currently creating a user.
    is_creating: bool,
    /// Created user response (for showing QR code).
    created_user: Option<CreateUserResponse>,
    /// Error from user creation.
    create_error: Option<String>,
}

impl InternalUsersState {
    /// Create a new internal users state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggle OTP visibility for a user.
    pub fn toggle_otp_visibility(&mut self, username: &str) {
        let revealed = self.revealed_otps.entry(username.to_string()).or_insert(false);
        *revealed = !*revealed;
    }

    /// Check if OTP is revealed for a user.
    pub fn is_otp_revealed(&self, username: &str) -> bool {
        self.revealed_otps.get(username).copied().unwrap_or(false)
    }

    /// Update users from API response.
    pub fn update_users(&mut self, users: Vec<InternalUserItem>) {
        self.users = users;
        self.is_fetching = false;
        self.error = None;
        self.last_fetch = Some(std::time::Instant::now());
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
        self.created_user = None;
        self.create_error = None;
    }

    /// Close create user modal.
    pub fn close_create_modal(&mut self) {
        self.create_modal_open = false;
        self.new_username.clear();
        self.created_user = None;
        self.create_error = None;
        self.is_creating = false;
    }

    /// Set created user response.
    pub fn set_created_user(&mut self, response: CreateUserResponse) {
        self.created_user = Some(response);
        self.is_creating = false;
        self.create_error = None;
    }

    /// Set create error.
    pub fn set_create_error(&mut self, error: String) {
        self.create_error = Some(error);
        self.is_creating = false;
    }
}

/// Displays the internal users panel with a table and create button.
pub fn internal_users_panel(
    state: &mut InternalUsersState,
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
                            let username = user.username.clone();
                            // We need to toggle after the loop to avoid borrow issues
                            ui.memory_mut(|mem| {
                                mem.data.insert_temp(
                                    egui::Id::new("toggle_otp_username"),
                                    username,
                                );
                            });
                        }

                        ui.end_row();
                    }
                });
        });

        // Handle toggle after iteration
        if let Some(username) = ui.memory(|mem| {
            mem.data.get_temp::<String>(egui::Id::new("toggle_otp_username"))
        }) {
            state.toggle_otp_visibility(&username);
            ui.memory_mut(|mem| {
                mem.data.remove::<String>(egui::Id::new("toggle_otp_username"));
            });
        }
    });

    // Create user modal
    if state.create_modal_open {
        show_create_user_modal(state, api_base_url, ui);
    }

    response.response
}

/// Shows the create user modal window.
fn show_create_user_modal(state: &mut InternalUsersState, api_base_url: &str, ui: &mut Ui) {
    let mut open = state.create_modal_open;

    Window::new("Create User")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(created) = &state.created_user {
                // Show success with QR code info
                ui.colored_label(Color32::from_rgb(34, 139, 34), "✓ User created successfully!");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.strong("Username:");
                    ui.label(&created.username);
                });

                ui.add_space(8.0);
                ui.label("Scan this QR code with Google Authenticator:");
                ui.add_space(4.0);

                // Display the otpauth URL (in a real app, render as QR code)
                egui::Frame::NONE
                    .fill(Color32::from_gray(240))
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(&created.otpauth_url).monospace().small());
                    });

                ui.add_space(8.0);

                // Show secret for manual entry
                ui.collapsing("Show secret (for manual entry)", |ui| {
                    ui.label(RichText::new(&created.secret).monospace());
                });

                ui.add_space(16.0);
                if ui.button("Close").clicked() {
                    state.close_create_modal();
                }
            } else {
                // Show create form
                ui.label("Enter a username for the new user:");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Username:");
                    ui.text_edit_singleline(&mut state.new_username);
                });

                // Error display
                if let Some(error) = &state.create_error {
                    ui.add_space(4.0);
                    ui.colored_label(Color32::RED, error);
                }

                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    let can_create = !state.new_username.is_empty() && !state.is_creating;

                    if ui.add_enabled(can_create, egui::Button::new("Create")).clicked() {
                        state.is_creating = true;
                        let username = state.new_username.clone();
                        create_user(api_base_url, &username, ui.ctx().clone());
                    }

                    if state.is_creating {
                        ui.spinner();
                    }

                    if ui.button("Cancel").clicked() {
                        state.close_create_modal();
                    }
                });
            }
        });

    if !open {
        state.close_create_modal();
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
                    if let Ok(list_response) = serde_json::from_slice::<ListUsersResponse>(&response.bytes) {
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
                    mem.data.insert_temp(
                        egui::Id::new("internal_users_error"),
                        err.to_string(),
                    );
                });
            }
        }
    });
}

/// Create a new user via the internal API.
fn create_user(api_base_url: &str, username: &str, ctx: egui::Context) {
    let url = format!("{api_base_url}/internal/users");
    let body = serde_json::to_vec(&CreateUserRequest {
        username: username.to_string(),
    })
    .unwrap_or_default();

    let mut request = ehttp::Request::post(&url, body);
    request.headers.insert("Content-Type", "application/json");

    ehttp::fetch(request, move |result| {
        ctx.request_repaint();
        match result {
            Ok(response) => {
                if response.status == 201 {
                    if let Ok(create_response) = serde_json::from_slice::<CreateUserResponse>(&response.bytes) {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("internal_create_user_response"),
                                create_response,
                            );
                        });
                    }
                } else {
                    ctx.memory_mut(|mem| {
                        mem.data.insert_temp(
                            egui::Id::new("internal_create_user_error"),
                            format!("API returned status: {}", response.status),
                        );
                    });
                }
            }
            Err(err) => {
                ctx.memory_mut(|mem| {
                    mem.data.insert_temp(
                        egui::Id::new("internal_create_user_error"),
                        err.to_string(),
                    );
                });
            }
        }
    });
}

/// Poll for async responses and update state.
/// Call this in the update loop.
pub fn poll_internal_users_responses(state: &mut InternalUsersState, ctx: &egui::Context) {
    // Check for users list response
    if let Some(users) = ctx.memory(|mem| {
        mem.data.get_temp::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"))
    }) {
        state.update_users(users);
        ctx.memory_mut(|mem| {
            mem.data.remove::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"));
        });
    }

    // Check for users list error
    if let Some(error) = ctx.memory(|mem| {
        mem.data.get_temp::<String>(egui::Id::new("internal_users_error"))
    }) {
        state.set_error(error);
        ctx.memory_mut(|mem| {
            mem.data.remove::<String>(egui::Id::new("internal_users_error"));
        });
    }

    // Check for create user response
    if let Some(response) = ctx.memory(|mem| {
        mem.data.get_temp::<CreateUserResponse>(egui::Id::new("internal_create_user_response"))
    }) {
        state.set_created_user(response);
        ctx.memory_mut(|mem| {
            mem.data.remove::<CreateUserResponse>(egui::Id::new("internal_create_user_response"));
        });
    }

    // Check for create user error
    if let Some(error) = ctx.memory(|mem| {
        mem.data.get_temp::<String>(egui::Id::new("internal_create_user_error"))
    }) {
        state.set_create_error(error);
        ctx.memory_mut(|mem| {
            mem.data.remove::<String>(egui::Id::new("internal_create_user_error"));
        });
    }
}
