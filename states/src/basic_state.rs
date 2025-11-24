use chrono::{DateTime, Utc};

use crate::{Reg, State};

#[derive(Default)]
pub struct Time {
    virt: DateTime<Utc>,
}

impl State for Time {
    const ID: Reg = Reg::Time;
}

impl AsMut<DateTime<Utc>> for Time {
    fn as_mut(&mut self) -> &mut DateTime<Utc> {
        &mut self.virt
    }
}

impl AsRef<DateTime<Utc>> for Time {
    fn as_ref(&self) -> &DateTime<Utc> {
        &self.virt
    }
}
