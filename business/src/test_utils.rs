//! Test utilities for business layer testing with mock servers.
//!
//! This module provides helpers to set up mock HTTP servers and test the
//! business commands (CreateContent, ListContents, GetContent, GetViewUrl, etc.)
//! without hitting real API endpoints.
//!
//! # Example
//!
//! ```ignore
//! use collects_business::test_utils::{TestContext, sample_content_item};
//!
//! #[tokio::test]
//! async fn test_list_contents() {
//!     let mut test_ctx = TestContext::new().await;
//!
//!     // Mount a mock response for the list contents endpoint
//!     test_ctx.mock_list_contents(vec![sample_content_item("1")], 1).await;
//!
//!     // Set up auth (most endpoints require authentication)
//!     test_ctx.set_authenticated("test_token");
//!
//!     // Execute the command
//!     test_ctx.ctx.enqueue_command::<ListContentsCommand>();
//!     test_ctx.flush_and_wait().await;
//!
//!     // Verify results
//!     let compute = test_ctx.ctx.compute::<ListContentsCompute>();
//!     // ... assert on compute.status
//! }
//! ```

#![cfg(all(test, not(target_arch = "wasm32")))]

use std::time::Duration;

use ustr::Ustr;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use crate::{
    AddGroupContentsCommand, AddGroupContentsCompute, AddGroupContentsInput, AuthCompute,
    BusinessConfig, CFTokenCompute, CreateContentCommand, CreateContentCompute, CreateContentInput,
    CreateGroupCommand, CreateGroupCompute, CreateGroupInput, GetContentCommand, GetContentCompute,
    GetContentInput, GetGroupContentsCommand, GetGroupContentsCompute, GetGroupContentsInput,
    GetViewUrlCommand, GetViewUrlCompute, GetViewUrlInput, GroupContentItem, GroupItem,
    ListContentsCommand, ListContentsCompute, ListContentsInput, ListGroupsCommand,
    ListGroupsCompute, ListGroupsInput, LoginCommand, LoginInput, PendingTokenValidation,
    ValidateTokenCommand, list_content::ContentItem,
};
use collects_states::StateCtx;

/// Test context that holds a mock server and a configured StateCtx.
pub struct TestContext {
    /// The mock server instance.
    pub mock_server: MockServer,
    /// The state context configured to use the mock server.
    pub ctx: StateCtx,
}

impl TestContext {
    /// Create a new test context with a fresh mock server.
    pub async fn new() -> Self {
        let mock_server = MockServer::start().await;
        let base_url = mock_server.uri();

        let config = BusinessConfig::new(base_url);
        let ctx = build_test_state_ctx(config);

        Self { mock_server, ctx }
    }

    /// Set the authentication state to authenticated with the given token.
    pub fn set_authenticated(&mut self, token: &str) {
        // Update AuthCompute via the Updater (since it's a Compute, not a State)
        let updater = self.ctx.updater();
        updater.set(AuthCompute::new_authenticated(
            token.to_owned(),
            "test_user".to_owned(),
        ));
        // Sync to apply the update
        self.ctx.sync_computes();
    }

    /// Set a CF Access token for internal endpoints.
    pub fn set_cf_token(&mut self, token: &str) {
        // Update CFTokenCompute via the Updater (since it's a Compute, not a State)
        let mut cf = CFTokenCompute::default();
        cf.set_token(Some(token.to_owned()));
        let updater = self.ctx.updater();
        updater.set(cf);
        // Sync to apply the update
        self.ctx.sync_computes();
    }

    /// Flush all pending commands and wait for async tasks to complete.
    ///
    /// This mirrors the CLI's `flush_and_await` pattern:
    /// 1. Sync any pending compute updates
    /// 2. Flush command queue (spawns async tasks)
    /// 3. Await all tasks in the JoinSet, syncing after each completes
    /// 4. Final sync to ensure all updates are applied
    pub async fn flush_and_wait(&mut self) {
        // Initial sync
        self.ctx.sync_computes();

        // Flush commands (this spawns async tasks)
        self.ctx.flush_commands();

        // Await all tasks, syncing after each completes
        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();

        while self.ctx.task_count() > 0 {
            if start.elapsed() > timeout {
                panic!(
                    "Timed out waiting for pending tasks ({} still in JoinSet)",
                    self.ctx.task_count()
                );
            }

            // Wait for the next task to complete
            if self.ctx.task_set_mut().join_next().await.is_some() {
                // Sync compute updates from the completed task
                self.ctx.sync_computes();
            }
        }

        // Final sync to ensure all updates are applied
        self.ctx.sync_computes();
    }

