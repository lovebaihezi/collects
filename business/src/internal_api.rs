//! Internal API module for internal/test-internal environments.
//!
//! This module provides functionality for internal environments only,
//! including fetching users and their OTP codes.

use std::any::{Any, TypeId};

use crate::BusinessConfig;
use chrono::{DateTime, Utc};
use collects_states::{Compute, ComputeDeps, Dep, State, Time, Updater, assign_impl};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

/// Represents a user with their current OTP code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalUser {
    pub username: String,
    pub current_otp: String,
}

/// Response from the internal users endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListUsersResponse {
    pub users: Vec<InternalUser>,
}

/// State for internal API status (checking if internal API is accessible).
#[derive(Default, Debug)]
pub struct InternalApiStatus {
    pub last_update_time: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// Availability status for internal API.
pub enum InternalAPIAvailability<'a> {
    Available(DateTime<Utc>),
    Unavailable((DateTime<Utc>, &'a str)),
    Unknown,
}

impl InternalApiStatus {
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
        let config = deps.get_state_ref::<BusinessConfig>();
        // Check internal API by calling /internal/users (GET)
        let url = Ustr::from(format!("{}/internal/users", config.api_url().as_str()).as_str());
        let request = ehttp::Request::get(url);
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();
        let should_fetch = match &self.last_update_time {
            Some(last_update_time) => {
                let duration_since_update = now.signed_duration_since(*last_update_time);
                // Check every 5 minutes
                let should = duration_since_update.num_minutes() >= 5;
                if should {
                    info!(
                        "Internal API status last updated at {:?}, now is {:?}, should fetch",
                        last_update_time, now
                    );
                }
                should
            }
            None => {
                info!("Not fetch Internal API yet, should fetch status");
                true
            }
        };
        if should_fetch {
            info!(
                "Fetching Internal API Status at {:?} on: {:?}, Waiting Result",
                &url, now
            );
            ehttp::fetch(request, move |res| match res {
                Ok(response) => {
                    if response.status == 200 {
                        debug!("Internal API Available, checked at {:?}", now);
                        let api_status = InternalApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                        };
                        updater.set(api_status);
                    } else {
                        info!(
                            "Internal API Return with status code: {:?}",
                            response.status
                        );
                        let api_status = InternalApiStatus {
                            last_update_time: Some(now),
                            last_error: Some(format!("Status code: {}", response.status)),
                        };
                        updater.set(api_status);
                    }
                }
                Err(err) => {
                    warn!("Internal API status check failed: {:?}", err);
                    let api_status = InternalApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                    };
                    updater.set(api_status);
                }
            });
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for InternalApiStatus {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// State for internal users list.
#[derive(Default, Debug)]
pub struct InternalUsers {
    pub last_update_time: Option<DateTime<Utc>>,
    pub users: Vec<InternalUser>,
    pub last_error: Option<String>,
    pub is_fetching: bool,
}

impl InternalUsers {
    /// Returns true if the users list has been successfully fetched.
    pub fn is_loaded(&self) -> bool {
        self.last_update_time.is_some() && self.last_error.is_none()
    }

    /// Triggers a refresh of the users list.
    pub fn refresh(&mut self) {
        self.last_update_time = None;
    }
}

impl Compute for InternalUsers {
    fn deps(&self) -> ComputeDeps {
        const IDS: [TypeId; 2] = [TypeId::of::<Time>(), TypeId::of::<BusinessConfig>()];
        (&IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) {
        // Don't refetch if already fetching
        if self.is_fetching {
            return;
        }

        let config = deps.get_state_ref::<BusinessConfig>();
        let url = Ustr::from(format!("{}/internal/users", config.api_url().as_str()).as_str());
        let request = ehttp::Request::get(url);
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();

        let should_fetch = match &self.last_update_time {
            Some(last_update_time) => {
                let duration_since_update = now.signed_duration_since(*last_update_time);
                // Refresh every 30 seconds for OTP codes
                duration_since_update.num_seconds() >= 30
            }
            None => true,
        };

        if should_fetch {
            info!("Fetching Internal Users at {:?} on: {:?}", &url, now);

            // Store current state to use in callback
            let current_users = self.users.clone();

            ehttp::fetch(request, move |res| match res {
                Ok(response) => {
                    if response.status == 200 {
                        match serde_json::from_slice::<ListUsersResponse>(&response.bytes) {
                            Ok(data) => {
                                debug!("Internal Users loaded: {} users", data.users.len());
                                let state = InternalUsers {
                                    last_update_time: Some(now),
                                    users: data.users,
                                    last_error: None,
                                    is_fetching: false,
                                };
                                updater.set(state);
                            }
                            Err(err) => {
                                warn!("Failed to parse internal users response: {:?}", err);
                                let state = InternalUsers {
                                    last_update_time: Some(now),
                                    users: current_users.clone(),
                                    last_error: Some(err.to_string()),
                                    is_fetching: false,
                                };
                                updater.set(state);
                            }
                        }
                    } else {
                        info!("Internal Users API returned status: {:?}", response.status);
                        let state = InternalUsers {
                            last_update_time: Some(now),
                            users: current_users.clone(),
                            last_error: Some(format!("Status code: {}", response.status)),
                            is_fetching: false,
                        };
                        updater.set(state);
                    }
                }
                Err(err) => {
                    warn!("Internal Users fetch failed: {:?}", err);
                    let state = InternalUsers {
                        last_update_time: Some(now),
                        users: current_users,
                        last_error: Some(err.to_string()),
                        is_fetching: false,
                    };
                    updater.set(state);
                }
            });
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl Clone for InternalUsers {
    fn clone(&self) -> Self {
        Self {
            last_update_time: self.last_update_time,
            users: self.users.clone(),
            last_error: self.last_error.clone(),
            is_fetching: self.is_fetching,
        }
    }
}

impl State for InternalUsers {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Request to create a new user.
#[derive(Debug, Serialize)]
pub struct CreateUserRequest {
    pub username: String,
}

/// Response after creating a user with OTP.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateUserResponse {
    pub username: String,
    pub secret: String,
    pub otpauth_url: String,
}

/// Creates a new user via the internal API.
pub fn create_user(
    api_base_url: &str,
    username: &str,
    callback: impl FnOnce(Result<CreateUserResponse, String>) + Send + 'static,
) {
    let url = format!("{}/internal/users", api_base_url);
    let body = serde_json::to_vec(&CreateUserRequest {
        username: username.to_string(),
    })
    .expect("Failed to serialize request");

    let mut request = ehttp::Request::post(url, body);
    request.headers.insert(
        "Content-Type".to_string(),
        "application/json".to_string(),
    );

    ehttp::fetch(request, move |res| match res {
        Ok(response) => {
            if response.status == 201 {
                match serde_json::from_slice::<CreateUserResponse>(&response.bytes) {
                    Ok(data) => callback(Ok(data)),
                    Err(err) => callback(Err(format!("Failed to parse response: {}", err))),
                }
            } else {
                let error_msg = String::from_utf8_lossy(&response.bytes).to_string();
                callback(Err(format!(
                    "Failed to create user (status {}): {}",
                    response.status, error_msg
                )))
            }
        }
        Err(err) => callback(Err(format!("Request failed: {}", err))),
    });
}
