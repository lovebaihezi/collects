use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, Ui};

pub fn api_status(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    match state_ctx
        .cached::<ApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(APIAvailability::Available(_)) => {
            ui.colored_label(Color32::GREEN, "API Status: Healthy")
        }
        Some(APIAvailability::Unavailable(_)) => {
            ui.colored_label(Color32::RED, "API Status: Unhealthy")
        }
        _ => ui.colored_label(Color32::YELLOW, "API Status: Checking..."),
    }
}
