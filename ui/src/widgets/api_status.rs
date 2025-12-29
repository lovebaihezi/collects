use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, RichText, Ui};

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{InternalAPIAvailability, InternalApiStatus};

/// Forest green color for healthy/available status
const COLOR_GREEN: Color32 = Color32::from_rgb(34, 139, 34);
/// Red color for error/unavailable status
const COLOR_RED: Color32 = Color32::from_rgb(220, 53, 69);
/// Amber color for checking/pending status
const COLOR_AMBER: Color32 = Color32::from_rgb(255, 193, 7);

fn format_tooltip(status: &str, version: Option<&str>) -> String {
    match version {
        Some(v) => format!("{status}:{v}"),
        None => status.to_string(),
    }
}

/// Renders a single status dot with tooltip
fn status_dot(ui: &mut Ui, tooltip_text: String, dot_color: Color32) -> Response {
    let response = ui.label(RichText::new("●").color(dot_color));
    response.on_hover_text(tooltip_text)
}

/// Get the regular API status dot info (tooltip and color)
fn get_api_status_info(state_ctx: &StateCtx) -> (String, Color32) {
    match state_ctx
        .cached::<ApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(APIAvailability::Available { version, .. }) => {
            (format_tooltip("api", version), COLOR_GREEN)
        }
        Some(APIAvailability::Unavailable { error, version, .. }) => {
            (format_tooltip(&format!("api({error})"), version), COLOR_RED)
        }
        _ => ("api:checking".to_string(), COLOR_AMBER),
    }
}

/// Get the internal API status dot info (tooltip and color)
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
fn get_internal_api_status_info(state_ctx: &StateCtx) -> (String, Color32) {
    match state_ctx
        .cached::<InternalApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(InternalAPIAvailability::Available(_)) => {
            ("internal:healthy".to_string(), COLOR_GREEN)
        }
        Some(InternalAPIAvailability::Unavailable((_, err))) => {
            (format!("internal({err})"), COLOR_RED)
        }
        _ => ("internal:checking".to_string(), COLOR_AMBER),
    }
}

/// Displays the API status indicator(s) centered in the current row.
///
/// For regular builds: shows a single dot for the main API status.
/// For internal builds: shows two dots - one for main API, one for internal API.
///
/// Each dot has a tooltip showing the status details and version information.
pub fn api_status(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    let (api_tooltip, api_color) = get_api_status_info(state_ctx);

    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    let (internal_tooltip, internal_color) = get_internal_api_status_info(state_ctx);

    // Use centered layout for the status dots
    ui.with_layout(
        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
        |ui| {
            ui.horizontal(|ui| {
                // Regular API status dot
                let response = status_dot(ui, api_tooltip, api_color);

                // Internal API status dot (only for internal builds)
                #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
                {
                    ui.add_space(4.0);
                    status_dot(ui, internal_tooltip, internal_color);
                }

                response
            })
            .inner
        },
    )
    .inner
}

#[cfg(test)]
mod api_state_widget_test {
    use std::time::Duration;

    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    /// Helper function to trigger tooltip by hovering and running multiple frames
    fn trigger_tooltip(harness: &mut egui_kittest::Harness<'_, crate::state::State>) {
        // In some configurations there may be multiple status dots rendered.
        // We just hover the first one to trigger the tooltip.
        if let Some(dot) = harness.query_all_by_label("●").next() {
            dot.hover();
        }
        // Run multiple frames to allow tooltip delay to pass
        harness.run_steps(10);
    }

    /// Helper function to check if tooltip contains expected text
    fn has_tooltip_containing(
        harness: &egui_kittest::Harness<'_, crate::state::State>,
        expected: &str,
    ) -> bool {
        harness.query_by_label_contains(expected).is_some()
    }

    #[tokio::test]
    async fn test_api_status_widget() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::api_status(&state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();

        harness.step();

        // Initially shows the status dot (yellow/checking state)
        assert!(
            harness.query_all_by_label("●").count() > 0,
            "Status dot should exist in UI"
        );

        // Trigger tooltip and check it shows checking state
        trigger_tooltip(harness);
        assert!(
            has_tooltip_containing(harness, "api:checking"),
            "Tooltip should show 'api:checking' initially"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The Mock Server Needs to wait a bit before it can return 200
        // TODO: finds best practice to wait for mock server
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // After API response, the dot should still be present (now green)
        assert!(
            harness.query_all_by_label("●").count() > 0,
            "Status dot should exist in UI after API response"
        );

        // Trigger tooltip and check it shows version (mock server returns "0.1.0+test")
        trigger_tooltip(harness);
        assert!(
            has_tooltip_containing(harness, "api:0.1.0+test"),
            "Tooltip should show 'api:0.1.0+test' after successful response"
        );
    }

    #[tokio::test]
    async fn test_api_status_widget_with_404() {
        let mut ctx = TestCtx::new_with_status(
            |ui, state| {
                super::api_status(&state.ctx, ui);
            },
            404,
        )
        .await;

        let harness = ctx.harness_mut();

        harness.step();

        // Initially shows the status dot
        assert!(
            harness.query_all_by_label("●").count() > 0,
            "Status dot should exist in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The Mock Server Needs to wait a bit before it can return 404
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // After API error response, the dot should still be present (now red)
        assert!(
            harness.query_all_by_label("●").count() > 0,
            "Status dot should exist in UI after API error"
        );

        // Trigger tooltip and check it shows error info
        trigger_tooltip(harness);
        assert!(
            has_tooltip_containing(harness, "api("),
            "Tooltip should contain error information after 404"
        );
    }

    #[tokio::test]
    async fn test_api_status_widget_with_500() {
        let mut ctx = TestCtx::new_with_status(
            |ui, state| {
                super::api_status(&state.ctx, ui);
            },
            500,
        )
        .await;

        let harness = ctx.harness_mut();

        harness.step();

        // Initially shows the status dot
        assert!(
            harness.query_all_by_label("●").count() > 0,
            "Status dot should exist in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The Mock Server Needs to wait a bit before it can return 500
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // After API error response, the dot should still be present (now red)
        assert!(
            harness.query_all_by_label("●").count() > 0,
            "Status dot should exist in UI after API error"
        );

        // Trigger tooltip and check it shows error info
        trigger_tooltip(harness);
        assert!(
            has_tooltip_containing(harness, "api("),
            "Tooltip should contain error information after 500"
        );
    }
}
