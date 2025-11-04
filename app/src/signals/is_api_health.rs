use chrono::{DateTime, Utc};
use egui::Id;

use crate::utils::signals::Signal;

pub struct HealStatus {
    last_fetch_time: DateTime<Utc>,
    is_healthy: bool,
}

pub struct APIStatusSignal;

impl Signal for APIStatusSignal {
    type Output = bool;

    fn id(&self) -> Id {
        Id::new(super::Signals::APIStatus)
    }
}
