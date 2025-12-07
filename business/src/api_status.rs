use std::any::{Any, TypeId};

use chrono::{DateTime, Utc};
use collects_states::{Compute, ComputeDeps, Dep, State, Time, Updater, assign_impl};

#[derive(Default, Debug)]
pub struct ApiStatus {
    last_update_time: Option<DateTime<Utc>>,
    // if exists error, means api avaliable
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
    fn compute(&self, deps: Dep, updater: Updater) {
        let request = ehttp::Request::get("https://collects.lqxclqxc./api/api-health");
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();
        ehttp::fetch(request, move |res| match res {
            Ok(response) => {
                if response.status == 200 {
                    let api_status = ApiStatus {
                        last_update_time: Some(now),
                        last_error: None,
                    };
                    updater.set(api_status);
                }
            }
            Err(err) => {
                let api_status = ApiStatus {
                    last_update_time: Some(now),
                    last_error: Some(err.to_string()),
                };
                updater.set(api_status);
                log::error!("API status check failed: {}", err);
            }
        });
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_boxed_any(self) -> Box<dyn Any> {
        Box::new(self)
    }
    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for ApiStatus {}
