//! Internal API status checking.
//!
//! This module provides status checking for the internal API endpoint.
//! It is only available in internal builds (env_internal and env_test_internal).
//!
//! ## Architecture
//!
//! Following the state-model.md guidelines:
//! - `InternalApiStatus` is a **pure cache** that checks conditions in `compute()`
//! - When fetch is needed, `compute()` enqueues `FetchInternalApiStatusCommand` via `Updater`
//! - `FetchInternalApiStatusCommand` performs the network IO and updates the cache
//! - UI just calls `sync_computes()` and `flush_commands()` - no manual scheduling needed

use std::any::{Any, TypeId};

use crate::BusinessConfig;
use chrono::{DateTime, Utc};
use collects_states::{
    Command, CommandSnapshot, Compute, ComputeDeps, Dep, SnapshotClone, State, Time, Updater,
    assign_impl, state_assign_impl,
};
use log::{debug, error, info, warn};

/// Maximum number of retry attempts on failure before waiting for the full interval
const MAX_RETRY_COUNT: u8 = 3;

/// Interval in minutes between internal API status checks
const FETCH_INTERVAL_MINUTES: i64 = 5;

/// Status of the internal API.
#[derive(Default, Debug, Clone)]
pub struct InternalApiStatus {
    last_update_time: Option<DateTime<Utc>>,
    /// If exists error, means internal API unavailable
    last_error: Option<String>,
    /// Number of consecutive failed attempts (resets on success)
    retry_count: u8,
    /// Whether an API fetch is currently in-flight (prevents duplicate requests)
    is_fetching: bool,
}

