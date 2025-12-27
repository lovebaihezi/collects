//! Internal API status widget.
//!
//! This widget displays the status of the internal API,
//! only available for internal/test-internal builds.

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{InternalAPIAvailability, InternalApiStatus};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_states::StateCtx;
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use egui::{Color32, Response, RichText, Ui};

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
pub fn internal_api_status(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    let (text, bg_color, text_color) = match state_ctx
        .cached::<InternalApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(InternalAPIAvailability::Available(_)) => (
            "Internal API: Healthy",
            Color32::from_rgb(34, 139, 34), // Forest green background
            Color32::WHITE,                 // White text
        ),
        Some(InternalAPIAvailability::Unavailable(_)) => (
            "Internal API: Unhealthy",
            Color32::from_rgb(220, 53, 69), // Red background
            Color32::WHITE,                 // White text
        ),
        _ => (
            "Internal API: Checking...",
            Color32::from_rgb(255, 193, 7), // Amber background
            Color32::BLACK,                 // Black text for contrast
        ),
    };

    egui::Frame::NONE
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .outer_margin(egui::Margin::symmetric(0, 4))
        .corner_radius(4.0)
        .show(ui, |ui| ui.label(RichText::new(text).color(text_color)))
        .inner
}
