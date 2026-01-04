//! Integration tests for image paste functionality.
//!
//! These tests verify the image paste feature works correctly in the context
//! of the full application, including:
//! - Image preview widget on the home page
//! - Paste handler integration
//! - State management
//!
//! Note: Most tests only run for non-internal builds since the home page
//! (where image preview appears) is only available in non-internal builds.
//! Internal builds route to the internal users table instead.

mod common;

/// Tests that only run on non-internal builds (home page specific tests).
/// Internal builds route authenticated users to the internal table, not home page.
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
mod home_page_tests {
    use crate::common::TestCtx;
    use collects_business::{AuthCompute, AuthStatus};
    use collects_ui::state::State;
    use collects_ui::widgets::ImagePreviewState;
    use egui::{Color32, ColorImage, Context};
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

    /// Helper to create a test image with the given dimensions.
    fn create_test_image(width: usize, height: usize) -> ColorImage {
        let pixels = vec![Color32::RED; width * height];
        ColorImage::new([width, height], pixels)
    }

    #[tokio::test]
    async fn test_image_preview_visible_on_home_page() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user to access home page
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // The home page should show Welcome message (no "Image Preview" heading anymore)
        assert!(
            harness.query_by_label_contains("Welcome").is_some(),
            "Home page should show Welcome message"
        );

