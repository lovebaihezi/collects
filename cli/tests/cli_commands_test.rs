//! CLI integration tests using mock servers.
//!
//! These tests exercise the CLI's business logic for `new` and `add` commands
//! by mocking the API endpoints with wiremock.
//!
//! Note: These tests don't spawn the actual CLI binary but instead test the
//! underlying command workflows directly using `StateCtx` and mock servers.

#![cfg(all(test, not(target_arch = "wasm32")))]

use std::time::Duration;

use collects_business::{
    AddGroupContentsCommand, AddGroupContentsCompute, AddGroupContentsInput,
    AddGroupContentsStatus, Attachment, AuthCompute, BusinessConfig, CreateContentCommand,
    CreateContentCompute, CreateContentInput, CreateGroupCommand, CreateGroupCompute,
    CreateGroupInput, CreateGroupStatus, ListGroupsCommand, ListGroupsCompute, ListGroupsInput,
    ListGroupsStatus,
};
use collects_states::StateCtx;
use ustr::Ustr;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

/// Test context for CLI integration tests.
struct CliTestContext {
    mock_server: MockServer,
    ctx: StateCtx,
}

impl CliTestContext {
    /// Create a new test context with a fresh mock server.
    async fn new() -> Self {
        let mock_server = MockServer::start().await;
        let base_url = mock_server.uri();

        let config = BusinessConfig::new(base_url);
        let ctx = build_cli_test_state_ctx(config);

        Self { mock_server, ctx }
    }

    /// Set the authentication state to authenticated with the given token.
    fn set_authenticated(&mut self, token: &str) {
        let updater = self.ctx.updater();
        updater.set(AuthCompute::new_authenticated(
            token.to_owned(),
            "test_user".to_owned(),
        ));
        self.ctx.sync_computes();
    }

    /// Flush all pending commands and wait for async tasks to complete.
    async fn flush_and_wait(&mut self) {
        self.ctx.sync_computes();
        self.ctx.flush_commands();

        let timeout = Duration::from_secs(5);
        let start = std::time::Instant::now();

        while self.ctx.task_count() > 0 {
            assert!(
                start.elapsed() <= timeout,
                "Timed out waiting for pending tasks ({} still in JoinSet)",
                self.ctx.task_count()
            );

            if self.ctx.task_set_mut().join_next().await.is_some() {
                self.ctx.sync_computes();
            }
        }

        self.ctx.sync_computes();
    }

    /// Shutdown the context.
    async fn shutdown(&mut self) {
        self.ctx.shutdown().await;
    }

    // =========================================================================
    // Mock helpers
    // =========================================================================

    /// Mock the list groups endpoint.
    async fn mock_list_groups(&self, items: Vec<serde_json::Value>, total: usize) {
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
    async fn mock_create_group(&self, id: &str, name: &str) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": id,
            "name": name,
            "description": null,
            "visibility": "private",
            "status": "active",
            "trashed_at": null,
            "archived_at": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/groups"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the create group endpoint with an error.
    async fn mock_create_group_error(&self, status: u16, error: &str) {
        let response = ResponseTemplate::new(status).set_body_json(serde_json::json!({
            "error": error
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/groups"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the create inline content endpoint.
    async fn mock_create_content(&self, content_id: &str) {
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

    /// Mock the add content to group endpoint.
    async fn mock_add_group_content(&self, group_id: &str) {
        let response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "success": true
        }));

        Mock::given(method("POST"))
            .and(path(format!("/api/v1/groups/{group_id}/contents")))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the add content to group endpoint with error.
    async fn mock_add_group_content_error(&self, group_id: &str, status: u16, error: &str) {
        let response = ResponseTemplate::new(status).set_body_json(serde_json::json!({
            "error": error
        }));

        Mock::given(method("POST"))
            .and(path(format!("/api/v1/groups/{group_id}/contents")))
            .respond_with(response)
            .mount(&self.mock_server)
            .await;
    }

    /// Mock the full upload flow (init + put + complete).
    async fn mock_full_upload(&self, upload_id: &str, content_id: &str) {
        // Mock upload init
        let upload_url = format!("{}/mock-upload/{}", self.mock_server.uri(), upload_id);
        let init_response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "upload_id": upload_id,
            "upload_url": upload_url
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/uploads/init"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(init_response)
            .mount(&self.mock_server)
            .await;

        // Mock upload PUT
        let put_response = ResponseTemplate::new(200);
        Mock::given(method("PUT"))
            .and(path(format!("/mock-upload/{upload_id}")))
            .respond_with(put_response)
            .mount(&self.mock_server)
            .await;

        // Mock upload complete
        let complete_response = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "content": {
                "id": content_id
            }
        }));

        Mock::given(method("POST"))
            .and(path("/api/v1/uploads/complete"))
            .and(header("Authorization", "Bearer test_token"))
            .respond_with(complete_response)
            .mount(&self.mock_server)
            .await;
    }
}

/// Build a `StateCtx` configured for CLI testing with all necessary states and commands.
fn build_cli_test_state_ctx(config: BusinessConfig) -> StateCtx {
    use collects_business::LoginInput;
    use collects_business::{
        CFTokenCompute, LoginCommand, PendingTokenValidation, ValidateTokenCommand,
    };

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

    // Group creation states and computes
    ctx.add_state(CreateGroupInput::default());
    ctx.record_compute(CreateGroupCompute::default());

    // Add-to-group states and computes
    ctx.add_state(AddGroupContentsInput::default());
    ctx.record_compute(AddGroupContentsCompute::default());

    // List groups states and computes
    ctx.add_state(ListGroupsInput::default());
    ctx.record_compute(ListGroupsCompute::default());

    // Commands
    ctx.record_command(LoginCommand);
    ctx.record_command(ValidateTokenCommand);
    ctx.record_command(CreateContentCommand);
    ctx.record_command(CreateGroupCommand);
    ctx.record_command(AddGroupContentsCommand);
    ctx.record_command(ListGroupsCommand);

    ctx
}

// =============================================================================
// Tests for `collects new` workflow
// =============================================================================

/// Test creating a new collect with inline text content (simulates stdin input).
#[tokio::test]
async fn test_new_collect_with_text() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    // Mock the API endpoints
    test_ctx
        .mock_create_group("group-123", "My Test Collect")
        .await;
    test_ctx.mock_create_content("content-456").await;
    test_ctx.mock_add_group_content("group-123").await;

