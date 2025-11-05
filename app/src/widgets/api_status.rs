use chrono::{DateTime, Utc};
use egui::{Color32, Context, Response, Ui, Widget};
use flume::{Sender, TryRecvError};
use log::info;

pub fn api_status(ctx: &Context, ui: &Ui) -> Response {
    match is_api_health {
        Some(true) => ui.colored_label(Color32::GREEN, "API Status: Healthy"),
        Some(false) => ui.colored_label(Color32::RED, "API Status: Unhealthy"),
        None => ui.colored_label(Color32::YELLOW, "API Status: Checking..."),
    }
}

pub fn signin_button(ui: &Ui, ctx: &Context) -> impl Widget + '_ {
    ui.button("Sign In")
}
