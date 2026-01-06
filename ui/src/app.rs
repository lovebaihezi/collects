#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
use crate::state::AUTH_TOKEN_STORAGE_KEY;
use crate::{
    pages,
    state::State,
    utils::drop_handler::{DropHandler, SystemDropHandler},
    utils::paste_handler::{PasteHandler, SystemPasteHandler},
    widgets,
};
use chrono::{Timelike, Utc};
use collects_business::{ApiStatus, AuthCompute, Route, ToggleApiStatusCommand};
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
use collects_business::{PendingTokenValidation, ValidateTokenCommand};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::RefreshInternalUsersCommand;
use collects_states::Time;

/// Horizontal offset for the API status window from the right edge (in pixels)
const API_STATUS_WINDOW_OFFSET_X: f32 = -8.0;
/// Vertical offset for the API status window from the top edge (in pixels)
const API_STATUS_WINDOW_OFFSET_Y: f32 = 8.0;

/// Main application state and logic for the Collects app.
pub struct CollectsApp<P: PasteHandler = SystemPasteHandler, D: DropHandler = SystemDropHandler> {
    /// The application state (public for testing access).
    pub state: State,
    paste_handler: P,
    /// Whether token validation has been triggered on startup.
    #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
    token_validation_started: bool,
    drop_handler: D,
}

impl CollectsApp<SystemPasteHandler, SystemDropHandler> {
    /// Called once before the first frame.
    pub fn new(state: State) -> Self {
        Self {
            state,
            paste_handler: SystemPasteHandler,
            #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
            token_validation_started: false,
            drop_handler: SystemDropHandler,
        }
    }
}

impl<P: PasteHandler, D: DropHandler> CollectsApp<P, D> {
    /// Create a new app with custom paste and drop handlers (for testing).
    pub fn with_handlers(state: State, paste_handler: P, drop_handler: D) -> Self {
        Self {
            state,
            paste_handler,
            #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
            token_validation_started: false,
            drop_handler,
        }
    }
}

impl<P: PasteHandler, D: DropHandler> eframe::App for CollectsApp<P, D> {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // On first frame (for non-internal builds), try to restore auth from storage
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        if !self.token_validation_started {
            self.token_validation_started = true;
            // Try to load token from storage and validate it
            if let Some(storage) = _frame.storage()
                && let Some(token) = storage.get_string(AUTH_TOKEN_STORAGE_KEY)
                && !token.is_empty()
            {
                log::info!("Found stored auth token, validating...");
                // Set the pending token for validation
                self.state.ctx.update::<PendingTokenValidation>(|pending| {
                    pending.token = Some(token);
                });
                // Dispatch token validation command
                self.state.ctx.dispatch::<ValidateTokenCommand>();
            }
        }

        // Handle paste shortcut (Ctrl+V / Cmd+V) for clipboard image
        // If an image was pasted, replace the current displayed image
        if let Some(clipboard_image) = self.paste_handler.handle_paste(ctx) {
            log::info!(
                "Processing pasted image: {}x{}, {} bytes",
                clipboard_image.width,
                clipboard_image.height,
                clipboard_image.bytes.len()
            );
            let image_state = self.state.ctx.state_mut::<widgets::ImagePreviewState>();
            let success = image_state.set_image_rgba(
                ctx,
                clipboard_image.width,
                clipboard_image.height,
                clipboard_image.bytes,
            );
            if success {
                log::info!("Pasted image set successfully");
            } else {
                log::warn!("Failed to set pasted image");
            }
        }

        // Handle drag-and-drop files for image preview
        // If an image was dropped, replace the current displayed image
        if let Some(dropped_image) = self.drop_handler.handle_drop(ctx) {
            log::info!(
                "Processing dropped image: {}x{}, {} bytes",
                dropped_image.width,
                dropped_image.height,
                dropped_image.bytes.len()
            );
            let image_state = self.state.ctx.state_mut::<widgets::ImagePreviewState>();
            let success = image_state.set_image_rgba(
                ctx,
                dropped_image.width,
                dropped_image.height,
                dropped_image.bytes,
            );
            if success {
                log::info!("Dropped image set successfully");
            } else {
                log::warn!("Failed to set dropped image");
            }
        }

        // Toggle API status display when F1 is pressed
        // Use consume_key to prevent browser default behavior (e.g., Chrome help) in WASM
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F1)) {
            self.state.ctx.dispatch::<ToggleApiStatusCommand>();
        }

        // Update Time state when second changes (chrono::Utc::now() is WASM-compatible)
        // This enables real-time updates for OTP countdown timers while avoiding
        // updates on every frame. Time-dependent computes (ApiStatus, InternalApiStatus)
        // have internal throttling to avoid unnecessary network requests.
        let now = Utc::now();
        let current_time = self.state.ctx.state::<Time>();
        let current_second = current_time.as_ref().second();
        let new_second = now.second();
        if current_second != new_second {
            self.state.ctx.update::<Time>(|t| {
                *t.as_mut() = now;
            });
        }

        // Sync Compute for render
        self.state.ctx.sync_computes();

        // Update route based on authentication state
        self.update_route();

        // Show API status window only when F1 is pressed (toggled)
        let show_api_status = self
            .state
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        if show_api_status {
            egui::Window::new("API Status")
                .anchor(
                    egui::Align2::RIGHT_TOP,
                    egui::vec2(API_STATUS_WINDOW_OFFSET_X, API_STATUS_WINDOW_OFFSET_Y),
                )
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .show(ctx, |ui| {
                    // API status dots (includes internal API for internal builds)
                    // The Window name "API Status" is used for accessibility/kittest queries
                    widgets::api_status(&self.state.ctx, ui);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Render the appropriate page based on current route
            self.render_page(ui);
        });

        // Show drop zone overlay when files are being dragged over the window
        // This provides visual feedback to the user that they can drop files here
        preview_file_being_dropped(ctx);

        // Run background jobs
        self.state.ctx.run_all_dirty();
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        // Save the auth token to storage if authenticated
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        if let Some(auth) = self.state.ctx.cached::<AuthCompute>() {
            if let Some(token) = auth.token() {
                _storage.set_string(AUTH_TOKEN_STORAGE_KEY, token.to_string());
                log::info!("Saved auth token to storage");
            } else {
                // Clear the stored token if not authenticated
                _storage.set_string(AUTH_TOKEN_STORAGE_KEY, String::new());
            }
        }
    }
}

