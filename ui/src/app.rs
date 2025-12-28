use crate::{state::State, widgets};

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
        // Sync Compute for render
        self.state.ctx.sync_computes();

        // Poll for async responses (internal builds only)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        widgets::poll_internal_users_responses(&mut self.state.internal_users, ctx);

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                widgets::api_status(&self.state.ctx, ui);
                // Show internal API status for internal builds
                #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
                widgets::internal_api_status(&self.state.ctx, ui);
                widgets::env_version(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Collects App");

            // Show internal users panel for internal builds
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            {
                use collects_business::BusinessConfig;
                ui.add_space(16.0);
                let api_base_url = self
                    .state
                    .ctx
                    .state_mut::<BusinessConfig>()
                    .api_url()
                    .to_string();
                widgets::internal_users_panel(
                    &mut self.state.internal_users,
                    &mut self.state.ctx,
                    &api_base_url,
                    ui,
                );
            }

            ui.add_space(16.0);
            powered_by_egui_and_eframe(ui);
        });

        // Run background jobs
        self.state.ctx.run_all_dirty();
    }
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
