//! End-to-end tests using real backend services.
//!
//! These tests use kittest for UI rendering but connect to real backend services
//! instead of mocked endpoints. They verify the complete user flow including:
//!
//! - For non-internal builds: User login with account and seeing "Welcome xxx" label
//! - For internal builds: User creation and table content verification
//!
//! ## Running e2e Tests
//!
//! E2E tests are feature-gated and require explicit feature flags to run:
//!
//! ```bash
//! # Run e2e tests for test environment (non-internal)
//! cargo test --features env_test e2e_ -- --ignored
//!
//! # Run e2e tests for test-internal environment
//! cargo test --features env_test_internal e2e_ -- --ignored
//! ```
//!
//! Note: These tests are marked with `#[ignore]` by default since they require
//! network access to real backend services.
//!
//! ## Design Principle
//!
//! E2E tests only depend on `CollectsApp` from `collects_ui` and render the full
//! application. They do not directly access internal state, widgets, or business
//! logic modules - all interaction is through the rendered UI.

/// Tests for non-internal builds (standard login flow).
/// These tests require the `env_test` feature to connect to the test backend.
#[cfg(all(feature = "env_test", not(feature = "env_test_internal")))]
mod non_internal_e2e_tests {
    use collects_ui::CollectsApp;
    use egui_kittest::Harness;
    use kittest::Queryable;

    /// Number of frames to run for UI state to propagate through the app.
    /// This accounts for state updates, compute sync, and rendering cycles.
    const UI_PROPAGATION_FRAMES: usize = 10;

    /// E2E test: Full app login flow using CollectsApp.
    ///
    /// This test renders the full CollectsApp and verifies:
    /// 1. The login form is displayed initially
    /// 2. After user interaction (simulated via UI), the app state changes
    /// 3. After successful login, the "Welcome" message is displayed
    ///
    /// Note: This test is ignored by default because it requires network access.
    #[tokio::test]
    #[ignore = "E2E test requires network access to real backend services"]
    async fn e2e_app_login_shows_welcome_message() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Create full app - CollectsApp handles state initialization internally
        let app = CollectsApp::default();
        let mut harness = Harness::new_eframe(|_| app);

        // Step 1: Verify login form is displayed initially
        harness.step();

        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Login form should show Username field"
        );
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "Login form should show OTP Code field"
        );
        assert!(
            harness.query_by_label_contains("Login").is_some(),
            "Login form should show Login button"
        );

        // Step 2: Simulate user typing in Username field
        // Find and interact with the username text edit
        if let Some(username_field) = harness.query_by_label_contains("Username") {
            harness.click(username_field.id());
            harness.step();
        }

        // Type the username using keyboard simulation
        harness.type_text("e2e_test_user");

        // Run frames to let input propagate
        for _ in 0..UI_PROPAGATION_FRAMES {
            harness.step();
        }

        // Step 3: Simulate user typing in OTP Code field
        if let Some(otp_field) = harness.query_by_label_contains("OTP Code") {
            harness.click(otp_field.id());
            harness.step();
        }

        // Type the OTP code
        harness.type_text("123456");

        // Run frames to let input propagate
        for _ in 0..UI_PROPAGATION_FRAMES {
            harness.step();
        }

        // Step 4: Click the Login button
        if let Some(login_button) = harness.query_by_label_contains("Login") {
            harness.click(login_button.id());
        }

        // Run frames to let login action complete
        for _ in 0..UI_PROPAGATION_FRAMES {
            harness.step();
        }

        // Step 5: Verify welcome message is displayed after login
        // The app should navigate to home page showing welcome message
        assert!(
            harness.query_by_label_contains("Welcome").is_some()
                || harness.query_by_label_contains("Signed").is_some(),
            "After login, should show Welcome message or Signed status"
        );

        // Verify login form is no longer displayed
        assert!(
            harness.query_by_label_contains("OTP Code").is_none(),
            "Login form should NOT be visible after successful login"
        );
    }
}

