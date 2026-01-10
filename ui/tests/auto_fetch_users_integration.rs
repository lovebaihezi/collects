//! Integration tests for auto-fetching users when navigating to Internal route.
//!
//! These tests verify that the user list is automatically fetched when
//! the app starts in internal environment (Zero Trust authentication).
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

mod common;

use crate::common::{DEFAULT_NETWORK_WAIT_MS, TestCtx, yield_wait_for_network};
use collects_business::{InternalUsersListUsersCompute, InternalUsersListUsersResult};
use kittest::Queryable;

/// Tests that users are automatically fetched when the app starts in internal environment.
///
/// In internal builds, the user is authenticated via Zero Trust and should be
/// routed to the Internal route automatically. When this route change happens,
/// the RefreshInternalUsersCommand should be dispatched to fetch the user list.
#[tokio::test]
async fn test_auto_fetch_users_on_startup() {
    let mut ctx = TestCtx::new_app_with_users().await;
    let harness = ctx.harness_mut();

    // Run several frames to allow route change detection and command dispatch
    for _ in 0..10 {
        harness.step();
    }

    // Wait for async API call to complete
    yield_wait_for_network(200).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.state.ctx.sync_computes();
    }

    // Run more frames to process the result
    for _ in 0..5 {
        harness.step();
    }

    // Verify the users list compute is in Loaded state (not Idle)
    let state = harness.state();
    let compute = state.state.ctx.cached::<InternalUsersListUsersCompute>();
    assert!(
        compute.is_some(),
        "InternalUsersListUsersCompute should exist"
    );

    let result = &compute.unwrap().result;
    assert!(
        matches!(result, InternalUsersListUsersResult::Loaded(_)),
        "Users should be automatically loaded on startup, got {:?}",
        result
    );

    // Verify the users data is correct
    if let InternalUsersListUsersResult::Loaded(users) = result {
        assert_eq!(users.len(), 2, "Should have 2 users from mock");
        assert_eq!(users[0].username, "alice");
        assert_eq!(users[1].username, "bob");
    }
}

/// Tests that the user table displays the auto-fetched users.
#[tokio::test]
async fn test_auto_fetched_users_displayed_in_table() {
    let mut ctx = TestCtx::new_app_with_users().await;
    let harness = ctx.harness_mut();

    // Run several frames to allow route change detection and command dispatch
    for _ in 0..10 {
        harness.step();
    }

    yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.state.ctx.sync_computes();
    }

    // Run more frames to render the table with users
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;

    // Verify that user "alice" is displayed in the table
    assert!(
        harness.query_by_label_contains("alice").is_some(),
        "User 'alice' should be displayed in the table after auto-fetch"
    );

    // Verify that user "bob" is displayed in the table
    assert!(
        harness.query_by_label_contains("bob").is_some(),
        "User 'bob' should be displayed in the table after auto-fetch"
    );
}

/// Tests that the compute goes through Loading state during auto-fetch.
#[tokio::test]
async fn test_auto_fetch_shows_loading_state() {
    let mut ctx = TestCtx::new_app_with_users().await;
    let harness = ctx.harness_mut();

    // Run just a few frames to allow route change and command dispatch
    // but not enough time for the async response to complete
    for _ in 0..3 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Sync computes immediately to capture the loading state
    {
        let state = harness.state_mut();
        state.state.ctx.sync_computes();
    }
    yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;

    harness.step();

    // Check the compute state - it should be either Loading or already Loaded
    // (depending on timing, the async response might have already arrived)
    let state = harness.state();
    let compute = state.state.ctx.cached::<InternalUsersListUsersCompute>();
    assert!(
        compute.is_some(),
        "InternalUsersListUsersCompute should exist"
    );

    let result = &compute.unwrap().result;
    // The state should NOT be Idle - it should have transitioned to Loading or Loaded
    assert!(
        !matches!(result, InternalUsersListUsersResult::Idle),
        "Auto-fetch should have started, state should not be Idle, got {:?}",
        result
    );
}