    /// Shutdown the context (cancel all tasks).
    pub async fn shutdown(&mut self) {
        self.ctx.shutdown().await;
    }

    // =========================================================================
    // Mock endpoint helpers
    // =========================================================================

    /// Mock the login endpoint.
    pub async fn mock_login(&self, success: bool, token: Option<&str>, username: Option<&str>) {
        let response = if success {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "token": token.unwrap_or("mock_token"),
                "username": username.unwrap_or("mock_user")
            }))
        } else {
            ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": "Invalid credentials"
            }))
        };

        Mock::given(method("POST"))
            .and(path("/api/v1/auth/login"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the token validation endpoint.
    pub async fn mock_validate_token(&self, valid: bool, username: Option<&str>) {
        let response = if valid {
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "valid": true,
                "username": username.unwrap_or("mock_user")
            }))
        } else {
            ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "valid": false,
                "error": "Token expired"
            }))
        };

        Mock::given(method("POST"))
            .and(path("/api/v1/auth/token/validate"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the list contents endpoint.
    pub async fn mock_list_contents(&self, items: Vec<ContentItem>, total: usize) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": items,
            "total": total
        }));

        Mock::given(method("GET"))
            .and(path("/api/v1/contents"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the list contents endpoint with an error.
    pub async fn mock_list_contents_error(&self, status: u16, error: &str) {
        let response = ResponseTemplate::new(status).set_body_json(serde_json::json!({
            "error": error
        }));

        Mock::given(method("GET"))
            .and(path("/api/v1/contents"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the get content endpoint.
    pub async fn mock_get_content(&self, id: &str, content: ContentItem) {
        // GetContentCommand expects a flat ContentItem response, not wrapped in {"content": ...}
        let response = ResponseTemplate::new(200).set_body_json(content);

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/contents/{}", id)))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the get content endpoint with 404.
    pub async fn mock_get_content_not_found(&self, id: &str) {
        let response = ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": "Content not found"
        }));

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/contents/{}", id)))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the view URL endpoint.
    pub async fn mock_view_url(&self, content_id: &str, url: &str, expires_at: &str) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "url": url,
            "expires_at": expires_at
        }));

        Mock::given(method("POST"))
            .and(path(format!("/api/v1/contents/{}/view-url", content_id)))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the view URL endpoint with error.
    pub async fn mock_view_url_error(&self, content_id: &str, status: u16, error: &str) {
        let response = ResponseTemplate::new(status).set_body_json(serde_json::json!({
            "error": error
        }));

        Mock::given(method("POST"))
            .and(path(format!("/api/v1/contents/{}/view-url", content_id)))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the create inline content endpoint.
    pub async fn mock_create_content(&self, content_id: &str) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": {
                "id": content_id
            }
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/contents"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the upload init endpoint.
    pub async fn mock_upload_init(&self, upload_id: &str) {
        // The upload URL points to our mock server for the PUT request
        let upload_url = format!("{}/mock-upload/{}", self.mock_server.uri(), upload_id);

        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "upload_id": upload_id,
            "upload_url": upload_url
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/uploads/init"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the upload PUT endpoint (R2 presigned URL simulation).
    pub async fn mock_upload_put(&self, upload_id: &str) {
        let response = ResponseTemplate::new(200);

        Mock::given(method("PUT"))
            .and(path(format!("/mock-upload/{}", upload_id)))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the upload complete endpoint.
    pub async fn mock_upload_complete(&self, content_id: &str) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": {
                "id": content_id
            }
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/uploads/complete"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the full upload flow (init + put + complete).
    pub async fn mock_full_upload(&self, upload_id: &str, content_id: &str) {
        self.mock_upload_init(upload_id).await;
        self.mock_upload_put(upload_id).await;
        self.mock_upload_complete(content_id).await;
    }

    // =========================================================================
    // Group (Collect) mock helpers
    // =========================================================================

    /// Mock the list groups endpoint.
    pub async fn mock_list_groups(&self, items: Vec<GroupItem>, total: usize) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": items,
            "total": total
        }));

        Mock::given(method("GET"))
            .and(path("/api/v1/groups"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the create group endpoint.
    pub async fn mock_create_group(&self, group: GroupItem) {
        let response = ResponseTemplate::new(200).set_body_json(&group);

        Mock::given(method("POST"))
            .and(path("/api/v1/groups"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the create group endpoint with an error.
    pub async fn mock_create_group_error(&self, status: u16, error: &str) {
        let response = ResponseTemplate::new(status).set_body_json(serde_json::json!({
            "error": error
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/groups"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the get group contents endpoint.
    pub async fn mock_get_group_contents(&self, group_id: &str, items: Vec<GroupContentItem>) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "items": items,
            "total": items.len()
        }));

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/groups/{}/contents", group_id)))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the get group contents endpoint with 404.
    pub async fn mock_get_group_contents_not_found(&self, group_id: &str) {
        let response = ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": "Group not found"
        }));

        Mock::given(method("GET"))
            .and(path(format!("/api/v1/groups/{}/contents", group_id)))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the add content to group endpoint.
    pub async fn mock_add_group_content(&self, group_id: &str) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true
        }));

        Mock::given(method("POST"))
            .and(path(format!("/api/v1/groups/{}/contents", group_id)))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the add content to group endpoint with error.
    pub async fn mock_add_group_content_error(&self, group_id: &str, status: u16, error: &str) {
        let response = ResponseTemplate::new(status).set_body_json(serde_json::json!({
            "error": error
        }));

        Mock::given(method("POST"))
            .and(path(format!("/api/v1/groups/{}/contents", group_id)))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }
}

