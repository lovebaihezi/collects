//! Internal API status checking.
//!
//! This module provides status checking for the internal API endpoint.
//! It is only available in internal builds (env_internal and env_test_internal).

use std::any::{Any, TypeId};

use collects_states::state_assign_impl;

use crate::BusinessConfig;
use chrono::{DateTime, Utc};
use collects_states::{
    Compute, ComputeDeps, Dep, SnapshotClone, State, Time, Updater, assign_impl,
};
use log::{debug, info, warn};
use ustr::Ustr;

/// Maximum number of retry attempts on failure before waiting for the full interval
const MAX_RETRY_COUNT: u8 = 3;

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
}

impl Compute for InternalApiStatus {
    fn deps(&self) -> ComputeDeps {
        const IDS: [TypeId; 2] = [TypeId::of::<Time>(), TypeId::of::<BusinessConfig>()];
        (&IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) {
        // Skip if a fetch is already in-flight to prevent duplicate requests
        if self.is_fetching {
            debug!("Internal API status fetch already in-flight, skipping");
            return;
        }

        let config = deps.get_state_ref::<BusinessConfig>();
        let url = Ustr::from(format!("{}/internal/users", config.api_url().as_str()).as_str());
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();
        let current_retry_count = self.retry_count;

        // Determine if we should fetch:
        // 1. Never fetched before -> fetch
        // 2. 5 minutes have passed since last update -> fetch
        // 3. Had an error and retry count < MAX_RETRY_COUNT -> retry immediately
        let should_fetch = match &self.last_update_time {
            Some(last_update_time) => {
                let duration_since_update = now.signed_duration_since(*last_update_time);
                let interval_passed = duration_since_update.num_minutes() >= 5;

                // If we have an error and haven't exceeded max retries, retry immediately
                let should_retry =
                    self.last_error.is_some() && current_retry_count < MAX_RETRY_COUNT;

                if interval_passed {
                    info!(
                        "Internal API status last updated at {:?}, now is {:?}, should fetch new status",
                        last_update_time, now
                    );
                } else if should_retry {
                    info!(
                        "Internal API status check failed, retry attempt {}/{}",
                        current_retry_count + 1,
                        MAX_RETRY_COUNT
                    );
                }

                interval_passed || should_retry
            }
            None => {
                info!("Have not fetched Internal API yet, should fetch new status");
                true
            }
        };
        if should_fetch {
            info!(
                "Fetching Internal API Status at {:?} on: {:?}, Waiting Result",
                &url, now
            );
            // Mark as fetching to prevent duplicate requests while this one is in-flight.
            updater.set(InternalApiStatus {
                last_update_time: self.last_update_time,
                last_error: self.last_error.clone(),
                retry_count: current_retry_count,
                is_fetching: true,
            });

            let url_string = url.to_string();
            tokio::spawn(async move {
                let client = reqwest::Client::new();
                match client.get(&url_string).send().await {
                    Ok(response) => {
                        let status = response.status();
                        if status.is_success() {
                            debug!("Internal API Available, checked at {:?}", now);
                            let api_status = InternalApiStatus {
                                last_update_time: Some(now),
                                last_error: None,
                                retry_count: 0, // Reset retry count on success
                                is_fetching: false,
                            };
                            updater.set(api_status);
                        } else {
                            info!("Internal API Return with status code: {:?}", status);
                            let api_status = InternalApiStatus {
                                last_update_time: Some(now),
                                last_error: Some(format!("Internal API: {}", status)),
                                retry_count: current_retry_count.saturating_add(1),
                                is_fetching: false,
                            };
                            updater.set(api_status);
                        }
                    }
                    Err(err) => {
                        warn!("Internal API status check failed: {:?}", err);
                        let api_status = InternalApiStatus {
                            last_update_time: Some(now),
                            last_error: Some(err.to_string()),
                            retry_count: current_retry_count.saturating_add(1),
                            is_fetching: false,
                        };
                        updater.set(api_status);
                    }
                }
            });
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
