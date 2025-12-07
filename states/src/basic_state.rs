use chrono::{DateTime, Utc};

use crate::State;

#[derive(Debug, Default)]
pub struct Time {
    virt: DateTime<Utc>,
}

impl State for Time {}

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