    // Step 1: Create the group (simulates what `run_new` does first)
    test_ctx.ctx.update::<CreateGroupInput>(|input| {
        input.name = Some("My Test Collect".to_owned());
        input.description = None;
        input.visibility = None;
    });
    test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
    test_ctx.flush_and_wait().await;

    // Verify group was created
    let group = match &test_ctx.ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Success(g) => g.clone(),
        other => panic!("Expected CreateGroupStatus::Success, got {other:?}"),
    };
    assert_eq!(group.id.as_str(), "group-123");
    assert_eq!(group.name.as_str(), "My Test Collect");

    // Step 2: Create content from text (simulates stdin body)
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.title = None;
        input.body = Some("This is text from stdin".to_owned());
        input.attachments = vec![];
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    // Verify content was created
    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
    };
    assert_eq!(content_ids.len(), 1);
    assert_eq!(content_ids[0], "content-456");

    // Step 3: Add content to the group
    test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
        input.group_id = Some(group.id);
        input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });
    test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
    test_ctx.flush_and_wait().await;

    // Verify content was added to group
    match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            assert_eq!(*added, 1);
        }
        other => panic!("Expected AddGroupContentsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test creating a new collect with a file attachment.
#[tokio::test]
async fn test_new_collect_with_file() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    // Mock the API endpoints
    test_ctx
        .mock_create_group("group-789", "File Collect")
        .await;
    test_ctx.mock_full_upload("upload-abc", "content-xyz").await;
    test_ctx.mock_add_group_content("group-789").await;

    // Step 1: Create the group
    test_ctx.ctx.update::<CreateGroupInput>(|input| {
        input.name = Some("File Collect".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
    test_ctx.flush_and_wait().await;

    let group = match &test_ctx.ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Success(g) => g.clone(),
        other => panic!("Expected CreateGroupStatus::Success, got {other:?}"),
    };
    assert_eq!(group.id.as_str(), "group-789");

    // Step 2: Upload file (simulates file attachment)
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.attachments = vec![Attachment {
            filename: "test-document.pdf".to_owned(),
            mime_type: "application/pdf".to_owned(),
            data: b"fake pdf content".to_vec(),
        }];
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
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
        other => panic!("Expected AddGroupContentsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test that creating a collect fails properly when not authenticated.
#[tokio::test]
async fn test_new_collect_unauthenticated() {
    let mut test_ctx = CliTestContext::new().await;
    // Don't call set_authenticated()

    test_ctx.ctx.update::<CreateGroupInput>(|input| {
        input.name = Some("Should Fail".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Error(msg) => {
            assert!(
                msg.to_lowercase().contains("authenticated") || msg.to_lowercase().contains("auth"),
                "Expected auth error, got: {msg}"
            );
        }
        other => panic!("Expected CreateGroupStatus::Error, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test that API errors are propagated correctly for group creation.
#[tokio::test]
async fn test_new_collect_api_error() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    // Mock a server error
    test_ctx
        .mock_create_group_error(500, "Internal server error")
        .await;

    test_ctx.ctx.update::<CreateGroupInput>(|input| {
        input.name = Some("Will Fail".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Error(msg) => {
            assert!(
                msg.contains("error") || msg.contains("Error") || msg.contains("500"),
                "Expected error message, got: {msg}"
            );
        }
        other => panic!("Expected CreateGroupStatus::Error, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

// =============================================================================
// Tests for `collects add` workflow
// =============================================================================

/// Test adding text content to an existing collect.
#[tokio::test]
async fn test_add_text_to_existing_collect() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    let existing_group_id = "existing-group-111";

    // Mock the API endpoints
    test_ctx.mock_create_content("new-content-222").await;
    test_ctx.mock_add_group_content(existing_group_id).await;

    // Step 1: Create content from text (simulates stdin)
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.body = Some("Adding this note to existing collect".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
    };
    assert_eq!(content_ids.len(), 1);

    // Step 2: Add content to the existing group
    test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
        input.group_id = Some(Ustr::from(existing_group_id));
        input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });
    test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            assert_eq!(*added, 1);
        }
        other => panic!("Expected AddGroupContentsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test adding a file to an existing collect.
#[tokio::test]
async fn test_add_file_to_existing_collect() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    let existing_group_id = "existing-group-333";

    // Mock the API endpoints
    test_ctx.mock_full_upload("upload-444", "content-555").await;
    test_ctx.mock_add_group_content(existing_group_id).await;

    // Step 1: Upload file
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.attachments = vec![Attachment {
            filename: "image.png".to_owned(),
            mime_type: "image/png".to_owned(),
            data: vec![0x89, 0x50, 0x4E, 0x47], // PNG magic bytes
        }];
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
    };
    assert_eq!(content_ids.len(), 1);

    // Step 2: Add content to existing group
    test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
        input.group_id = Some(Ustr::from(existing_group_id));
        input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });
    test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            assert_eq!(*added, 1);
        }
        other => panic!("Expected AddGroupContentsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test adding multiple files to an existing collect.
#[tokio::test]
async fn test_add_multiple_files_to_collect() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    let existing_group_id = "existing-group-multi";

    // For multiple files, we need to mock multiple upload flows
    // Since the upload init is matched generically, we can use the same mock
    // The mock_full_upload sets up init/put/complete that can handle multiple calls
    test_ctx
        .mock_full_upload("upload-multi", "content-multi")
        .await;
    test_ctx.mock_add_group_content(existing_group_id).await;

    // Upload multiple files (in a real scenario, each would get its own upload ID)
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.attachments = vec![
            Attachment {
                filename: "file1.txt".to_owned(),
                mime_type: "text/plain".to_owned(),
                data: b"File 1 content".to_vec(),
            },
            Attachment {
                filename: "file2.txt".to_owned(),
                mime_type: "text/plain".to_owned(),
                data: b"File 2 content".to_vec(),
            },
        ];
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
    };
    // Note: The actual count depends on how CreateContentCommand handles multiple attachments
    assert!(!content_ids.is_empty());

    // Add all created content to group
    test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
        input.group_id = Some(Ustr::from(existing_group_id));
        input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });
    test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            assert!(*added > 0);
        }
        other => panic!("Expected AddGroupContentsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test that adding content fails properly when not authenticated.
#[tokio::test]
async fn test_add_to_collect_unauthenticated() {
    let mut test_ctx = CliTestContext::new().await;
    // Don't call set_authenticated()

    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.body = Some("Should fail".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Error(msg) => {
            assert!(
                msg.to_lowercase().contains("authenticated") || msg.to_lowercase().contains("auth"),
                "Expected auth error, got: {msg}"
            );
        }
        other => panic!("Expected ContentCreationStatus::Error, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test that errors when adding content to a group are propagated.
#[tokio::test]
async fn test_add_to_collect_group_error() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    let group_id = "group-with-error";

    // Mock content creation success but group add failure
    test_ctx.mock_create_content("content-ok").await;
    test_ctx
        .mock_add_group_content_error(group_id, 404, "Group not found")
        .await;

    // Create content successfully
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.body = Some("Content that will fail to add".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
    };

    // Try to add to group (should fail)
    test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
        input.group_id = Some(Ustr::from(group_id));
        input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });
    test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Error(msg) => {
            assert!(
                msg.contains("not found") || msg.contains("404") || msg.contains("Not Found"),
                "Expected not found error, got: {msg}"
            );
        }
        other => panic!("Expected AddGroupContentsStatus::Error, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

// =============================================================================
// Tests for `collects list` workflow
// =============================================================================

/// Test listing collects (groups).
#[tokio::test]
async fn test_list_collects() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    let groups = vec![
        serde_json::json!({
            "id": "group-1",
            "name": "First Collect",
            "description": "My first collect",
            "visibility": "private",
            "status": "active",
            "trashed_at": null,
            "archived_at": null,
            "created_at": "2024-01-01T00:00:00Z",
            "updated_at": "2024-01-01T00:00:00Z"
        }),
        serde_json::json!({
            "id": "group-2",
            "name": "Second Collect",
            "description": null,
            "visibility": "private",
            "status": "active",
            "trashed_at": null,
            "archived_at": null,
            "created_at": "2024-01-02T00:00:00Z",
            "updated_at": "2024-01-02T00:00:00Z"
        }),
    ];
    test_ctx.mock_list_groups(groups, 2).await;

    test_ctx.ctx.update::<ListGroupsInput>(|input| {
        input.limit = Some(20);
        input.offset = Some(0);
        input.status = None;
    });
    test_ctx.ctx.enqueue_command::<ListGroupsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<ListGroupsCompute>().status {
        ListGroupsStatus::Success(items) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].name.as_str(), "First Collect");
            assert_eq!(items[1].name.as_str(), "Second Collect");
        }
        other => panic!("Expected ListGroupsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

/// Test listing collects when not authenticated.
#[tokio::test]
async fn test_list_collects_unauthenticated() {
    let mut test_ctx = CliTestContext::new().await;
    // Don't call set_authenticated()

    test_ctx.ctx.update::<ListGroupsInput>(|input| {
        input.limit = Some(20);
    });
    test_ctx.ctx.enqueue_command::<ListGroupsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<ListGroupsCompute>().status {
        ListGroupsStatus::Error(msg) => {
            assert!(
                msg.to_lowercase().contains("authenticated") || msg.to_lowercase().contains("auth"),
                "Expected auth error, got: {msg}"
            );
        }
        other => panic!("Expected ListGroupsStatus::Error, got {other:?}"),
    }

    test_ctx.shutdown().await;
}

// =============================================================================
// Tests for mixed content scenarios
// =============================================================================

/// Test creating a collect with both text and file content.
#[tokio::test]
async fn test_new_collect_with_text_and_file() {
    let mut test_ctx = CliTestContext::new().await;
    test_ctx.set_authenticated("test_token");

    test_ctx
        .mock_create_group("mixed-group", "Mixed Content Collect")
        .await;
    // Mock for inline text
    test_ctx.mock_create_content("text-content-id").await;
    // Mock for file upload
    test_ctx
        .mock_full_upload("file-upload-id", "file-content-id")
        .await;
    test_ctx.mock_add_group_content("mixed-group").await;

    // Create the group
    test_ctx.ctx.update::<CreateGroupInput>(|input| {
        input.name = Some("Mixed Content Collect".to_owned());
    });
    test_ctx.ctx.enqueue_command::<CreateGroupCommand>();
    test_ctx.flush_and_wait().await;

    let group = match &test_ctx.ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Success(g) => g.clone(),
        other => panic!("Expected CreateGroupStatus::Success, got {other:?}"),
    };

    // Create content with both text body and file attachment
    test_ctx.ctx.update::<CreateContentInput>(|input| {
        input.body = Some("Some text notes".to_owned());
        input.attachments = vec![Attachment {
            filename: "doc.pdf".to_owned(),
            mime_type: "application/pdf".to_owned(),
            data: b"PDF data".to_vec(),
        }];
    });
    test_ctx.ctx.enqueue_command::<CreateContentCommand>();
    test_ctx.flush_and_wait().await;

    let content_ids = match &test_ctx.ctx.compute::<CreateContentCompute>().status {
        collects_business::ContentCreationStatus::Success(ids) => ids.clone(),
        other => panic!("Expected ContentCreationStatus::Success, got {other:?}"),
    };
    // Should have at least one content (text or file)
    assert!(!content_ids.is_empty());

    // Add all content to group
    test_ctx.ctx.update::<AddGroupContentsInput>(|input| {
        input.group_id = Some(group.id);
        input.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });
    test_ctx.ctx.enqueue_command::<AddGroupContentsCommand>();
    test_ctx.flush_and_wait().await;

    match &test_ctx.ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            assert!(*added > 0);
        }
        other => panic!("Expected AddGroupContentsStatus::Success, got {other:?}"),
    }

    test_ctx.shutdown().await;
}
