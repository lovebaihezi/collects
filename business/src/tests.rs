#[cfg(test)]
mod tests {
    use crate::{APIAvailability, ApiStatus, FetchState, MockFetcher};
    use chrono::{TimeZone, Utc};
    use collects_states::{StateCtx, Time};
    use ehttp::{Request, Response};

    #[test]
    fn test_api_status_mock_fetch() {
        let mut ctx = StateCtx::new();
        let _ = env_logger::builder().is_test(true).try_init();

        // Setup mock fetcher
        let mut mock_fetcher = MockFetcher::default();
        mock_fetcher.response = Some(Ok(Response {
            url: "https://collects.lqxclqxc.com/api/is-health".to_string(),
            status: 200,
            status_text: "OK".to_string(),
            headers: Default::default(),
            bytes: vec![],
            ok: true,
        }));

        ctx.add_state(FetchState {
            inner: mock_fetcher,
        });

        // Setup time
        ctx.add_state(Time::default());

        // Register ApiStatus compute with MockFetcher
        ctx.record_compute(ApiStatus::<MockFetcher>::default());

        // Run compute cycle
        ctx.run_computed();

        // At this point, compute logic ran, but since `fetch` takes a callback and `MockFetcher` calls it synchronously (in my implementation),
        // the `updater.set` inside the callback should have been called.
        // However, `updater.set` sends a message to a channel. We need to process that message.
        // `StateCtx::run_computed` runs `graph.calculate_computes`, which executes `compute` method.
        // `ctx.sync_computes()` processes the updates.

        ctx.sync_computes();

        // Check result
        if let Some(status) = ctx.cached::<ApiStatus<MockFetcher>>() {
            match status.api_availability() {
                APIAvailability::Available(time) => {
                    println!("API Available at {:?}", time);
                    // Success!
                }
                APIAvailability::Unavailable((time, err)) => {
                    panic!("API Unavailable: {} at {:?}", err, time);
                }
                APIAvailability::Unknown => {
                    panic!("API Status Unknown");
                }
            }
        } else {
            panic!("ApiStatus not found in context");
        }
    }

    #[test]
    fn test_api_status_mock_fetch_error() {
        let mut ctx = StateCtx::new();

        // Setup mock fetcher with error
        let mut mock_fetcher = MockFetcher::default();
        mock_fetcher.response = Some(Err("Network Error".to_string()));

        ctx.add_state(FetchState {
            inner: mock_fetcher,
        });

        ctx.add_state(Time::default());
        ctx.record_compute(ApiStatus::<MockFetcher>::default());

        ctx.run_computed();
        ctx.sync_computes();

        if let Some(status) = ctx.cached::<ApiStatus<MockFetcher>>() {
            match status.api_availability() {
                APIAvailability::Unavailable((_, err)) => {
                    assert_eq!(err, "Network Error");
                }
                _ => panic!("Expected API Unavailable"),
            }
        } else {
            panic!("ApiStatus not found in context");
        }
    }
}