/// Build a StateCtx configured for testing with all necessary states and commands.
fn build_test_state_ctx(config: BusinessConfig) -> StateCtx {
    let mut ctx = StateCtx::new();

    // Business config
    ctx.add_state(config);

    // Login states and computes
    ctx.add_state(LoginInput::default());
    ctx.add_state(PendingTokenValidation::default());
    ctx.record_compute(CFTokenCompute::default());
    ctx.record_compute(AuthCompute::default());

    // Content creation states and computes
    ctx.add_state(CreateContentInput::default());
    ctx.record_compute(CreateContentCompute::default());

    // List contents states and computes
    ctx.add_state(ListContentsInput::default());
    ctx.record_compute(ListContentsCompute::default());

    // Get content states and computes
    ctx.add_state(GetContentInput::default());
    ctx.record_compute(GetContentCompute::default());

    // Get view URL states and computes
    ctx.add_state(GetViewUrlInput::default());
    ctx.record_compute(GetViewUrlCompute::default());

    // Group (collect) states and computes
    ctx.add_state(ListGroupsInput::default());
    ctx.record_compute(ListGroupsCompute::default());
    ctx.add_state(CreateGroupInput::default());
    ctx.record_compute(CreateGroupCompute::default());
    ctx.add_state(AddGroupContentsInput::default());
    ctx.record_compute(AddGroupContentsCompute::default());
    ctx.add_state(GetGroupContentsInput::default());
    ctx.record_compute(GetGroupContentsCompute::default());

    // Commands
    ctx.record_command(LoginCommand);
    ctx.record_command(ValidateTokenCommand);
    ctx.record_command(CreateContentCommand);
    ctx.record_command(ListContentsCommand);
    ctx.record_command(GetContentCommand);
    ctx.record_command(GetViewUrlCommand);
    ctx.record_command(ListGroupsCommand);
    ctx.record_command(CreateGroupCommand);
    ctx.record_command(AddGroupContentsCommand);
    ctx.record_command(GetGroupContentsCommand);

    ctx
}

/// Helper to create a sample ContentItem for testing (file type).
pub fn sample_content_item(id: &str) -> ContentItem {
    ContentItem {
        id: Ustr::from(id),
        title: Ustr::from(&format!("Test Content {}", id)),
        description: None,
        storage_backend: Ustr::from("r2"),
        storage_profile: Ustr::from("default"),
        storage_key: Ustr::from(&format!("files/{}.txt", id)),
        content_type: Ustr::from("text/plain"),
        file_size: 1024,
        status: Ustr::from("active"),
        visibility: Ustr::from("private"),
        kind: Ustr::from("file"),
        body: None,
        trashed_at: None,
        archived_at: None,
        created_at: Ustr::from("2024-01-01T00:00:00Z"),
        updated_at: Ustr::from("2024-01-01T00:00:00Z"),
    }
}

