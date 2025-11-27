use chrono::{DateTime, Utc};
use collects_states::{Compute, Reg, State};

#[derive(Default, Debug)]
pub struct ApiStatus {
    last_update_time: Option<DateTime<Utc>>,
    // if exists error, means api avaliable
    last_error: Option<String>,
}

pub enum APIAvailability {
    Available(DateTime<Utc>),
    Unavailable((DateTime<Utc>, String)),
    Unknown,
}

impl ApiStatus {
    pub fn api_availability(self) -> APIAvailability {
        match (self.last_update_time, self.last_error) {
            (None, None) => APIAvailability::Unknown,
            (Some(time), None) => APIAvailability::Available(time),
            (Some(time), Some(err)) => APIAvailability::Unavailable((time, err)),
            _ => APIAvailability::Unknown,
        }
    }
}

impl Compute for ApiStatus {
    fn compute(&self, ctx: &collects_states::StateCtx) {
        let request = ehttp::Request::get("https://collects.lqxclqxc./api/api-health");
        let api_status_updater = self.updater(ctx);
        ehttp::fetch(request, move |res| {
            let now = Utc::now();
            match res {
                Ok(response) => {
                    if response.status == 200 {
                        let api_status = ApiStatus {
                            last_update_time: Some(now),
                            last_error: None,
                        };
                        api_status_updater.set(api_status);
                    }
                }
                Err(err) => {
                    let api_status = ApiStatus {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                    };
                    api_status_updater.set(api_status);
                    log::error!("API status check failed: {}", err);
                }
            }
        });
    }
}

impl State for ApiStatus {
    fn id(&self) -> Reg {
        Reg::ApiStatus
    }
}
