use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, Ui};

pub fn api_status(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    match state_ctx.cached::<ApiStatus>().api_availability() {
        APIAvailability::Available(_) => ui.colored_label(Color32::GREEN, "API Status: Healthy"),
        APIAvailability::Unavailable(_) => ui.colored_label(Color32::RED, "API Status: Unhealthy"),
        _ => ui.colored_label(Color32::YELLOW, "API Status: Checking..."),
    }
}
