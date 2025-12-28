use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, RichText, Ui};

fn format_tooltip(status: &str, version: Option<&str>) -> String {
    match version {
        Some(v) => format!("{status}:{v}"),
        None => status.to_string(),
    }
}

pub fn api_status(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    let (tooltip_text, dot_color) = match state_ctx
        .cached::<ApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(APIAvailability::Available { version, .. }) => (
            format_tooltip("api", version),
            Color32::from_rgb(34, 139, 34), // Forest green
        ),
        Some(APIAvailability::Unavailable { error, version, .. }) => (
            format_tooltip(&format!("api({error})"), version),
            Color32::from_rgb(220, 53, 69), // Red
        ),
        _ => (
            "api:checking".to_string(),
            Color32::from_rgb(255, 193, 7), // Amber
        ),
    };

    let response = ui.label(RichText::new("●").color(dot_color));

    response.on_hover_text(tooltip_text)
}

#[cfg(test)]
mod api_state_widget_test {
    use std::time::Duration;

    use kittest::Queryable;

    use crate::test_utils::TestCtx;

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
            harness.query_by_label("●").is_some(),
            "Status dot should exist in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // The Mock Server Needs to wait a bit before it can return 200
        // TODO: finds best practice to wait for mock server
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // After API response, the dot should still be present (now green)
        assert!(
            harness.query_by_label("●").is_some(),
            "Status dot should exist in UI after API response"
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
            harness.query_by_label("●").is_some(),
            "Status dot should exist in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // The Mock Server Needs to wait a bit before it can return 404
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // After API error response, the dot should still be present (now red)
        assert!(
            harness.query_by_label("●").is_some(),
            "Status dot should exist in UI after API error"
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
            harness.query_by_label("●").is_some(),
            "Status dot should exist in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // The Mock Server Needs to wait a bit before it can return 500
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // After API error response, the dot should still be present (now red)
        assert!(
            harness.query_by_label("●").is_some(),
            "Status dot should exist in UI after API error"
        );
    }
}