impl<P: PasteHandler, D: DropHandler> CollectsApp<P, D> {
    /// Updates the route based on authentication state.
    fn update_route(&mut self) {
        let is_authenticated = self
            .state
            .ctx
            .cached::<AuthCompute>()
            .is_some_and(|c| c.is_authenticated());

        let current_route = self.state.ctx.state::<Route>().clone();

        let new_route = if is_authenticated {
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            {
                Route::Internal
            }
            #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
            {
                Route::Home
            }
        } else {
            Route::Login
        };

        if current_route != new_route {
            self.state.ctx.update::<Route>(|route| {
                *route = new_route.clone();
            });

            // Auto-fetch users when navigating to Internal route
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            if matches!(new_route, Route::Internal) {
                self.state.ctx.dispatch::<RefreshInternalUsersCommand>();
            }
        }
    }

    /// Renders the appropriate page based on the current route.
    fn render_page(&mut self, ui: &mut egui::Ui) {
        let route = self.state.ctx.state::<Route>().clone();

        match route {
            Route::Login => {
                pages::login_page(&mut self.state, ui);
            }
            Route::Home => {
                pages::home_page(&mut self.state, ui);
            }
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            Route::Internal => {
                pages::internal_page(&mut self.state, ui);
            }
        }
    }
}

/// Shows a drop zone overlay when files are being dragged over the window.
///
/// This function provides visual feedback to users when they drag files over
/// the application window, indicating that they can drop image files to display them.
fn preview_file_being_dropped(ctx: &egui::Context) {
    use egui::{Align2, Area, Color32, Frame, Id, Order, RichText, Stroke, StrokeKind};

    // Check if there are any files being hovered
    let hovered_files = ctx.input(|i| i.raw.hovered_files.clone());
    if hovered_files.is_empty() {
        return;
    }

    // Show overlay with drop zone indicator - get screen rect from raw input or viewport
    let screen_rect = ctx
        .input(|i| i.viewport().outer_rect)
        .or_else(|| ctx.input(|i| i.raw.screen_rect))
        .unwrap_or_else(|| {
            egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, 600.0))
        });

    // Semi-transparent dark overlay
    let painter = ctx.layer_painter(egui::LayerId::new(
        Order::Foreground,
        Id::new("drop_overlay"),
    ));
    painter.rect_filled(screen_rect, 0.0, Color32::from_black_alpha(180));

    // Draw border
    painter.rect_stroke(
        screen_rect.shrink(8.0),
        8.0,
        Stroke::new(3.0, Color32::from_rgb(100, 200, 255)),
        StrokeKind::Outside,
    );

    // Show drop zone message in center
    Area::new(Id::new("drop_zone_message"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            Frame::NONE
                .fill(Color32::from_rgb(40, 40, 50))
                .inner_margin(20.0)
                .outer_margin(10.0)
                .corner_radius(12.0)
                .stroke(Stroke::new(2.0, Color32::from_rgb(100, 200, 255)))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("📷")
                                .size(48.0)
                                .color(Color32::from_rgb(100, 200, 255)),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Drop Image Here")
                                .size(24.0)
                                .color(Color32::WHITE),
                        );
                        ui.add_space(4.0);

                        // Show names of files being dragged
                        let file_names: Vec<String> = hovered_files
                            .iter()
                            .filter_map(get_hovered_file_display_name)
                            .collect();

                        if !file_names.is_empty() {
                            let display_text = if file_names.len() == 1 {
                                file_names[0].clone()
                            } else {
                                format!("{} files", file_names.len())
                            };
                            ui.label(
                                RichText::new(display_text)
                                    .size(14.0)
                                    .color(Color32::LIGHT_GRAY),
                            );
                        }
                    });
                });
        });
}

/// Gets a display name for a hovered file during drag-and-drop.
///
/// Returns the filename if a path is available, the MIME type if available,
/// or None if neither is present.
fn get_hovered_file_display_name(file: &egui::HoveredFile) -> Option<String> {
    // Try to get filename from path
    if let Some(path) = &file.path {
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !filename.is_empty() {
            return Some(filename);
        }
    }

    // Fall back to MIME type
    if !file.mime.is_empty() {
        return Some(format!("({})", file.mime));
    }

    None
}
