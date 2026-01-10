//! Integration tests for drag-and-drop image functionality.
//!
//! These tests verify the drag-drop feature works correctly in the context
//! of the full application, including:
//! - Image preview widget on the home page
//! - Drop handler integration
//! - Image display verification using kittest
//!
//! Note: Most tests only run for non-internal builds since the home page
//! (where image preview appears) is only available in non-internal builds.
//! Internal builds route to the internal users table instead.

mod common;

use crate::common::yield_wait_for_network;

/// Tests that only run on non-internal builds (home page specific tests).
/// Internal builds route authenticated users to the internal table, not home page.
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
mod home_page_tests {
    use super::yield_wait_for_network;
    use crate::common::TestCtx;
    use collects_business::{AuthCompute, AuthStatus};
    use collects_ui::state::State;
    use collects_ui::widgets::ImagePreviewState;
    use egui::Context;
    use kittest::Queryable;

    /// Helper to create an authenticated state for testing.
    fn create_authenticated_state(state: &mut State) {
        state.ctx.record_compute(AuthCompute {
            status: AuthStatus::Authenticated {
                username: "TestUser".to_string(),
                token: None,
            },
        });
    }

    #[tokio::test]
    async fn test_drop_image_displays_in_ui() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user to access home page
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        yield_wait_for_network(200).await;
        for _ in 0..5 {
            harness.step();
        }

        // Initially no image should be displayed
        assert!(
            harness.query_by_label_contains("Image:").is_none(),
            "Should not show image initially"
        );

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Simulate dropping an image by directly setting the image state
        // (This mimics what the DropHandler does when processing a dropped file)
        let image_state = harness
            .state_mut()
            .state
            .ctx
            .state_mut::<ImagePreviewState>();

        // Create test image data (simulating decoded dropped image)
        let width = 50;
        let height = 50;
        let rgba_bytes = vec![255u8; width * height * 4]; // White image

        // Set the image using RGBA bytes (same method used by drop handler)
        let success = image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
        assert!(
            success,
            "Should successfully set image from dropped file data"
        );

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // Verify the image is actually displayed on screen using kittest
        assert!(
            harness.query_by_label_contains("Image:").is_some(),
            "Dropped image should be displayed on screen"
        );

        // Verify image dimensions are shown
        assert!(
            harness.query_by_label_contains("50×50").is_some(),
            "Should show dropped image dimensions"
        );
    }

    #[tokio::test]
    async fn test_drop_image_replaces_existing_image() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        yield_wait_for_network(200).await;
        for _ in 0..5 {
            harness.step();
        }

        let egui_ctx = Context::default();

        // Set first image (simulating first drop)
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 100;
            let height = 100;
            let rgba_bytes = vec![255u8; width * height * 4];
            image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
        }

        // Run frames to update UI
        for _ in 0..10 {
            harness.step();
        }

        // Verify first image is displayed
        assert!(
            harness.query_by_label_contains("100×100").is_some(),
            "First dropped image should show 100×100 dimensions"
        );

        // Set second image (simulating second drop - should replace)
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 200;
            let height = 150;
            let rgba_bytes = vec![255u8; width * height * 4];
            image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
        }

        // Run frames to update UI
        for _ in 0..10 {
            harness.step();
        }

        // Verify second image replaced the first
        assert!(
            harness.query_by_label_contains("200×150").is_some(),
            "Second dropped image should replace first and show 200×150 dimensions"
        );
        assert!(
            harness.query_by_label_contains("100×100").is_none(),
            "First image dimensions should no longer be visible"
        );
    }

    #[tokio::test]
    async fn test_drop_image_shows_close_button() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        yield_wait_for_network(200).await;
        for _ in 0..5 {
            harness.step();
        }

        let egui_ctx = Context::default();

        // Simulate dropping an image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 75;
            let height = 75;
            let rgba_bytes = vec![255u8; width * height * 4];
            image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
        }

        // Run frames to update UI
        for _ in 0..10 {
            harness.step();
        }

        // Verify the Close Image button is shown
        assert!(
            harness.query_by_label_contains("Close Image").is_some(),
            "Should show Close Image button when image is dropped"
        );
    }

    #[tokio::test]
    async fn test_drop_invalid_image_data_rejected() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        yield_wait_for_network(200).await;
        for _ in 0..5 {
            harness.step();
        }

        let egui_ctx = Context::default();

        // Try to set invalid image data (wrong byte count)
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 10;
            let height = 10;
            // Only 3 bytes per pixel instead of 4 (RGBA)
            let invalid_bytes = vec![255u8; width * height * 3];

            let success = image_state.set_image_rgba(&egui_ctx, width, height, invalid_bytes);
            assert!(!success, "Should reject invalid RGBA byte count");
            assert!(
                !image_state.has_image(),
                "Should not have image after invalid drop data"
            );
        }

        // Run frames to update UI
        for _ in 0..10 {
            harness.step();
        }

        // Verify no image is displayed
        assert!(
            harness.query_by_label_contains("Image:").is_none(),
            "Should not display image for invalid drop data"
        );
    }

    #[tokio::test]
    async fn test_drop_image_state_persists_after_frames() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        yield_wait_for_network(200).await;
        for _ in 0..5 {
            harness.step();
        }

        let egui_ctx = Context::default();

        // Simulate dropping an image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 120;
            let height = 80;
            let rgba_bytes = vec![255u8; width * height * 4];
            image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
        }

        // Run many frames to ensure state persists
        for _ in 0..50 {
            harness.step();
        }

        // Verify image is still displayed after many frames
        assert!(
            harness.query_by_label_contains("Image:").is_some(),
            "Dropped image should persist after many frames"
        );
        assert!(
            harness.query_by_label_contains("120×80").is_some(),
            "Image dimensions should persist after many frames"
        );

        // Verify the image state is still valid
        let image_state = harness
            .state_mut()
            .state
            .ctx
            .state_mut::<ImagePreviewState>();
        assert!(image_state.has_image(), "Image state should persist");
        let entry = image_state.current_image().unwrap();
        assert_eq!(entry.width, 120);
        assert_eq!(entry.height, 80);
    }
}
