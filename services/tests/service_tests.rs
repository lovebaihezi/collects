use collects_services::core_services::{AuthService, AuthServiceImpl};
use collects_services::gateways::{MockCacheGateway, MockStorageGateway};
use std::sync::Arc;

#[tokio::test]
async fn test_auth_service_login_flow() {
    let mut mock_storage = MockStorageGateway::new();
    let mut mock_cache = MockCacheGateway::new();

    // Expect health check to be called
    mock_storage
        .expect_check_health()
        .times(1)
        .returning(|| Ok(()));

    // Expect cache set to be called
    mock_cache
        .expect_set()
        .with(
            mockall::predicate::string::starts_with("token_for_"),
            mockall::predicate::eq("user123"),
        )
        .times(1)
        .returning(|_, _| Ok(()));

    let service = AuthServiceImpl::new(Arc::new(mock_storage), Arc::new(mock_cache));

    let result: anyhow::Result<String> = service.login("user123").await;
    assert!(result.is_ok());
    let token = result.unwrap();
    assert!(token.starts_with("token_for_user123"));
}

#[tokio::test]
async fn test_auth_service_check_session() {
    let mock_storage = MockStorageGateway::new(); // Not used here
    let mut mock_cache = MockCacheGateway::new();

    // Case 1: Session exists
    mock_cache
        .expect_get()
        .with(mockall::predicate::eq("valid_token"))
        .returning(|_| Ok(Some("user123".to_string())));

    // Case 2: Session missing
    mock_cache
        .expect_get()
        .with(mockall::predicate::eq("invalid_token"))
        .returning(|_| Ok(None));

    let service = AuthServiceImpl::new(Arc::new(mock_storage), Arc::new(mock_cache));

    assert!(service.check_session("valid_token").await.unwrap());
    assert!(!service.check_session("invalid_token").await.unwrap());
}
