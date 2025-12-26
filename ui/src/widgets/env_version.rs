use collects_business::version_info;
use egui::{Color32, Response, Ui};

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

    // Color based on environment
    let color = match env_name {
        "stable" => Color32::GREEN,
        "nightly" => Color32::from_rgb(255, 165, 0), // Orange
        "pr" => Color32::LIGHT_BLUE,
        "internal" => Color32::YELLOW,
        "main" => Color32::from_rgb(200, 200, 200), // Light gray
        _ => Color32::WHITE,
    };

    ui.colored_label(color, display_text)
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
