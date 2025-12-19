use collects_services::gateways::opendal::OpenDalGateway;
use collects_services::gateways::postgres::PostgresGateway;
use collects_services::gateways::redis::RedisGateway;
use collects_services::gateways::{CacheGateway, FileGateway, StorageGateway};
use opendal::{Operator, services::Memory};
use sqlx::postgres::PgPoolOptions;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::redis::Redis;

#[tokio::test]
async fn test_postgres_gateway_real_integration() {
    let node = Postgres::default().start().await.unwrap();
    let connection_string = &format!(
        "postgres://postgres:postgres@127.0.0.1:{}/postgres",
        node.get_host_port_ipv4(5432).await.unwrap()
    );

    let pool = PgPoolOptions::new()
        .connect(connection_string)
        .await
        .unwrap();

    let gateway = PostgresGateway::new(pool);
    assert!(gateway.check_health().await.is_ok());
}

#[tokio::test]
async fn test_redis_gateway_real_integration() {
    let node = Redis::default().start().await.unwrap();
    let connection_string = &format!(
        "redis://127.0.0.1:{}/",
        node.get_host_port_ipv4(6379).await.unwrap()
    );

    let gateway = RedisGateway::new(connection_string).unwrap();

    // Test Health
    assert!(gateway.check_health().await.is_ok());

    // Test Set/Get
    gateway.set("foo", "bar").await.unwrap();
    let val = gateway.get("foo").await.unwrap();
    assert_eq!(val, Some("bar".to_string()));

    let missing = gateway.get("missing").await.unwrap();
    assert_eq!(missing, None);
}

#[tokio::test]
async fn test_opendal_gateway_real_integration() {
    // For OpenDAL, we can use the Memory service for testing as it implements the same Operator interface
    // This mocks the backend but tests the Gateway logic and OpenDAL integration.
    let op = Operator::new(Memory::default()).unwrap().finish();
    let gateway = OpenDalGateway::new(op);

    // Test Health
    assert!(gateway.check_health().await.is_ok());

    // Test Write/Read
    let data = b"hello world".to_vec();
    gateway.write("test_file", data.clone()).await.unwrap();

    let read_data = gateway.read("test_file").await.unwrap();
    assert_eq!(read_data, data);
}
