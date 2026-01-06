use std::any::{Any, TypeId};

use crate::BusinessConfig;
use chrono::{DateTime, Utc};
use collects_states::{
    Command, Compute, ComputeDeps, Dep, State, Time, Updater, assign_impl, state_assign_impl,
};
use log::{debug, info, warn};
use ustr::Ustr;

/// HTTP header name for the service version
const SERVICE_VERSION_HEADER: &str = "x-service-version";

/// Maximum number of retry attempts on failure before waiting for the full interval
const MAX_RETRY_COUNT: u8 = 3;

#[derive(Default, Debug)]
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
            (None, None) => APIAvailability::Unknown,
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
}

impl Compute for ApiStatus {
    fn deps(&self) -> ComputeDeps {
        const IDS: [TypeId; 2] = [TypeId::of::<Time>(), TypeId::of::<BusinessConfig>()];
        (&IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) {
        // Skip if a fetch is already in-flight to prevent duplicate requests
        if self.is_fetching {
            debug!("API status fetch already in-flight, skipping");
            return;
        }

        let config = deps.get_state_ref::<BusinessConfig>();
        let url = Ustr::from(format!("{}/is-health", config.api_url().as_str()).as_str());
        let request = ehttp::Request::get(url);
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();
        let current_retry_count = self.retry_count;
        let current_show_status = self.show_status;

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
                        "API status last updated at {:?}, now is {:?}, should fetch new status",
                        last_update_time, now
                    );
                } else if should_retry {
                    info!(
                        "API status check failed, retry attempt {}/{}",
                        current_retry_count + 1,
                        MAX_RETRY_COUNT
                    );
                }

                interval_passed || should_retry
            }
            None => {
                info!("Not fetch API yet, should fetch new status");
                true
            }
        };
        if should_fetch {
            info!(
                "Fetching API Status at {:?} on: {:?}, Waiting Result",
                &url, now
            );
            // Mark as fetching to prevent duplicate requests while this one is in-flight.
            // We use updater.set() to update the state immediately, which will be synced
            // on the next sync_computes() call.
            updater.set(ApiStatus {
                last_update_time: self.last_update_time,
                last_error: self.last_error.clone(),
                service_version: self.service_version.clone(),
                retry_count: current_retry_count,
                show_status: current_show_status,
                is_fetching: true,
            });
            ehttp::fetch(request, move |res| match res {
                Ok(response) => {
                    let service_version = response
                        .headers
                        .get(SERVICE_VERSION_HEADER)
                        .map(String::from);
                    if response.status == 200 {
                        debug!("BackEnd Available, checked at {:?}", now);
                        let api_status = ApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                            service_version,
                            retry_count: 0, // Reset retry count on success
                            show_status: current_show_status,
                            is_fetching: false,
                        };
                        updater.set(api_status);
                    } else {
                        info!("BackEnd Return with status code: {:?}", response.status);
                        let api_status = ApiStatus {
                            last_update_time: Some(now),
                            last_error: Some(format!("API Health: {}", response.status)),
                            service_version,
                            retry_count: current_retry_count.saturating_add(1),
                            show_status: current_show_status,
                            is_fetching: false,
                        };
                        updater.set(api_status);
                    }
                }
                Err(err) => {
                    warn!("API status check failed: {:?}", err);
                    let api_status = ApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                        service_version: None,
                        retry_count: current_retry_count.saturating_add(1),
                        show_status: current_show_status,
                        is_fetching: false,
                    };
                    updater.set(api_status);
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

impl State for ApiStatus {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Command to toggle the API status panel visibility.
///
/// Dispatch explicitly via `ctx.dispatch::<ToggleApiStatusCommand>()`.
#[derive(Default, Debug)]
pub struct ToggleApiStatusCommand;

impl Command for ToggleApiStatusCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let current = deps.get_compute_ref::<ApiStatus>();
        let new_show_status = !current.show_status;

        updater.set(ApiStatus {
            last_update_time: current.last_update_time,
            last_error: current.last_error.clone(),
            service_version: current.service_version.clone(),
            retry_count: current.retry_count,
            show_status: new_show_status,
            is_fetching: current.is_fetching,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that ApiStatus defaults with is_fetching = false
    #[test]
    fn test_api_status_default_is_fetching_false() {
        let status = ApiStatus::default();
        assert!(!status.is_fetching, "is_fetching should default to false");
    }

    /// Tests that is_fetching flag can be set to true
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

    /// Tests that api_availability returns Unknown when is_fetching is true but no data
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

    /// Tests that show_status returns correct value
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
}