impl SnapshotClone for InternalApiStatus {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

/// Availability status for internal API.
pub enum InternalAPIAvailability<'a> {
    Available(DateTime<Utc>),
    Unavailable((DateTime<Utc>, &'a str)),
    Unknown,
}

impl InternalApiStatus {
    /// Get the availability status of the internal API.
    pub fn api_availability(&self) -> InternalAPIAvailability<'_> {
        match (self.last_update_time, &self.last_error) {
            (None, None) => InternalAPIAvailability::Unknown,
            (Some(time), None) => InternalAPIAvailability::Available(time),
            (Some(time), Some(err)) => InternalAPIAvailability::Unavailable((time, err.as_str())),
            _ => InternalAPIAvailability::Unknown,
        }
    }

    /// Returns whether an API fetch is currently in-flight
    pub fn is_fetching(&self) -> bool {
        self.is_fetching
    }

    /// Returns the current retry count
    pub fn retry_count(&self) -> u8 {
        self.retry_count
    }

    /// Returns the last update time
    pub fn last_update_time(&self) -> Option<DateTime<Utc>> {
        self.last_update_time
    }

    /// Returns true if a fetch should be triggered based on current state and time.
    ///
    /// Fetch conditions:
    /// 1. Never fetched before -> fetch
    /// 2. FETCH_INTERVAL_MINUTES have passed since last update -> fetch
    /// 3. Had an error and retry count < MAX_RETRY_COUNT -> retry immediately
    pub fn should_fetch(&self, now: DateTime<Utc>) -> bool {
        if self.is_fetching {
            return false;
        }

        match &self.last_update_time {
            Some(last_update_time) => {
                let duration_since_update = now.signed_duration_since(*last_update_time);
                let interval_passed = duration_since_update.num_minutes() >= FETCH_INTERVAL_MINUTES;

                // If we have an error and haven't exceeded max retries, retry immediately
                let should_retry = self.last_error.is_some() && self.retry_count < MAX_RETRY_COUNT;

                interval_passed || should_retry
            }
            None => true,
        }
    }
}

impl Compute for InternalApiStatus {
    fn deps(&self) -> ComputeDeps {
        // Depends on Time to trigger periodic checks
        const STATE_IDS: [TypeId; 1] = [TypeId::of::<Time>()];
        const COMPUTE_IDS: [TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, deps: Dep, updater: Updater) {
        // Check if we should fetch and enqueue the command if needed.
        // This keeps scheduling logic in the compute (which has access to Time)
        // while delegating actual network IO to the command.
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();

        if self.should_fetch(now) {
            log::debug!("InternalApiStatus: enqueueing FetchInternalApiStatusCommand");
            updater.enqueue_command::<FetchInternalApiStatusCommand>();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for InternalApiStatus {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Command to fetch the internal API status from the backend.
///
/// This command performs network IO and updates the `InternalApiStatus` compute cache.
/// It should be dispatched periodically by the UI (e.g., when Time updates and
/// `InternalApiStatus::should_fetch()` returns true).
///
/// Dispatch explicitly via `ctx.enqueue_command::<FetchInternalApiStatusCommand>()`.
#[derive(Default, Debug)]
pub struct FetchInternalApiStatusCommand;

impl Command for FetchInternalApiStatusCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: Updater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let current: InternalApiStatus = snap.compute::<InternalApiStatus>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let time: Time = snap.state::<Time>().clone();

        Box::pin(async move {
            // Skip if already fetching
            if current.is_fetching {
                debug!("Internal API status fetch already in-flight, skipping");
                return;
            }

            let now = time.as_ref().to_utc();

            // Check if we should fetch
            if !current.should_fetch(now) {
                debug!("Internal API status fetch not needed at this time");
                return;
            }

            let url = format!("{}/internal/users", config.api_url().as_str());
            let current_retry_count = current.retry_count;

            if current.last_update_time.is_none() {
                info!("Have not fetched Internal API yet, fetching new status");
            } else if current.last_error.is_some() && current_retry_count < MAX_RETRY_COUNT {
                info!(
                    "Internal API status check failed, retry attempt {}/{}",
                    current_retry_count + 1,
                    MAX_RETRY_COUNT
                );
            } else {
                info!(
                    "Internal API status interval passed, fetching new status at {:?}",
                    now
                );
            }

            info!("Fetching Internal API Status from {:?}", &url);

            // Mark as fetching to prevent duplicate requests
            updater.set(InternalApiStatus {
                last_update_time: current.last_update_time,
                last_error: current.last_error.clone(),
                retry_count: current_retry_count,
                is_fetching: true,
            });

            let client = reqwest::Client::new();
            match client.get(&url).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        debug!("Internal API Available, checked at {:?}", now);
                        updater.set(InternalApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                            retry_count: 0, // Reset retry count on success
                            is_fetching: false,
                        });
                    } else {
                        info!("Internal API Return with status code: {:?}", status);
                        updater.set(InternalApiStatus {
                            last_update_time: Some(now),
                            last_error: Some(format!("Internal API: {}", status)),
                            retry_count: current_retry_count.saturating_add(1),
                            is_fetching: false,
                        });
                    }
                }
                Err(err) => {
                    warn!("Internal API status check failed: {:?}", err);
                    error!("FetchInternalApiStatusCommand: Network error: {}", err);
                    updater.set(InternalApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                        retry_count: current_retry_count.saturating_add(1),
                        is_fetching: false,
                    });
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that InternalApiStatus defaults with is_fetching = false
    #[test]
    fn test_internal_api_status_default_is_fetching_false() {
        let status = InternalApiStatus::default();
        assert!(!status.is_fetching, "is_fetching should default to false");
    }

    /// Tests that is_fetching flag can be set to true
    #[test]
    fn test_internal_api_status_is_fetching_can_be_set() {
        let status = InternalApiStatus {
            last_update_time: None,
            last_error: None,
            retry_count: 0,
            is_fetching: true,
        };
        assert!(status.is_fetching, "is_fetching should be settable to true");
    }

    /// Tests that api_availability returns Unknown when is_fetching is true but no data
    #[test]
    fn test_api_availability_unknown_when_fetching() {
        let status = InternalApiStatus {
            last_update_time: None,
            last_error: None,
            retry_count: 0,
            is_fetching: true,
        };
        assert!(
            matches!(status.api_availability(), InternalAPIAvailability::Unknown),
            "Should return Unknown when fetching with no data"
        );
    }

    /// Tests should_fetch returns true when never fetched
    #[test]
    fn test_should_fetch_when_never_fetched() {
        let status = InternalApiStatus::default();
        let now = Utc::now();
        assert!(
            status.should_fetch(now),
            "should_fetch should return true when never fetched"
        );
    }

    /// Tests should_fetch returns false when is_fetching is true
    #[test]
    fn test_should_fetch_false_when_fetching() {
        let status = InternalApiStatus {
            last_update_time: None,
            last_error: None,
            retry_count: 0,
            is_fetching: true,
        };
        let now = Utc::now();
        assert!(
            !status.should_fetch(now),
            "should_fetch should return false when already fetching"
        );
    }

    /// Tests should_fetch returns false when recently fetched successfully
    #[test]
    fn test_should_fetch_false_when_recently_fetched() {
        let now = Utc::now();
        let status = InternalApiStatus {
            last_update_time: Some(now),
            last_error: None,
            retry_count: 0,
            is_fetching: false,
        };
        assert!(
            !status.should_fetch(now),
            "should_fetch should return false when recently fetched"
        );
    }

    /// Tests should_fetch returns true when interval has passed
    #[test]
    fn test_should_fetch_true_when_interval_passed() {
        let now = Utc::now();
        let old_time = now - chrono::Duration::minutes(FETCH_INTERVAL_MINUTES + 1);
        let status = InternalApiStatus {
            last_update_time: Some(old_time),
            last_error: None,
            retry_count: 0,
            is_fetching: false,
        };
        assert!(
            status.should_fetch(now),
            "should_fetch should return true when interval has passed"
        );
    }

    /// Tests should_fetch returns true for retry on error
    #[test]
    fn test_should_fetch_true_for_retry_on_error() {
        let now = Utc::now();
        let status = InternalApiStatus {
            last_update_time: Some(now), // Just fetched
            last_error: Some("Network error".to_string()),
            retry_count: 1, // Below MAX_RETRY_COUNT
            is_fetching: false,
        };
        assert!(
            status.should_fetch(now),
            "should_fetch should return true for retry on error"
        );
    }

    /// Tests should_fetch returns false when max retries exceeded
    #[test]
    fn test_should_fetch_false_when_max_retries_exceeded() {
        let now = Utc::now();
        let status = InternalApiStatus {
            last_update_time: Some(now),
            last_error: Some("Network error".to_string()),
            retry_count: MAX_RETRY_COUNT, // At max
            is_fetching: false,
        };
        assert!(
            !status.should_fetch(now),
            "should_fetch should return false when max retries exceeded"
        );
    }
}
