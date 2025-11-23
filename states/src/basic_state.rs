use chrono::{DateTime, Utc};

use crate::{Reg, State};

#[derive(Default)]
pub struct Time {
    virt: DateTime<Utc>,
}

impl State for Time {
    const ID: Reg = Reg::Time;
}
