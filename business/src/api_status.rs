use std::any::{Any, TypeId};

use chrono::{DateTime, Utc};
use collects_states::{Compute, ComputeDeps, ComputeStage, Dep, State, Time, Updater, assign_impl};
use log::{error, info};

#[derive(Default, Debug)]
pub struct ApiStatus {
    last_update_time: Option<DateTime<Utc>>,
    // if exists error, means api available
    last_error: Option<String>,
}

pub enum APIAvailability<'a> {
    Available(DateTime<Utc>),
    Unavailable((DateTime<Utc>, &'a str)),
    Unknown,
}

impl ApiStatus {
    pub fn api_availability(&self) -> APIAvailability<'_> {
        match (self.last_update_time, &self.last_error) {
            (None, None) => APIAvailability::Unknown,
            (Some(time), None) => APIAvailability::Available(time),
            (Some(time), Some(err)) => APIAvailability::Unavailable((time, err.as_str())),
            _ => APIAvailability::Unknown,
        }
    }
}

impl Compute for ApiStatus {
    fn deps(&self) -> ComputeDeps {
        const IDS: [TypeId; 1] = [TypeId::of::<Time>()];
        (&IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) -> ComputeStage {
        let request = ehttp::Request::get("https://collects.lqxclqxc.com/api/is-health");
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
            info!("Get API Status at {:?}", now);
            ehttp::fetch(request, move |res| match res {
                Ok(response) => {
                    if response.status == 200 {
                        info!("BackEnd Available, checked at {:?}", now);
                        let api_status = ApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                        };
                        updater.set(api_status);
                    } else {
                        info!("BackEnd Return with status code: {:?}", response.status);
                    }
                }
                Err(err) => {
                    let api_status = ApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                    };
                    updater.set(api_status);
                    error!("API status check failed: {:?}", err);
                }
            });
            ComputeStage::Pending
        } else {
            ComputeStage::Finished
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for ApiStatus {}
