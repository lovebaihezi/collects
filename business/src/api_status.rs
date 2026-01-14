//! API status checking.
//!
//! This module provides status checking for the main API endpoint.
//!
//! ## Architecture
//!
//! Following the state-model.md guidelines:
//! - `ApiStatus` is a **pure cache** that checks conditions in `compute()`
//! - When fetch is needed, `compute()` enqueues `FetchApiStatusCommand` via `Updater`
//! - `FetchApiStatusCommand` performs the network IO and updates the cache
//! - UI just calls `sync_computes()` and `flush_commands()` - no manual scheduling needed

use std::any::{Any, TypeId};

use crate::BusinessConfig;
use crate::http::Client;
use chrono::{DateTime, Utc};
use collects_states::{
    Command, CommandSnapshot, Compute, ComputeDeps, Dep, LatestOnlyUpdater, SnapshotClone, State,
    Time, Updater, assign_impl, state_assign_impl,
};
use log::{debug, error, info, warn};

/// HTTP header name for the service version
const SERVICE_VERSION_HEADER: &str = "x-service-version";

/// Maximum number of retry attempts on failure before waiting for the full interval
const MAX_RETRY_COUNT: u8 = 3;

/// Interval in minutes between API status checks
const FETCH_INTERVAL_MINUTES: i64 = 5;

#[derive(Default, Debug, Clone)]
pub struct ApiStatus {
    last_update_time: Option<DateTime<Utc>>,
    /// If exists error, means api unavailable
    last_error: Option<String>,
    /// Service version from x-service-version header
    service_version: Option<String>,
    /// Number of consecutive failed attempts (resets on success)
    retry_count: u8,
    /// Whether to show the API status panel (toggled by F1 key)
    show_status: bool,
    /// Whether an API fetch is currently in-flight (prevents duplicate requests)
    is_fetching: bool,
}

impl SnapshotClone for ApiStatus {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

pub enum APIAvailability<'a> {
    Available {
        time: DateTime<Utc>,
        version: Option<&'a str>,
    },
    Unavailable {
        time: DateTime<Utc>,
        error: &'a str,
        version: Option<&'a str>,
    },
    Unknown,
}

