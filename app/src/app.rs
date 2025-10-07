use chrono::{DateTime, Utc};
use egui::{
    FontData, FontFamily,
    epaint::text::{FontInsert, FontPriority, InsertFontFamily},
};
use flume::{Receiver, Sender, TryRecvError};
use log::info;
use serde::{Deserialize, Serialize};

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(Deserialize, Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct CollectsApp {
    // inner state
    #[serde(skip)]
    recv: Receiver<bool>,
    #[serde(skip)]
    send: Sender<bool>,

    // state for render
    #[serde(skip)]
    previous_check_api_status: Option<(bool, DateTime<Utc>)>,
}

impl Default for CollectsApp {
    fn default() -> Self {
        let (send, recv) = flume::unbounded();
        Self {
            send,
            recv,
            previous_check_api_status: None,
        }
    }
}

// TODO: not bundle the font in binary
// In Native build, the binary and font should packed to executed.
// In Wasm Build, we will use the font file provided by the HTML page, which loaded by manually font loading and app starting.
#[cfg(not(target_arch = "wasm32"))]
fn add_font(ctx: &egui::Context) {
    let data = FontData::from_static(include_bytes!("../assets/fonts/SourceHanSerifCN-VF.ttf"));
    ctx.add_font(FontInsert::new(
        "source han serif",
        data,
        vec![InsertFontFamily {
            family: FontFamily::Proportional,
            priority: FontPriority::Lowest,
        }],
    ));
}

impl CollectsApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        #[cfg(not(target_arch = "wasm32"))]
        add_font(&cc.egui_ctx);

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    pub fn is_api_health(&mut self) -> Option<bool> {
        // first, if we have one res from the channel, which gives the latest status of the API,
        // so update the previous_check_api_status and previous_check_time
        // if not, check the previous_check_time, if it is None or more than 5 minutes ago, send a new check request
        // if not, return the previous_check_api_status, if previous check api status is None, return false and send a new check request
        let cur_time = Utc::now();

        fn send_check_request(cur_time: &DateTime<Utc>, sender: Sender<bool>) {
            info!("Send one check request at {:?}", cur_time);
            let req = ehttp::Request::get("https://collects.lqxclqxc.com/api/is-health");
            ehttp::fetch(req, move |res| match res {
                Ok(res) => {
                    sender.send(res.status == 200).unwrap_or(());
                }
                Err(_) => {
                    sender.send(false).unwrap_or(());
                }
            });
        }

        match self.recv.try_recv() {
            Ok(res) => {
                self.previous_check_api_status = Some((res, cur_time));
                Some(res)
            }
            Err(TryRecvError::Empty) => match self.previous_check_api_status {
                None => {
                    self.previous_check_api_status = Some((false, cur_time));
                    send_check_request(&cur_time, self.send.clone());
                    None
                }
                Some((previous_status, previous_check_time)) => {
                    if (cur_time - previous_check_time).num_minutes() < 5 {
                        Some(previous_status)
                    } else {
                        self.previous_check_api_status = Some((previous_status, cur_time));
                        send_check_request(&cur_time, self.send.clone());
                        None
                    }
                }
            },
            Err(TryRecvError::Disconnected) => {
                panic!("All Sender got dropped, some bugs exits in code!@")
            }
        }
    }
}

impl eframe::App for CollectsApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        let is_api_health = self.is_api_health();

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                egui::widgets::global_theme_preference_buttons(ui);
                match is_api_health {
                    Some(true) => {
                        ui.colored_label(egui::Color32::GREEN, "API Status: Healthy");
                    }
                    Some(false) => {
                        ui.colored_label(egui::Color32::RED, "API Status: Unhealthy");
                    }
                    None => {
                        ui.colored_label(egui::Color32::YELLOW, "API Status: Checking...");
                    }
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's
            ui.heading("Collects App");
            powered_by_egui_and_eframe(ui);
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
