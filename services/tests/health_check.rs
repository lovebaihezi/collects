use axum::http::StatusCode;
use axum_test::TestServer;
use collects_services::{config::Config, database::PersistentStructureDataService, routes};
use std::future::Future;

#[derive(Clone)]
struct NeonTestService {
    is_connected: bool,
}

impl PersistentStructureDataService for NeonTestService {
    fn is_connected(&self) -> impl Future<Output = bool> + Send {
        let connected = self.is_connected;
        async move { connected }
    }
}

#[tokio::test]
async fn test_health_check_integration() {
    // Case 1: Connected
    let storage_connected = NeonTestService { is_connected: true };
    let config = Config::new_for_test();
    let app_connected = routes(storage_connected, config).await;
    let server_connected = TestServer::new(app_connected).unwrap();

    let response = server_connected.get("/is-health").await;
    response.assert_status(StatusCode::OK);

    // Case 2: Disconnected
    let storage_disconnected = NeonTestService { is_connected: false };
    let config = Config::new_for_test(); // Create fresh config
    let app_disconnected = routes(storage_disconnected, config).await;
    let server_disconnected = TestServer::new(app_disconnected).unwrap();

    let response = server_disconnected.get("/is-health").await;
    response.assert_status(StatusCode::BAD_GATEWAY);
}
