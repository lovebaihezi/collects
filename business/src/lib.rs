mod api_status;

use std::any::TypeId;

pub use api_status::{APIAvailability, ApiStatus};

pub const COMPUTES: &[TypeId] = &[TypeId::of::<ApiStatus>()];
