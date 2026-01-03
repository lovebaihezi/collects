use crate::utils::colors::{COLOR_AMBER, COLOR_GREEN, COLOR_RED};
use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, Ui};

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{InternalAPIAvailability, InternalApiStatus};

fn format_tooltip(status: &str, service_version: Option<&str>) -> String {
    let ui_version = collects_business::version_info::format_env_version();
    
    match service_version {
        Some(v) => format!("UI: {ui_version}\nService: {status}:{v}"),
        None => format!("UI: {ui_version}\nService: {status}"),
    }
}

/// Renders a single status dot with tooltip using a drawn circle
fn status_dot(ui: &mut Ui, tooltip_text: String, dot_color: Color32) -> Response {
    // Allocate space for the circle
    let radius = 5.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(radius * 2.0, radius * 2.0),
        egui::Sense::hover(),
    );
    
    // Draw the circle
    let center = rect.center();
    ui.painter().circle(
        center,
        radius,
        dot_color,
        egui::Stroke::NONE,
    );
    
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
        _ => (format_tooltip("api:checking", None), COLOR_AMBER),
    }
}

/// Get the internal API status dot info (tooltip and color)
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
fn get_internal_api_status_info(state_ctx: &StateCtx) -> (String, Color32) {
    let ui_version = collects_business::version_info::format_env_version();
    match state_ctx
        .cached::<InternalApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(InternalAPIAvailability::Available(_)) => {
            (format!("UI: {ui_version}\nInternal: healthy"), COLOR_GREEN)
        }
        Some(InternalAPIAvailability::Unavailable((_, err))) => {
            (format!("UI: {ui_version}\nInternal: {err}"), COLOR_RED)
        }
        _ => (format!("UI: {ui_version}\nInternal: checking"), COLOR_AMBER),
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

    use crate::test_utils::TestCtx;

    #[tokio::test]
    async fn test_api_status_widget() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::api_status(&state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();

        // Verify the widget renders without errors
        harness.step();

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The Mock Server Needs to wait a bit before it can return 200
        // TODO: finds best practice to wait for mock server
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The widget should render successfully (we can't easily test the drawn circle or tooltip
        // with current kittest capabilities, but we verify no panics/errors occur)
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

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The Mock Server Needs to wait a bit before it can return 404
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The widget should render successfully with error state
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

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The Mock Server Needs to wait a bit before it can return 500
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_all_dirty();

        // The widget should render successfully with error state
    }
}
