use collects_business::{APIAvailability, ApiStatus};
use collects_states::StateCtx;
use egui::{Color32, Response, RichText, Ui};

pub fn api_status(state_ctx: &StateCtx, ui: &mut Ui) -> Response {
    let (text, bg_color, text_color) = match state_ctx
        .cached::<ApiStatus>()
        .map(|v| v.api_availability())
    {
        Some(APIAvailability::Available(_)) => (
            "API Status: Healthy".to_string(),
            Color32::from_rgb(34, 139, 34), // Forest green background
            Color32::WHITE,                 // White text
        ),
        Some(APIAvailability::Unavailable((_, err))) => (
            err.to_string(),
            Color32::from_rgb(220, 53, 69), // Red background
            Color32::WHITE,                 // White text
        ),
        _ => (
            "API Status: Checking...".to_string(),
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

        assert!(
            harness.query_by_label("API Status: Checking...").is_some(),
            "'API Status: Checking...' should exists in UI"
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

        if let Some(n) = harness.query_by_label_contains("API Status") {
            eprintln!("NODE: {:?}", n);
        }

        assert!(
            harness.query_by_label("API Status: Healthy").is_some(),
            "'API Status: Healthy' should exists in UI"
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

        assert!(
            harness.query_by_label("API Status: Checking...").is_some(),
            "'API Status: Checking...' should exists in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // The Mock Server Needs to wait a bit before it can return 404
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        assert!(
            harness.query_by_label("API Health: 404").is_some(),
            "'API Health: 404' should exists in UI"
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

        assert!(
            harness.query_by_label("API Status: Checking...").is_some(),
            "'API Status: Checking...' should exists in UI"
        );

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        // The Mock Server Needs to wait a bit before it can return 500
        tokio::time::sleep(Duration::from_millis(100)).await;

        harness.state_mut().ctx.sync_computes();
        harness.step();
        harness.state_mut().ctx.run_computed();

        assert!(
            harness.query_by_label("API Health: 500").is_some(),
            "'API Health: 500' should exists in UI"
        );
    }
}