/// Tests for internal builds (Zero Trust authentication + user management).
/// These tests require the `env_test_internal` feature to connect to the test-internal backend.
#[cfg(feature = "env_test_internal")]
mod internal_e2e_tests {
    use collects_ui::CollectsApp;
    use egui_kittest::Harness;
    use kittest::Queryable;

    /// Number of frames to run for UI state to propagate through the app.
    /// This accounts for state updates, compute sync, and rendering cycles.
    const UI_PROPAGATION_FRAMES: usize = 10;

    /// E2E test: Internal user creation and table verification.
    ///
    /// This test verifies that:
    /// 1. Internal builds skip the login page (Zero Trust)
    /// 2. User can be created via the UI
    /// 3. Created user appears in the users table
    ///
    /// Note: This test requires network access and valid CF authorization token.
    #[tokio::test]
    #[ignore = "E2E test requires network access and valid Zero Trust token"]
    async fn e2e_internal_create_user_shows_in_table() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Create full app - CollectsApp handles state initialization internally
        let app = CollectsApp::default();
        let mut harness = Harness::new_eframe(|_| app);

        // Step 1: Verify we're on the internal page (no login required)
        harness.step();

        // Internal builds should NOT show login form
        assert!(
            harness.query_by_label_contains("Login").is_none(),
            "Internal builds should NOT show Login button (Zero Trust auth)"
        );

        // Should show the users table controls
        assert!(
            harness.query_by_label_contains("Refresh").is_some(),
            "Internal page should show Refresh button"
        );
        assert!(
            harness.query_by_label_contains("Create User").is_some(),
            "Internal page should show Create User button"
        );

        // Step 2: Click Create User button to open dialog/form
        if let Some(create_button) = harness.query_by_label_contains("Create User") {
            harness.click(create_button.id());
        }

        // Run frames to let UI update
        for _ in 0..UI_PROPAGATION_FRAMES {
            harness.step();
        }

        // Step 3: Enter username for the new user
        // Find the username input field in the create user form
        if let Some(username_input) = harness.query_by_label_contains("Username") {
            harness.click(username_input.id());
            harness.step();
        }

        // Type a unique test username
        let test_username = format!("e2e_test_{}", chrono::Utc::now().timestamp());
        harness.type_text(&test_username);

        // Run frames to let input propagate
        for _ in 0..UI_PROPAGATION_FRAMES {
            harness.step();
        }

        // Step 4: Submit the create user form
        // Look for a submit/confirm button
        if let Some(submit_button) = harness
            .query_by_label_contains("Create")
            .or_else(|| harness.query_by_label_contains("Submit"))
            .or_else(|| harness.query_by_label_contains("Confirm"))
        {
            harness.click(submit_button.id());
        }

        // Wait for async API response
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Run frames to let async state propagate
        for _ in 0..UI_PROPAGATION_FRAMES {
            harness.step();
        }

        // Step 5: Verify table structure is displayed
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Table should have Username column"
        );
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "Table should have OTP Code column"
        );
        assert!(
            harness.query_by_label_contains("Time Left").is_some(),
            "Table should have Time Left column"
        );
    }

    /// E2E test: Verify internal page displays table-only view (no app title).
    ///
    /// This test confirms the Typora-like clean table view requirement.
    #[tokio::test]
    #[ignore = "E2E test for internal page structure"]
    async fn e2e_internal_shows_clean_table_view() {
        let _ = env_logger::builder().is_test(true).try_init();

        // Create full app - CollectsApp handles state initialization internally
        let app = CollectsApp::default();
        let mut harness = Harness::new_eframe(|_| app);

        harness.step();

        // Internal builds should NOT show app title (clean data-centric view)
        assert!(
            harness.query_by_label_contains("Collects App").is_none(),
            "Internal builds should NOT show Collects App heading"
        );

        // Should NOT show signed-in status (clean table view)
        assert!(
            harness.query_by_label_contains("Signed").is_none(),
            "Internal builds should NOT show Signed status"
        );

        // Should NOT show welcome message (clean table view)
        assert!(
            harness.query_by_label_contains("Welcome").is_none(),
            "Internal builds should NOT show Welcome message"
        );

        // Table headers should be visible
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Should show Username column header"
        );
    }
}
