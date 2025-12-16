mod api_status;
mod fetch_service;
mod fetch_state;

pub use api_status::{APIAvailability, ApiStatus};
pub use fetch_service::{EhttpFetcher, FetchService};
pub use fetch_state::FetchState;

#[cfg(feature = "test-utils")]
pub use fetch_service::MockFetcher;

#[cfg(test)]
mod tests;