        // No "No image" placeholder text (removed for cleaner UI)
        assert!(
            harness.query_by_label_contains("No image").is_none(),
            "Should not show 'No image' placeholder (removed)"
        );
    }

    #[tokio::test]
    async fn test_image_paste_stores_image_in_state() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Simulate pasting an image by directly setting the image state
        let image_state = harness
            .state_mut()
            .state
            .ctx
            .state_mut::<ImagePreviewState>();

        // Create and set a test image
        let test_image = create_test_image(100, 100);
        image_state.set_image(&egui_ctx, test_image);

        // Verify the image is stored
        assert!(
            image_state.has_image(),
            "Image state should have an image after paste"
        );
        let entry = image_state.current_image().unwrap();
        assert_eq!(entry.width, 100);
        assert_eq!(entry.height, 100);

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // Should not show "No image" placeholder anymore
        assert!(
            harness.query_by_label_contains("No image").is_none(),
            "Should not show 'No image' when an image is present"
        );

        // Verify the image is actually displayed on screen - in fullscreen mode shows "Image: WxH"
        assert!(
            harness.query_by_label_contains("Image:").is_some(),
            "Image should be displayed on screen after pasting"
        );
    }

    #[tokio::test]
    async fn test_image_paste_replaces_previous_image() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Set first image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            let test_image1 = create_test_image(100, 100);
            image_state.set_image(&egui_ctx, test_image1);

            assert_eq!(image_state.current_image().unwrap().width, 100);
        }

        // Set second image - should replace
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            let test_image2 = create_test_image(200, 150);
            image_state.set_image(&egui_ctx, test_image2);

            // Verify replacement
            assert_eq!(
                image_state.current_image().unwrap().width,
                200,
                "Second paste should replace first image"
            );
            assert_eq!(
                image_state.current_image().unwrap().height,
                150,
                "Second paste should replace first image"
            );
        }
    }

    #[tokio::test]
    async fn test_image_preview_maximize_state() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Set an image and maximize it
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            let test_image = create_test_image(100, 100);
            image_state.set_image(&egui_ctx, test_image);
            image_state.set_maximized(true);
        }

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // Should show the maximized window with dimensions in title
        assert!(
            harness.query_by_label_contains("100×100").is_some(),
            "Maximized view should show image dimensions"
        );
    }

    #[tokio::test]
    async fn test_image_clear_removes_image() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Set an image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            let test_image = create_test_image(100, 100);
            image_state.set_image(&egui_ctx, test_image);
            image_state.set_maximized(true);
            assert!(image_state.has_image());
        }

        // Clear the image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            image_state.clear();
            assert!(!image_state.has_image());
            assert!(!image_state.is_maximized());
        }

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // After clearing, should show Welcome message again (no "No image" text anymore)
        assert!(
            harness.query_by_label_contains("Welcome").is_some(),
            "Should show Welcome message after clearing image"
        );
    }

    #[tokio::test]
    async fn test_image_rgba_bytes_integration() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Test setting image from RGBA bytes (simulates clipboard paste)
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 10;
            let height = 10;
            let rgba_bytes = vec![255u8; width * height * 4];

            let success = image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
            assert!(success, "Should successfully set image from RGBA bytes");
            assert!(image_state.has_image());

            let entry = image_state.current_image().unwrap();
            assert_eq!(entry.width, width);
            assert_eq!(entry.height, height);
        }

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        assert!(
            harness.query_by_label_contains("No image").is_none(),
            "Should not show 'No image' when image is set via RGBA bytes"
        );
    }

    #[tokio::test]
    async fn test_invalid_rgba_bytes_rejected() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Test setting image with invalid RGBA bytes (wrong size)
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();

            let width = 10;
            let height = 10;
            // Wrong number of bytes - only 3 bytes per pixel instead of 4
            let rgba_bytes = vec![255u8; width * height * 3];

            let success = image_state.set_image_rgba(&egui_ctx, width, height, rgba_bytes);
            assert!(!success, "Should reject invalid RGBA bytes");
            assert!(
                !image_state.has_image(),
                "Should not have image after invalid data"
            );
        }
    }

    #[tokio::test]
    async fn test_image_displays_fullscreen_without_header() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Initially without image, should show header elements
        assert!(
            harness.query_by_label_contains("Signed").is_some()
                || harness.query_by_label_contains("Welcome").is_some(),
            "Should show header when no image is pasted"
        );

        // Create an egui context for texture creation
        let egui_ctx = Context::default();

        // Paste an image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            let test_image = create_test_image(100, 100);
            image_state.set_image(&egui_ctx, test_image);
        }

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // When image is displayed fullscreen, should show close button
        assert!(
            harness.query_by_label_contains("Close Image").is_some(),
            "Should show Close Image button in fullscreen mode"
        );

        // Should verify the image is actually displayed on screen
        assert!(
            harness.query_by_label_contains("Image:").is_some(),
            "Should show 'Image:' label when image is rendered"
        );

        // Should show image dimensions
        assert!(
            harness.query_by_label_contains("100×100").is_some(),
            "Should show image dimensions in fullscreen mode"
        );
    }

    #[tokio::test]
    async fn test_close_button_returns_to_normal_view() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Authenticate the user
        create_authenticated_state(&mut harness.state_mut().state);

        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        let egui_ctx = Context::default();

        // Paste an image
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            let test_image = create_test_image(100, 100);
            image_state.set_image(&egui_ctx, test_image);
        }

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // Verify we're in fullscreen mode
        assert!(
            harness.query_by_label_contains("Close Image").is_some(),
            "Should be in fullscreen mode with Close button"
        );

        // Clear the image (simulating clicking Close)
        {
            let image_state = harness
                .state_mut()
                .state
                .ctx
                .state_mut::<ImagePreviewState>();
            image_state.clear();
        }

        // Run several frames to let UI update
        for _ in 0..10 {
            harness.step();
        }

        // Should be back to normal view with Welcome message
        assert!(
            harness.query_by_label_contains("Welcome").is_some()
                || harness.query_by_label_contains("Signed").is_some(),
            "Should be back to normal view after clearing image"
        );
    }
}

/// Tests for login page behavior (non-internal builds only).
/// This test verifies that the image preview is NOT visible on the login page.
/// Note: Internal builds use Zero Trust and skip the login page entirely.
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
mod login_page_tests {
    use crate::common::TestCtx;
    use kittest::Queryable;

    #[tokio::test]
    async fn test_image_preview_not_visible_on_login_page() {
        let mut ctx = TestCtx::new_app().await;
        let harness = ctx.harness_mut();

        // Don't authenticate - should show login page
        // Run several frames to let state sync
        for _ in 0..10 {
            harness.step();
        }
        // Wait for async operations
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        for _ in 0..5 {
            harness.step();
        }

        // Login page should NOT show the Image Preview section
        // (Image preview is only on home page after sign-in)
        assert!(
            harness.query_by_label_contains("Image Preview").is_none(),
            "Login page should NOT show Image Preview section"
        );
    }
}