impl ApiStatus {
    pub fn api_availability(&self) -> APIAvailability<'_> {
        let version = self.service_version.as_deref();
        match (self.last_update_time, &self.last_error) {
            (Some(time), None) => APIAvailability::Available { time, version },
            (Some(time), Some(err)) => APIAvailability::Unavailable {
                time,
                error: err.as_str(),
                version,
            },
            _ => APIAvailability::Unknown,
        }
    }

    /// Returns whether the API status panel should be shown
    pub fn show_status(&self) -> bool {
        self.show_status
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
    /// 2. `FETCH_INTERVAL_MINUTES` have passed since last update -> fetch
    /// 3. Had an error and retry count < `MAX_RETRY_COUNT` -> retry immediately
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

impl Compute for ApiStatus {
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
            debug!("ApiStatus: enqueueing FetchApiStatusCommand");
            updater.enqueue_command::<FetchApiStatusCommand>();
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for ApiStatus {
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

/// Command to toggle the API status panel visibility.
///
/// Dispatch explicitly via `ctx.enqueue_command::<ToggleApiStatusCommand>()`.
#[derive(Default, Debug)]
pub struct ToggleApiStatusCommand;

impl Command for ToggleApiStatusCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let current: ApiStatus = snap.compute::<ApiStatus>().clone();
        Box::pin(async move {
            let new_show_status = !current.show_status;

            updater.set(ApiStatus {
                last_update_time: current.last_update_time,
                last_error: current.last_error.clone(),
                service_version: current.service_version.clone(),
                retry_count: current.retry_count,
                show_status: new_show_status,
                is_fetching: current.is_fetching,
            });
        })
    }
}

/// Command to fetch the API status from the backend.
///
/// This command performs network IO and updates the `ApiStatus` compute cache.
/// It should be dispatched periodically by the UI (e.g., when Time updates and
/// `ApiStatus::should_fetch()` returns true).
///
/// Dispatch explicitly via `ctx.enqueue_command::<FetchApiStatusCommand>()`.
#[derive(Default, Debug)]
pub struct FetchApiStatusCommand;

impl Command for FetchApiStatusCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let current: ApiStatus = snap.compute::<ApiStatus>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let time: Time = snap.state::<Time>().clone();

        Box::pin(async move {
            // Skip if already fetching
            if current.is_fetching {
                debug!("API status fetch already in-flight, skipping");
                return;
            }

            let now = time.as_ref().to_utc();

            // Check if we should fetch
            if !current.should_fetch(now) {
                debug!("API status fetch not needed at this time");
                return;
            }

            let url = format!("{}/is-health", config.api_url().as_str());
            let current_retry_count = current.retry_count;
            let current_show_status = current.show_status;

            if current.last_update_time.is_none() {
                info!("Not fetched API yet, fetching new status");
            } else if current.last_error.is_some() && current_retry_count < MAX_RETRY_COUNT {
                info!(
                    "API status check failed, retry attempt {}/{}",
                    current_retry_count + 1,
                    MAX_RETRY_COUNT
                );
            } else {
                info!("API status interval passed, fetching new status at {now:?}");
            }

            info!("Fetching API Status from {:?}", &url);

            // Mark as fetching to prevent duplicate requests
            updater.set(ApiStatus {
                last_update_time: current.last_update_time,
                last_error: current.last_error.clone(),
                service_version: current.service_version.clone(),
                retry_count: current_retry_count,
                show_status: current_show_status,
                is_fetching: true,
            });

            match Client::get(&url).send().await {
                Ok(response) => {
                    let service_version = response.header(SERVICE_VERSION_HEADER).map(String::from);
                    if response.is_success() {
                        debug!("BackEnd Available, checked at {now:?}");
                        updater.set(ApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                            service_version,
                            retry_count: 0, // Reset retry count on success
                            show_status: current_show_status,
                            is_fetching: false,
                        });
                    } else {
                        info!("BackEnd Return with status code: {:?}", response.status);
                        updater.set(ApiStatus {
                            last_update_time: Some(now),
                            last_error: Some(format!("API Health: {}", response.status)),
                            service_version,
                            retry_count: current_retry_count.saturating_add(1),
                            show_status: current_show_status,
                            is_fetching: false,
                        });
                    }
                }
                Err(err) => {
                    warn!("API status check failed: {err:?}");
                    error!("FetchApiStatusCommand: Network error: {err}");
                    updater.set(ApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                        service_version: None,
                        retry_count: current_retry_count.saturating_add(1),
                        show_status: current_show_status,
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

    /// Tests that `ApiStatus` defaults with `is_fetching` = false
    #[test]
    fn test_api_status_default_is_fetching_false() {
        let status = ApiStatus::default();
        assert!(!status.is_fetching, "is_fetching should default to false");
    }

    /// Tests that `is_fetching` flag can be set to true
    #[test]
    fn test_api_status_is_fetching_can_be_set() {
        let status = ApiStatus {
            last_update_time: None,
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: false,
            is_fetching: true,
        };
        assert!(status.is_fetching, "is_fetching should be settable to true");
    }

    /// Tests that `api_availability` returns Unknown when `is_fetching` is true but no data
    #[test]
    fn test_api_availability_unknown_when_fetching() {
        let status = ApiStatus {
            last_update_time: None,
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: false,
            is_fetching: true,
        };
        assert!(
            matches!(status.api_availability(), APIAvailability::Unknown),
            "Should return Unknown when fetching with no data"
        );
    }

    /// Tests that `show_status` returns correct value
    #[test]
    fn test_show_status_getter() {
        let status_hidden = ApiStatus {
            last_update_time: None,
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: false,
            is_fetching: false,
        };
        assert!(
            !status_hidden.show_status(),
            "show_status should return false"
        );

        let status_shown = ApiStatus {
            last_update_time: None,
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: true,
            is_fetching: false,
        };
        assert!(status_shown.show_status(), "show_status should return true");
    }

    /// Tests `should_fetch` returns true when never fetched
    #[test]
    fn test_should_fetch_when_never_fetched() {
        let status = ApiStatus::default();
        let now = Utc::now();
        assert!(
            status.should_fetch(now),
            "should_fetch should return true when never fetched"
        );
    }

    /// Tests `should_fetch` returns false when `is_fetching` is true
    #[test]
    fn test_should_fetch_false_when_fetching() {
        let status = ApiStatus {
            last_update_time: None,
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: false,
            is_fetching: true,
        };
        let now = Utc::now();
        assert!(
            !status.should_fetch(now),
            "should_fetch should return false when already fetching"
        );
    }

    /// Tests `should_fetch` returns false when recently fetched successfully
    #[test]
    fn test_should_fetch_false_when_recently_fetched() {
        let now = Utc::now();
        let status = ApiStatus {
            last_update_time: Some(now),
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: false,
            is_fetching: false,
        };
        assert!(
            !status.should_fetch(now),
            "should_fetch should return false when recently fetched"
        );
    }

    /// Tests `should_fetch` returns true when interval has passed
    #[test]
    fn test_should_fetch_true_when_interval_passed() {
        let now = Utc::now();
        let old_time = now - chrono::Duration::minutes(FETCH_INTERVAL_MINUTES + 1);
        let status = ApiStatus {
            last_update_time: Some(old_time),
            last_error: None,
            service_version: None,
            retry_count: 0,
            show_status: false,
            is_fetching: false,
        };
        assert!(
            status.should_fetch(now),
            "should_fetch should return true when interval has passed"
        );
    }

    /// Tests `should_fetch` returns true for retry on error
    #[test]
    fn test_should_fetch_true_for_retry_on_error() {
        let now = Utc::now();
        let status = ApiStatus {
            last_update_time: Some(now), // Just fetched
            last_error: Some("Network error".to_owned()),
            service_version: None,
            retry_count: 1, // Below MAX_RETRY_COUNT
            show_status: false,
            is_fetching: false,
        };
        assert!(
            status.should_fetch(now),
            "should_fetch should return true for retry on error"
        );
    }

    /// Tests `should_fetch` returns false when max retries exceeded
    #[test]
    fn test_should_fetch_false_when_max_retries_exceeded() {
        let now = Utc::now();
        let status = ApiStatus {
            last_update_time: Some(now),
            last_error: Some("Network error".to_owned()),
            service_version: None,
            retry_count: MAX_RETRY_COUNT, // At max
            show_status: false,
            is_fetching: false,
        };
        assert!(
            !status.should_fetch(now),
            "should_fetch should return false when max retries exceeded"
        );
    }
}
