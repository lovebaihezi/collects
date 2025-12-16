use std::any::{Any, TypeId};
use std::marker::PhantomData;

use chrono::{DateTime, Utc};
use collects_states::{Compute, ComputeDeps, ComputeStage, Dep, State, Time, Updater, assign_impl};
use log::{error, info};

use crate::{FetchService, FetchState};

#[derive(Debug)]
pub struct ApiStatus<S> {
    last_update_time: Option<DateTime<Utc>>,
    // if exists error, means api available
    last_error: Option<String>,
    _phantom: PhantomData<S>,
}

impl<S> Default for ApiStatus<S> {
    fn default() -> Self {
        Self {
            last_update_time: None,
            last_error: None,
            _phantom: PhantomData,
        }
    }
}

pub enum APIAvailability<'a> {
    Available(DateTime<Utc>),
    Unavailable((DateTime<Utc>, &'a str)),
    Unknown,
}

impl<S> ApiStatus<S> {
    pub fn api_availability(&self) -> APIAvailability<'_> {
        match (self.last_update_time, &self.last_error) {
            (None, None) => APIAvailability::Unknown,
            (Some(time), None) => APIAvailability::Available(time),
            (Some(time), Some(err)) => APIAvailability::Unavailable((time, err.as_str())),
            _ => APIAvailability::Unknown,
        }
    }
}

impl<S: FetchService> Compute for ApiStatus<S> {
    fn deps(&self) -> ComputeDeps {
        // We cannot use const IDS for generic types in Rust (yet, easily),
        // or rather TypeId::of::<FetchState<S>>() is not const-stable if S is generic.
        // Wait, TypeId::of::<T>() IS const since 1.77 or so?
        // But let's check if we can do it.
        // If not, we might need to return a static slice or reference to something thread-local/lazy_static?
        // ComputeDeps signature is (&[TypeId], &[TypeId]). It expects references to slices.
        // The slices must live long enough. Usually they are static constants.
        // For generics, we might need a workaround.
        // However, `collects-states` might expect valid references.
        // Let's try to do it dynamically if needed, but the trait requires returning references.
        //
        // Workaround: Use a static generic struct to hold the IDS?
        // Or simply leak the vector? `Box::leak`
        //
        // Actually, `TypeId::of` is const stable.
        // But generic const items are tricky.
        //
        // Let's try:
        // struct DepIds<S>(PhantomData<S>);
        // impl<S: 'static> DepIds<S> {
        //    const IDS: [TypeId; 2] = [TypeId::of::<Time>(), TypeId::of::<FetchState<S>>()];
        // }
        // return (&DepIds::<S>::IDS, &[]);

        struct DepIds<S: ?Sized>(PhantomData<S>);
        impl<S: FetchService> DepIds<S> {
            const IDS: [TypeId; 2] = [TypeId::of::<Time>(), TypeId::of::<FetchState<S>>()];
        }
        (&DepIds::<S>::IDS, &[])
    }

    fn compute(&self, deps: Dep, updater: Updater) -> ComputeStage {
        let request = ehttp::Request::get("https://collects.lqxclqxc.com/api/is-health");
        let now = deps.get_state_ref::<Time>().as_ref().to_utc();
        let fetcher = &deps.get_state_ref::<FetchState<S>>().inner;

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
            fetcher.fetch(request, move |res| match res {
                Ok(response) => {
                    if response.status == 200 {
                        info!("BackEnd Available, checked at {:?}", now);
                        let api_status = ApiStatus::<S> {
                            last_update_time: Some(now),
                            last_error: None,
                            _phantom: PhantomData,
                        };
                        updater.set(api_status);
                    } else {
                        info!("BackEnd Return with status code: {:?}", response.status);
                    }
                }
                Err(err) => {
                    let api_status = ApiStatus::<S> {
                        last_update_time: Some(now),
                        last_error: Some(err.to_string()),
                        _phantom: PhantomData,
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

impl<S: FetchService> State for ApiStatus<S> {}
