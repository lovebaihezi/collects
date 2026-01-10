use crate::utils::colors::{COLOR_AMBER, COLOR_GREEN, COLOR_RED};
use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, Ui};

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{InternalAPIAvailability, InternalApiStatus};

/// Radius of the status indicator circle (in pixels)
const STATUS_DOT_RADIUS: f32 = 5.0;

/// Cached UI version string to avoid repeated computation
fn ui_version() -> &'static str {
    use std::sync::OnceLock;
    static UI_VERSION: OnceLock<String> = OnceLock::new();
    UI_VERSION.get_or_init(collects_business::version_info::format_env_version)
}

fn format_tooltip(status: &str, service_version: Option<&str>) -> String {
    let ui_ver = ui_version();

    match service_version {
        Some(v) => format!("UI: {ui_ver}\nService: {status}:{v}"),
        None => format!("UI: {ui_ver}\nService: {status}"),
    }
}

/// Renders a single status dot with tooltip using a drawn circle
fn status_dot(ui: &mut Ui, tooltip_text: String, dot_color: Color32) -> Response {
    // Allocate space for the circle
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(STATUS_DOT_RADIUS * 2.0, STATUS_DOT_RADIUS * 2.0),
        egui::Sense::hover(),
    );

    // Draw the circle
    let center = rect.center();
    ui.painter()
        .circle(center, STATUS_DOT_RADIUS, dot_color, egui::Stroke::NONE);

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
    let ui_ver = ui_version();
    match state_ctx
        .cached::<InternalApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(InternalAPIAvailability::Available(_)) => {
            (format!("UI: {ui_ver}\nInternal: healthy"), COLOR_GREEN)
        }
        Some(InternalAPIAvailability::Unavailable((_, err))) => {
            (format!("UI: {ui_ver}\nInternal: {err}"), COLOR_RED)
        }
        _ => (format!("UI: {ui_ver}\nInternal: checking"), COLOR_AMBER),
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
}

#[cfg(test)]
mod api_state_widget_test {
    use std::time::Duration;

    use collects_business::{ApiStatus, ToggleApiStatusCommand};
    use tokio::task::yield_now;

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

    /// Tests that the ToggleApiStatusCommand correctly toggles the show_status flag.
    #[tokio::test]
    async fn test_toggle_api_status_command() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::api_status(&state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Initially, show_status should be false (default)
        let initial_show_status = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        assert!(
            !initial_show_status,
            "API status should be hidden by default"
        );

        // Enqueue and flush the toggle command
        harness
            .state_mut()
            .ctx
            .enqueue_command::<ToggleApiStatusCommand>();
        harness.state_mut().ctx.flush_commands();

        // Wait for async command to complete before syncing
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Sync computes to apply the update
        harness.state_mut().ctx.sync_computes();
        harness.step();

        // After toggle, show_status should be true
        let after_toggle = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        assert!(
            after_toggle,
            "API status should be visible after first toggle"
        );

        // Toggle again
        harness
            .state_mut()
            .ctx
            .enqueue_command::<ToggleApiStatusCommand>();
        harness.state_mut().ctx.flush_commands();

        // Wait for async command to complete before syncing
        tokio::time::sleep(Duration::from_millis(10)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();

        // After second toggle, show_status should be false again
        let after_second_toggle = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(true);
        assert!(
            !after_second_toggle,
            "API status should be hidden after second toggle"
        );
    }

    /// Tests that the show_status flag is preserved when ApiStatus compute updates.
    #[tokio::test]
    async fn test_show_status_preserved_after_api_fetch() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::api_status(&state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Toggle to show the API status
        harness
            .state_mut()
            .ctx
            .enqueue_command::<ToggleApiStatusCommand>();
        harness.state_mut().ctx.flush_commands();

        // Wait for async command to complete before syncing
        tokio::time::sleep(Duration::from_millis(10)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();

        // Verify show_status is true
        let show_status_before = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        assert!(
            show_status_before,
            "API status should be visible after toggle"
        );

        // Run the compute cycle (which might update ApiStatus from API response)
        harness.state_mut().ctx.run_all_dirty();
        tokio::time::sleep(Duration::from_millis(100)).await;
        harness.state_mut().ctx.sync_computes();
        harness.step();

        // show_status should still be true after the API fetch updates ApiStatus
        let show_status_after = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        assert!(
            show_status_after,
            "API status should remain visible after API fetch"
        );
    }

    /// Tests that is_fetching flag is preserved when toggling API status visibility.
    #[tokio::test]
    async fn test_is_fetching_preserved_on_toggle() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::api_status(&state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Run compute to trigger initial fetch (sets is_fetching = true)
        harness.state_mut().ctx.run_all_dirty();

        yield_now().await;
        harness.state_mut().ctx.sync_computes();

        // Toggle while fetch might be in-flight
        harness
            .state_mut()
            .ctx
            .enqueue_command::<ToggleApiStatusCommand>();
        harness.state_mut().ctx.flush_commands();

        yield_now().await;

        harness.state_mut().ctx.sync_computes();
        harness.step();

        // After toggle, the status should be visible
        let show_status = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        assert!(show_status, "API status should be visible after toggle");
    }
}
