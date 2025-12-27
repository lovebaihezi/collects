use std::any::{Any, TypeId};

use crate::BusinessConfig;
use chrono::{DateTime, Utc};
use collects_states::{Compute, ComputeDeps, Dep, State, Time, Updater, assign_impl};
use log::{debug, info, warn};
use ustr::Ustr;

#[derive(Default, Debug)]
pub struct ApiStatus {
    last_update_time: Option<DateTime<Utc>>,
    // if exists error, means api unavailable
    last_error: Option<String>,
    // HTTP status code for non-200 responses
    status_code: Option<u16>,
}

pub enum APIAvailability<'a> {
    Available(DateTime<Utc>),
    Unavailable((DateTime<Utc>, &'a str)),
    UnhealthyStatus((DateTime<Utc>, u16)),
    Unknown,
}

impl ApiStatus {
    pub fn api_availability(&self) -> APIAvailability<'_> {
        match (self.last_update_time, &self.last_error, self.status_code) {
            (None, None, None) => APIAvailability::Unknown,
            (Some(time), None, None) => APIAvailability::Available(time),
            (Some(time), None, Some(status)) => APIAvailability::UnhealthyStatus((time, status)),
            (Some(time), Some(err), _) => APIAvailability::Unavailable((time, err.as_str())),
            _ => APIAvailability::Unknown,
        }
    }
}

impl Compute for ApiStatus {
    fn deps(&self) -> ComputeDeps {
        const IDS: [TypeId; 2] = [TypeId::of::<Time>(), TypeId::of::<BusinessConfig>()];
        (&IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) {
        let config = deps.get_state_ref::<BusinessConfig>();
        let url = Ustr::from(format!("{}/is-health", config.api_url().as_str()).as_str());
        let request = ehttp::Request::get(url);
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();
        let should_fetch = match &self.last_update_time {
            Some(last_update_time) => {
                let duration_since_update = now.signed_duration_since(*last_update_time);
                let should = duration_since_update.num_minutes() >= 5;
                if should {
                    info!(
                        "API status last updated at {:?}, now is {:?}, should fetch new status",
                        last_update_time, now
                    );
                }
                should
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
            ehttp::fetch(request, move |res| match res {
                Ok(response) => {
                    if response.status == 200 {
                        debug!("BackEnd Available, checked at {:?}", now);
                        let api_status = ApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                            status_code: None,
                        };
                        updater.set(api_status);
                    } else {
                        info!("BackEnd Return with status code: {:?}", response.status);
                        let api_status = ApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                            status_code: Some(response.status),
                        };
                        updater.set(api_status);
                    }
                }
                Err(err) => {
                    warn!("API status check failed: {:?}", err);
                    let api_status = ApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                        status_code: None,
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

impl State for ApiStatus {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
