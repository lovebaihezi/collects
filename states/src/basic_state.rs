use chrono::{DateTime, Utc};

use crate::State;

#[derive(Debug)]
pub struct Time {
    virt: DateTime<Utc>,
}

impl Default for Time {
    fn default() -> Self {
        Self { virt: Utc::now() }
    }
}

use std::any::Any;

impl State for Time {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
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
