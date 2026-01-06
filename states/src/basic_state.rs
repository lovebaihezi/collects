use std::any::Any;

use chrono::{DateTime, Utc};

use crate::{State, state_assign_impl};

#[derive(Debug)]
pub struct Time {
    virt: DateTime<Utc>,
}

impl Default for Time {
    fn default() -> Self {
        Self { virt: Utc::now() }
    }
}

impl State for Time {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
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