/// Helper to create a text ContentItem for testing.
pub fn sample_text_content(id: &str, body: &str) -> ContentItem {
    ContentItem {
        id: Ustr::from(id),
        title: Ustr::from(&format!("Note {}", id)),
        description: None,
        storage_backend: Ustr::from(""),
        storage_profile: Ustr::from(""),
        storage_key: Ustr::from(""),
        content_type: Ustr::from("text/plain"),
        file_size: body.len() as i64,
        status: Ustr::from("active"),
        visibility: Ustr::from("private"),
        kind: Ustr::from("text"),
        body: Some(body.to_owned()),
        trashed_at: None,
        archived_at: None,
        created_at: Ustr::from("2024-01-01T00:00:00Z"),
        updated_at: Ustr::from("2024-01-01T00:00:00Z"),
    }
}

/// Helper to create a sample GroupItem for testing.
pub fn sample_group_item(id: &str, name: &str) -> GroupItem {
    GroupItem {
        id: Ustr::from(id),
        name: Ustr::from(name),
        description: None,
        visibility: Ustr::from("private"),
        status: Ustr::from("active"),
        trashed_at: None,
        archived_at: None,
        created_at: Ustr::from("2024-01-01T00:00:00Z"),
        updated_at: Ustr::from("2024-01-01T00:00:00Z"),
    }
}

