use collects_business::version_info;
use egui::{Color32, Response, RichText, Ui};

/// Displays the current environment and version/info in the UI.
///
/// Display format varies by environment:
/// - PR: `pr:{number}`
/// - Prod (stable): `stable:{version}`
/// - Nightly: `nightly:{date}`
/// - Internal: `internal:{commit}`
/// - Main/Test: `main:{commit}`
pub fn env_version(ui: &mut Ui) -> Response {
    let display_text = version_info::format_env_version();
    let (env_name, _) = version_info::env_version_info();

    // Background color and text color based on environment
    let (bg_color, text_color) = match env_name {
        "stable" => (
            Color32::from_rgb(34, 139, 34), // Forest green background
            Color32::WHITE,                 // White text
        ),
        "nightly" => (
            Color32::from_rgb(255, 140, 0), // Dark orange background
            Color32::WHITE,                 // White text
        ),
        "pr" => (
            Color32::from_rgb(13, 110, 253), // Blue background
            Color32::WHITE,                  // White text
        ),
        "internal" => (
            Color32::from_rgb(255, 193, 7), // Amber background
            Color32::BLACK,                 // Black text for contrast
        ),
        "main" => (
            Color32::from_rgb(108, 117, 125), // Gray background
            Color32::WHITE,                   // White text
        ),
        _ => (
            Color32::from_rgb(108, 117, 125), // Gray background
            Color32::WHITE,                   // White text
        ),
    };

    egui::Frame::NONE
        .fill(bg_color)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.label(RichText::new(display_text).color(text_color))
        })
        .inner
}

#[cfg(test)]
mod env_version_widget_test {
    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    #[tokio::test]
    async fn test_env_version_widget() {
        let mut ctx = TestCtx::new(|ui, _state| {
            super::env_version(ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // The widget should display something containing a colon (env:info format)
        let found = harness.query_by_label_contains(":");
        assert!(
            found.is_some(),
            "env_version widget should display format like 'env:info'"
        );
    }
}
