use crate::{pages, state::State, utils::clipboard, widgets};
use chrono::{Timelike, Utc};
use collects_business::{ApiStatus, AuthCompute, Route, ToggleApiStatusCommand};
use collects_states::Time;

/// Main application state and logic for the Collects app.
pub struct CollectsApp {
    /// The application state (public for testing access).
    pub state: State,
}

impl CollectsApp {
    /// Called once before the first frame.
    pub fn new(state: State) -> Self {
        Self { state }
    }
}

impl eframe::App for CollectsApp {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle paste shortcut (Ctrl+V / Cmd+V) for clipboard image
        clipboard::handle_paste_shortcut(ctx);

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
}

impl CollectsApp {
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
