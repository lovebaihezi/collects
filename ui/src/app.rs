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

        // Poll for login results
        widgets::poll_login_result(
            &self.state.login_result_receiver,
            &mut self.state.auth_state,
            &mut self.state.login_dialog_state,
        );

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                widgets::api_status(&self.state.ctx, ui);
                widgets::env_version(ui);

                // Add flexible space to push sign-in button to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::signin_button(
                        &self.state.ctx,
                        &self.state.auth_state,
                        &mut self.state.login_dialog_state,
                        ui,
                    );
                });
            });
        });

        // Show login dialog if open
        if let Some(form_data) = widgets::login_dialog(
            ctx,
            &self.state.ctx,
            &mut self.state.auth_state,
            &mut self.state.login_dialog_state,
        ) {
            // Handle login submission
            widgets::perform_login(
                &self.state.ctx,
                &form_data,
                self.state.login_result_sender.clone(),
            );
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Collects App");

            // Show different content based on login status
            if self.state.auth_state.is_logged_in() {
                if let Some(ref username) = self.state.auth_state.username {
                    ui.label(format!("Welcome, {}!", username));
                }
                ui.add_space(10.0);
                if ui.button("Sign Out").clicked() {
                    self.state.auth_state.logout();
                }
            } else {
                ui.label("Sign in to access your collections.");
            }

            ui.add_space(20.0);
            powered_by_egui_and_eframe(ui);
        });

        // Run background jobs
        self.state.ctx.run_computed();
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
