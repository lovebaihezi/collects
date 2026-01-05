use crate::{
    pages,
    state::{State, AUTH_TOKEN_STORAGE_KEY},
    utils::paste_handler::{PasteHandler, SystemPasteHandler},
    widgets,
};
use chrono::{Timelike, Utc};
use collects_business::{
    ApiStatus, AuthCompute, PendingTokenValidation, Route, ToggleApiStatusCommand,
    ValidateTokenCommand,
};
use collects_states::Time;

/// Main application state and logic for the Collects app.
pub struct CollectsApp<P: PasteHandler = SystemPasteHandler> {
    /// The application state (public for testing access).
    pub state: State,
    paste_handler: P,
    /// Whether token validation has been triggered on startup.
    token_validation_started: bool,
}

impl CollectsApp<SystemPasteHandler> {
    /// Called once before the first frame.
    pub fn new(state: State) -> Self {
        Self {
            state,
            paste_handler: SystemPasteHandler,
            token_validation_started: false,
        }
    }
}

impl<P: PasteHandler> CollectsApp<P> {
    /// Create a new app with a custom paste handler (for testing).
    pub fn with_paste_handler(state: State, paste_handler: P) -> Self {
        Self {
            state,
            paste_handler,
            token_validation_started: false,
        }
    }
}

impl<P: PasteHandler> eframe::App for CollectsApp<P> {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // On first frame (for non-internal builds), try to restore auth from storage
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        if !self.token_validation_started {
            self.token_validation_started = true;
            // Try to load token from storage and validate it
            if let Some(storage) = _frame.storage() {
                if let Some(token) = storage.get_string(AUTH_TOKEN_STORAGE_KEY) {
                    if !token.is_empty() {
                        log::info!("Found stored auth token, validating...");
                        // Set the pending token for validation
                        self.state.ctx.update::<PendingTokenValidation>(|pending| {
                            pending.token = Some(token);
                        });
                        // Dispatch token validation command
                        self.state.ctx.dispatch::<ValidateTokenCommand>();
                    }
                }
            }
        }

        // Handle paste shortcut (Ctrl+V / Cmd+V) for clipboard image
        // If an image was pasted, replace the current displayed image
        if let Some(clipboard_image) = self.paste_handler.handle_paste(ctx) {
            let image_state = self.state.ctx.state_mut::<widgets::ImagePreviewState>();
            image_state.set_image_rgba(
                ctx,
                clipboard_image.width,
                clipboard_image.height,
                clipboard_image.bytes,
            );
        }

        // Toggle API status display when F1 is pressed
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.state.ctx.dispatch::<ToggleApiStatusCommand>();
        }

        // Update Time state when second changes (chrono::Utc::now() is WASM-compatible)
        // This enables real-time updates for OTP countdown timers while avoiding
        // updates on every frame. Time-dependent computes (ApiStatus, InternalApiStatus)
        // have internal throttling to avoid unnecessary network requests.
        let now = Utc::now();
        let current_time = self.state.ctx.state_mut::<Time>();
        let current_second = current_time.as_ref().second();
        let new_second = now.second();
        if current_second != new_second {
            self.state.ctx.update::<Time>(|t| {
                *t.as_mut() = now;
            });
        }

        // Sync Compute for render
        self.state.ctx.sync_computes();

        // Poll for async responses (internal builds only)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        widgets::poll_internal_users_responses(&mut self.state.ctx, ctx);

        // Update route based on authentication state
        self.update_route();

        // Show top panel with API status only when F1 is pressed (toggled)
        let show_api_status = self
            .state
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        if show_api_status {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    // Label for accessibility and kittest queries
                    ui.label("API Status");
                    // API status dots (includes internal API for internal builds)
                    widgets::api_status(&self.state.ctx, ui);
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Render the appropriate page based on current route
            self.render_page(ui);
        });

        // Run background jobs
        self.state.ctx.run_all_dirty();
    }

    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // Save the auth token to storage if authenticated
        #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
        if let Some(auth) = self.state.ctx.cached::<AuthCompute>() {
            if let Some(token) = auth.token() {
                storage.set_string(AUTH_TOKEN_STORAGE_KEY, token.to_string());
                log::info!("Saved auth token to storage");
            } else {
                // Clear the stored token if not authenticated
                storage.set_string(AUTH_TOKEN_STORAGE_KEY, String::new());
            }
        }
    }
}

impl<P: PasteHandler> CollectsApp<P> {
    /// Updates the route based on authentication state.
    fn update_route(&mut self) {
        let is_authenticated = self
            .state
            .ctx
            .cached::<AuthCompute>()
            .is_some_and(|c| c.is_authenticated());

        let current_route = self.state.ctx.state_mut::<Route>().clone();

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
                *route = new_route;
            });
        }
    }

    /// Renders the appropriate page based on the current route.
    fn render_page(&mut self, ui: &mut egui::Ui) {
        let route = self.state.ctx.state_mut::<Route>().clone();

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
