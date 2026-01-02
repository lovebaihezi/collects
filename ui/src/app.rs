use crate::{state::State, utils::clipboard, widgets};
use chrono::{Timelike, Utc};
use collects_business::AuthCompute;
use collects_states::Time;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct CollectsApp {
    state: State,
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

        // Update Time state only when minute changes (chrono::Utc::now() is WASM-compatible)
        // This avoids triggering time-dependent computes (ApiStatus, InternalApiStatus) every frame
        let now = Utc::now();
        let current_time = self.state.ctx.state_mut::<Time>();
        let current_minute = current_time.as_ref().minute();
        let new_minute = now.minute();
        if current_minute != new_minute {
            self.state.ctx.update::<Time>(|t| {
                *t.as_mut() = now;
            });
        }

        // Sync Compute for render
        self.state.ctx.sync_computes();

        // Poll for async responses (internal builds only)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        widgets::poll_internal_users_responses(
            &mut self.state.internal_users,
            &self.state.ctx,
            ctx,
        );

        // Check if user is authenticated
        let is_authenticated = self
            .state
            .ctx
            .cached::<AuthCompute>()
            .is_some_and(|c| c.is_authenticated());

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // API status dots (includes internal API for internal builds)
                widgets::api_status(&self.state.ctx, ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if is_authenticated {
                // Show main app content when authenticated
                show_authenticated_content(ui, &mut self.state);
            } else {
                // Show login form when not authenticated
                widgets::login_widget(&mut self.state.ctx, ui);
            }
        });

        // Run background jobs
        self.state.ctx.run_all_dirty();
    }
}

/// Shows the main application content when user is authenticated.
fn show_authenticated_content(ui: &mut egui::Ui, state: &mut State) {
    // Get username for display
    let username = state
        .ctx
        .cached::<AuthCompute>()
        .and_then(|c| c.username().map(String::from))
        .unwrap_or_default();

    // Show signed-in header (reusing the shared widget)
    widgets::show_signed_in_header(ui, &username);

    // Show internal users panel for internal builds
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    {
        use collects_business::BusinessConfig;
        ui.add_space(16.0);
        let api_base_url = state
            .ctx
            .state_mut::<BusinessConfig>()
            .api_url()
            .to_string();
        widgets::internal_users_panel(&mut state.internal_users, &mut state.ctx, &api_base_url, ui);
    }

    ui.add_space(16.0);
    powered_by_egui_and_eframe(ui);
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