/// Helper to create a sample GroupContentItem for testing.
pub fn sample_group_content_item(id: &str, group_id: &str, content_id: &str) -> GroupContentItem {
    GroupContentItem {
        id: Ustr::from(id),
        group_id: Ustr::from(group_id),
        content_id: Ustr::from(content_id),
        sort_order: 0,
        added_at: Ustr::from("2024-01-01T00:00:00Z"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::ContentCreationStatus;
    use crate::list_content::{GetContentStatus, GetViewUrlStatus, ListContentsStatus};
    use crate::list_groups::{
        AddGroupContentsStatus, CreateGroupStatus, GetGroupContentsStatus, ListGroupsStatus,
    };

    #[tokio::test]
    async fn test_context_creation() {
        let test_ctx = TestContext::new().await;
        // Verify the mock server is running

        assert!(!test_ctx.mock_server.uri().is_empty());
    }

    #[tokio::test]
    async fn test_list_contents_success() {
        let mut test_ctx = TestContext::new().await;

        // Set up auth
        test_ctx.set_authenticated("test_token");

        // Mount mock response
        let items = vec![sample_content_item("1"), sample_content_item("2")];
        test_ctx.mock_list_contents(items, 2).await;

        // Execute command
        test_ctx.ctx.update::<ListContentsInput>(|input| {
            input.limit = Some(10);
            input.offset = Some(0);
        });
        test_ctx.ctx.enqueue_command::<ListContentsCommand>();
        test_ctx.flush_and_wait().await;

        // Verify result
        let compute = test_ctx.ctx.compute::<ListContentsCompute>();
        match &compute.status {
            ListContentsStatus::Success(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].id, "1");
                assert_eq!(items[1].id, "2");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_list_contents_unauthenticated() {
        let mut test_ctx = TestContext::new().await;

        // Don't set auth - should fail
        test_ctx.mock_list_contents(vec![], 0).await;

        test_ctx.ctx.enqueue_command::<ListContentsCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<ListContentsCompute>();
        match &compute.status {
            ListContentsStatus::Error(msg) => {
                assert!(
                    msg.to_lowercase().contains("authenticated")
                        || msg.to_lowercase().contains("auth")
                        || msg.to_lowercase().contains("not logged")
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_get_content_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        let content = sample_text_content("123", "Hello, World!");
        test_ctx.mock_get_content("123", content.clone()).await;

        test_ctx.ctx.update::<GetContentInput>(|input| {
            input.id = Ustr::from("123");
        });
        test_ctx.ctx.enqueue_command::<GetContentCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<GetContentCompute>();
        match &compute.status {
            GetContentStatus::Success(item) => {
                assert_eq!(item.id, "123");
                assert_eq!(item.body, Some("Hello, World!".to_owned()));
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_get_content_not_found() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        test_ctx.mock_get_content_not_found("999").await;

        test_ctx.ctx.update::<GetContentInput>(|input| {
            input.id = Ustr::from("999");
        });
        test_ctx.ctx.enqueue_command::<GetContentCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<GetContentCompute>();
        assert!(matches!(compute.status, GetContentStatus::NotFound));

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_get_view_url_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        test_ctx
            .mock_view_url(
                "123",
                "https://example.com/file.pdf",
                "2024-12-31T23:59:59Z",
            )
            .await;

        test_ctx.ctx.update::<GetViewUrlInput>(|input| {
            input.content_id = Ustr::from("123");
            input.disposition = Ustr::from("inline");
        });
        test_ctx.ctx.enqueue_command::<GetViewUrlCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<GetViewUrlCompute>();
        match &compute.status {
            GetViewUrlStatus::Success(data) => {
                assert_eq!(data.url, "https://example.com/file.pdf");
                assert_eq!(data.expires_at, "2024-12-31T23:59:59Z");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_create_inline_content_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        test_ctx.mock_create_content("new-content-id").await;

        test_ctx.ctx.update::<CreateContentInput>(|input| {
            input.title = Some("My Note".to_owned());
            input.body = Some("This is the content body".to_owned());
        });
        test_ctx.ctx.enqueue_command::<CreateContentCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<CreateContentCompute>();
        match &compute.status {
            ContentCreationStatus::Success(ids) => {
                assert_eq!(ids.len(), 1);
                assert_eq!(ids[0], "new-content-id");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_create_content_unauthenticated() {
        let mut test_ctx = TestContext::new().await;
        // Don't set auth

        test_ctx.ctx.update::<CreateContentInput>(|input| {
            input.body = Some("Some content".to_owned());
        });
        test_ctx.ctx.enqueue_command::<CreateContentCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<CreateContentCompute>();
        match &compute.status {
            ContentCreationStatus::Error(msg) => {
                assert!(
                    msg.to_lowercase().contains("authenticated")
                        || msg.to_lowercase().contains("auth")
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_upload_file_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        // Mock the full upload flow
        test_ctx.mock_full_upload("upload-123", "content-456").await;

        test_ctx.ctx.update::<CreateContentInput>(|input| {
            input.attachments = vec![crate::Attachment {
                filename: "test.txt".to_owned(),
                mime_type: "text/plain".to_owned(),
                data: b"Hello, World!".to_vec(),
            }];
        });
        test_ctx.ctx.enqueue_command::<CreateContentCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<CreateContentCompute>();
        match &compute.status {
            ContentCreationStatus::Success(ids) => {
                assert_eq!(ids.len(), 1);
                assert_eq!(ids[0], "content-456");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    // =========================================================================
    // Group (Collect) tests
    // =========================================================================

    #[tokio::test]
    async fn test_list_groups_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        let groups = vec![
            sample_group_item("group-1", "My First Collect"),
            sample_group_item("group-2", "My Second Collect"),
        ];
        test_ctx.mock_list_groups(groups, 2).await;

        test_ctx.ctx.update::<ListGroupsInput>(|input| {
            input.limit = Some(10);
            input.offset = Some(0);
        });
        test_ctx.ctx.enqueue_command::<ListGroupsCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<ListGroupsCompute>();
        match &compute.status {
            ListGroupsStatus::Success(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].name.as_str(), "My First Collect");
                assert_eq!(items[1].name.as_str(), "My Second Collect");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_create_group_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        let group = sample_group_item("new-group-id", "Test Collect");
        test_ctx.mock_create_group(group).await;

        test_ctx.ctx.update::<CreateGroupInput>(|input| {
            input.name = Some("Test Collect".to_owned());
            input.description = None;
            input.visibility = Some("private".to_owned());
        });
        test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<CreateGroupCompute>();
        match &compute.status {
            CreateGroupStatus::Success(group) => {
                assert_eq!(group.id.as_str(), "new-group-id");
                assert_eq!(group.name.as_str(), "Test Collect");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_create_group_unauthenticated() {
        let mut test_ctx = TestContext::new().await;
        // Don't set auth

        test_ctx.ctx.update::<CreateGroupInput>(|input| {
            input.name = Some("Test Collect".to_owned());
        });
        test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<CreateGroupCompute>();
        match &compute.status {
            CreateGroupStatus::Error(msg) => {
                assert!(
                    msg.to_lowercase().contains("authenticated")
                        || msg.to_lowercase().contains("auth")
                );
            }
            other => panic!("Expected Error, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_add_group_contents_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        test_ctx.mock_add_group_content("group-123").await;

        test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
            input.group_id = Some(Ustr::from("group-123"));
            input.content_ids = vec![Ustr::from("content-1"), Ustr::from("content-2")];
        });
        test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<AddGroupContentsCompute>();
        match &compute.status {
            AddGroupContentsStatus::Success { added } => {
                assert_eq!(*added, 2);
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_get_group_contents_success() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        let items = vec![
            sample_group_content_item("gc-1", "group-123", "content-1"),
            sample_group_content_item("gc-2", "group-123", "content-2"),
        ];
        test_ctx.mock_get_group_contents("group-123", items).await;

        test_ctx.ctx.update::<GetGroupContentsInput>(|input| {
            input.group_id = Some(Ustr::from("group-123"));
        });
        test_ctx.ctx.enqueue_command::<GetGroupContentsCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<GetGroupContentsCompute>();
        match &compute.status {
            GetGroupContentsStatus::Success(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].content_id.as_str(), "content-1");
                assert_eq!(items[1].content_id.as_str(), "content-2");
            }
            other => panic!("Expected Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    #[tokio::test]
    async fn test_get_group_contents_not_found() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        test_ctx
            .mock_get_group_contents_not_found("nonexistent-group")
            .await;

        test_ctx.ctx.update::<GetGroupContentsInput>(|input| {
            input.group_id = Some(Ustr::from("nonexistent-group"));
        });
        test_ctx.ctx.enqueue_command::<GetGroupContentsCommand>();
        test_ctx.flush_and_wait().await;

        let compute = test_ctx.ctx.compute::<GetGroupContentsCompute>();
        assert!(matches!(compute.status, GetGroupContentsStatus::NotFound));

        test_ctx.shutdown().await;
    }

    /// Test the full "new collect" workflow:
    /// 1. Create a group
    /// 2. Create inline content
    /// 3. Add content to the group
    #[tokio::test]
    async fn test_new_collect_workflow() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        // Mock create group
        let group = sample_group_item("new-group-id", "My New Collect");
        test_ctx.mock_create_group(group).await;

        // Mock create inline content
        test_ctx.mock_create_content("new-content-id").await;

        // Mock add content to group
        test_ctx.mock_add_group_content("new-group-id").await;

        // Step 1: Create the group
        test_ctx.ctx.update::<CreateGroupInput>(|input| {
            input.name = Some("My New Collect".to_owned());
        });
        test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
        test_ctx.flush_and_wait().await;

        let group = match &test_ctx.ctx.compute::<CreateGroupCompute>().status {
            CreateGroupStatus::Success(g) => g.clone(),
            other => panic!("Expected CreateGroupStatus::Success, got {:?}", other),
        };
        assert_eq!(group.id.as_str(), "new-group-id");

        // Step 2: Create content
        test_ctx.ctx.update::<CreateContentInput>(|input| {
            input.body = Some("This is my note content".to_owned());
        });
        test_ctx.ctx.enqueue_command::<CreateContentCommand>();
        test_ctx.flush_and_wait().await;

        let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
            ContentCreationStatus::Success(ids) => ids.clone(),
            other => panic!("Expected ContentCreationStatus::Success, got {:?}", other),
        };
        assert_eq!(content_ids.len(), 1);

        // Step 3: Add content to group
        test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
            input.group_id = Some(group.id);
            input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
        });
        test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
        test_ctx.flush_and_wait().await;

        match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
            AddGroupContentsStatus::Success { added } => {
                assert_eq!(*added, 1);
            }
            other => panic!("Expected AddGroupContentsStatus::Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }

    /// Test the "add to collect" workflow:
    /// 1. Create content (file upload)
    /// 2. Add content to existing group
    #[tokio::test]
    async fn test_add_to_collect_workflow() {
        let mut test_ctx = TestContext::new().await;
        test_ctx.set_authenticated("test_token");

        // Mock file upload
        test_ctx.mock_full_upload("upload-123", "content-456").await;

        // Mock add content to group
        test_ctx.mock_add_group_content("existing-group-id").await;

        // Step 1: Upload file
        test_ctx.ctx.update::<CreateContentInput>(|input| {
            input.attachments = vec![crate::Attachment {
                filename: "document.pdf".to_owned(),
                mime_type: "application/pdf".to_owned(),
                data: b"PDF content here".to_vec(),
            }];
        });
        test_ctx.ctx.enqueue_command::<CreateContentCommand>();
        test_ctx.flush_and_wait().await;

        let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
            ContentCreationStatus::Success(ids) => ids.clone(),
            other => panic!("Expected ContentCreationStatus::Success, got {:?}", other),
        };
        assert_eq!(content_ids.len(), 1);

        // Step 2: Add content to existing group
        test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
            input.group_id = Some(Ustr::from("existing-group-id"));
            input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
        });
        test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
        test_ctx.flush_and_wait().await;

        match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
            AddGroupContentsStatus::Success { added } => {
                assert_eq!(*added, 1);
            }
            other => panic!("Expected AddGroupContentsStatus::Success, got {:?}", other),
        }

        test_ctx.shutdown().await;
    }
}
