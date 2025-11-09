use egui::{Color32, Context, Response, Ui};

pub fn api_status(state_ctx: &StateCtx, ctx: &Context, ui: &Ui) -> Response {
    match state_ctx.cached::<ApiStatus>() {
        Some(true) => ui.colored_label(Color32::GREEN, "API Status: Healthy"),
        Some(false) => ui.colored_label(Color32::RED, "API Status: Unhealthy"),
        None => ui.colored_label(Color32::YELLOW, "API Status: Checking..."),
    }
}
