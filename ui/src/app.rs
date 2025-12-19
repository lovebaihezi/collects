use crate::{state::State, widgets};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
pub struct CollectsApp<'a> {
    state: &'a mut State,
}

impl<'a> CollectsApp<'a> {
    /// Called once before the first frame.
    pub fn new(state: &'a mut State) -> Self {
        Self { state }
    }
}

impl<'a> eframe::App for CollectsApp<'a> {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Sync Compute for render
        self.state.ctx.sync_computes();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                widgets::api_status(&self.state.ctx, ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Collects App");
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
