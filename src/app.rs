use std::sync::Arc;

use egui::FontData;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
#[derive(Default)]
pub struct Collects {}

// TODO: The font should loaded as a file, not embedded in the binary.
#[expect(clippy::large_include_file)]
const NOTO_SANS_SC_FONT_TTF: &[u8] = include_bytes!("../assets/NotoSansSC-Thin.ttf");

impl Collects {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let fonts = Self::setup_app_fonts();
        cc.egui_ctx.set_fonts(fonts);

        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    fn setup_app_fonts() -> egui::FontDefinitions {
        let mut fonts = egui::FontDefinitions::default();
        let name = "Noto Sans SC".to_owned();

        fonts.font_data.insert(
            name.clone(),
            Arc::new(FontData::from_static(NOTO_SANS_SC_FONT_TTF)),
        );

        // Add the font as fallback for all font families
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push(name.clone());

        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push(name);

        fonts
    }
}

impl eframe::App for Collects {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            // Display different headings based on the build environment
            #[cfg(feature = "preview")]
            ui.heading("Collects (Preview)");

            #[cfg(not(feature = "preview"))]
            ui.heading("Collects");

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
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
