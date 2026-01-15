//! List and get content business operations.
//!
//! Provides commands and computes for listing user's content and getting specific content by ID.

use std::any::Any;

use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::BusinessConfig;
use crate::cf_token_compute::CFTokenCompute;
use crate::http::Client;
use crate::login_state::AuthCompute;
use collects_states::{
    Command, CommandSnapshot, Compute, ComputeDeps, Dep, LatestOnlyUpdater, SnapshotClone, State,
    Updater, assign_impl, state_assign_impl,
};

// ============================================================================
// Content Item (shared response type)
// ============================================================================

/// A content item from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentItem {
    /// Unique identifier (UUID).
    pub id: Ustr,
    /// Content title.
    pub title: Ustr,
    /// Optional description.
    #[serde(default)]
    pub description: Option<Ustr>,
    /// Storage backend identifier.
    pub storage_backend: Ustr,
    /// Storage profile identifier.
    pub storage_profile: Ustr,
    /// Storage key/path.
    pub storage_key: Ustr,
    /// MIME type of the content.
    pub content_type: Ustr,
    /// File size in bytes.
    pub file_size: i64,
    /// Content status (active, archived, trashed).
    pub status: Ustr,
    /// Content visibility (private, public, restricted).
    pub visibility: Ustr,
    /// Content kind: "file" (uploaded to R2) or "text" (stored inline).
    pub kind: Ustr,
    /// Inline text content (only present when kind="text").
    #[serde(default)]
    pub body: Option<String>,
    /// Timestamp when content was trashed (ISO 8601 format).
    #[serde(default)]
    pub trashed_at: Option<Ustr>,
    /// Timestamp when content was archived (ISO 8601 format).
    #[serde(default)]
    pub archived_at: Option<Ustr>,
    /// Timestamp when content was created (ISO 8601 format).
    pub created_at: Ustr,
    /// Timestamp when content was last updated (ISO 8601 format).
    pub updated_at: Ustr,
}

impl ContentItem {
    /// Returns true if this is a file (stored in R2).
    pub fn is_file(&self) -> bool {
        self.kind == "file"
    }

    /// Returns true if this is inline text content.
    pub fn is_text(&self) -> bool {
        self.kind == "text"
    }

    /// Returns a human-readable size string.
    pub fn size_display(&self) -> String {
        if self.file_size < 1024 {
            format!("{} B", self.file_size)
        } else if self.file_size < 1024 * 1024 {
            format!("{:.1} KB", self.file_size as f64 / 1024.0)
        } else if self.file_size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.file_size as f64 / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.2} GB",
                self.file_size as f64 / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }
}

/// Response from listing contents.
#[derive(Debug, Clone, Deserialize)]
pub struct ListContentsResponse {
    /// List of content items.
    pub items: Vec<ContentItem>,
    /// Total number of items returned.
    pub total: usize,
}

// ============================================================================
// List Contents
// ============================================================================

/// Input parameters for listing contents.
#[derive(Default, Debug, Clone)]
pub struct ListContentsInput {
    /// Maximum number of items to return (1-100, default 50).
    pub limit: Option<i32>,
    /// Offset for pagination.
    pub offset: Option<i32>,
    /// Filter by status: "active", "archived", "trashed".
    pub status: Option<Ustr>,
}

impl SnapshotClone for ListContentsInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for ListContentsInput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Status of the list contents operation.
#[derive(Debug, Clone, Default)]
pub enum ListContentsStatus {
    #[default]
    Idle,
    Loading,
    Success(Vec<ContentItem>),
    Error(String),
}

/// Compute to track list contents status.
#[derive(Default, Debug, Clone)]
pub struct ListContentsCompute {
    pub status: ListContentsStatus,
}

impl SnapshotClone for ListContentsCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for ListContentsCompute {
    fn deps(&self) -> ComputeDeps {
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // No-op, updated by command
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for ListContentsCompute {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Command to list contents for the authenticated user.
#[derive(Default, Debug)]
pub struct ListContentsCommand;

impl Command for ListContentsCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: ListContentsInput = snap.state::<ListContentsInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            if !auth.is_authenticated() {
                updater.set(ListContentsCompute {
                    status: ListContentsStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            updater.set(ListContentsCompute {
                status: ListContentsStatus::Loading,
            });

            let token = auth.token().unwrap_or_default();

            // Build query parameters
            let mut params = Vec::new();
            if let Some(limit) = input.limit {
                params.push(format!("limit={}", limit));
            }
            if let Some(offset) = input.offset {
                params.push(format!("offset={}", offset));
            }
            if let Some(status) = &input.status {
                params.push(format!("status={}", status));
            }

            let url = if params.is_empty() {
                format!("{}/v1/contents", config.api_url())
            } else {
                format!("{}/v1/contents?{}", config.api_url(), params.join("&"))
            };

            let request = Client::get(&url).header("Authorization", format!("Bearer {token}"));

            let request = if let Some(cf) = cf_token.token() {
                request.header("cf-access-token", cf)
            } else {
                request
            };

            match request.send().await {
                Ok(response) => {
                    if response.is_success() {
                        match response.json::<ListContentsResponse>() {
                            Ok(resp) => {
                                updater.set(ListContentsCompute {
                                    status: ListContentsStatus::Success(resp.items),
                                });
                            }
                            Err(e) => {
                                updater.set(ListContentsCompute {
                                    status: ListContentsStatus::Error(format!(
                                        "Failed to parse response: {}",
                                        e
                                    )),
                                });
                            }
                        }
                    } else {
                        let error = response
                            .text()
                            .unwrap_or_else(|_| "Unknown error".to_owned());
                        updater.set(ListContentsCompute {
                            status: ListContentsStatus::Error(error),
                        });
                    }
                }
                Err(e) => {
                    updater.set(ListContentsCompute {
                        status: ListContentsStatus::Error(e.to_string()),
                    });
                }
            }
        })
    }
}

// ============================================================================
// Get Content
// ============================================================================

/// Input for getting a specific content by ID.
#[derive(Default, Debug, Clone)]
pub struct GetContentInput {
    /// Content ID (UUID string).
    pub id: Ustr,
}

impl SnapshotClone for GetContentInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for GetContentInput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Status of the get content operation.
#[derive(Debug, Clone, Default)]
pub enum GetContentStatus {
    #[default]
    Idle,
    Loading,
    Success(ContentItem),
    NotFound,
    Error(String),
}

/// Compute to track get content status.
#[derive(Default, Debug, Clone)]
pub struct GetContentCompute {
    pub status: GetContentStatus,
}

impl SnapshotClone for GetContentCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for GetContentCompute {
    fn deps(&self) -> ComputeDeps {
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // No-op, updated by command
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for GetContentCompute {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Command to get a specific content by ID.
#[derive(Default, Debug)]
pub struct GetContentCommand;

impl Command for GetContentCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: GetContentInput = snap.state::<GetContentInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            if !auth.is_authenticated() {
                updater.set(GetContentCompute {
                    status: GetContentStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            if input.id.is_empty() {
                updater.set(GetContentCompute {
                    status: GetContentStatus::Error("Content ID is required".to_owned()),
                });
                return;
            }

            updater.set(GetContentCompute {
                status: GetContentStatus::Loading,
            });

            let token = auth.token().unwrap_or_default();
            let url = format!("{}/v1/contents/{}", config.api_url(), input.id);

            let request = Client::get(&url).header("Authorization", format!("Bearer {token}"));

            let request = if let Some(cf) = cf_token.token() {
                request.header("cf-access-token", cf)
            } else {
                request
            };

            match request.send().await {
                Ok(response) => {
                    if response.is_success() {
                        match response.json::<ContentItem>() {
                            Ok(item) => {
                                updater.set(GetContentCompute {
                                    status: GetContentStatus::Success(item),
                                });
                            }
                            Err(e) => {
                                updater.set(GetContentCompute {
                                    status: GetContentStatus::Error(format!(
                                        "Failed to parse response: {}",
                                        e
                                    )),
                                });
                            }
                        }
                    } else if response.status == 404 {
                        updater.set(GetContentCompute {
                            status: GetContentStatus::NotFound,
                        });
                    } else {
                        let error = response
                            .text()
                            .unwrap_or_else(|_| "Unknown error".to_owned());
                        updater.set(GetContentCompute {
                            status: GetContentStatus::Error(error),
                        });
                    }
                }
                Err(e) => {
                    updater.set(GetContentCompute {
                        status: GetContentStatus::Error(e.to_string()),
                    });
                }
            }
        })
    }
}

// ============================================================================
// Get View URL
// ============================================================================

/// Input for getting a view URL for a content.
#[derive(Default, Debug, Clone)]
pub struct GetViewUrlInput {
    /// Content ID (UUID string).
    pub content_id: Ustr,
    /// Disposition: "inline" or "attachment".
    pub disposition: Ustr,
}

impl SnapshotClone for GetViewUrlInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for GetViewUrlInput {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// View URL response data.
#[derive(Debug, Clone, Deserialize)]
pub struct ViewUrlData {
    /// Presigned URL for viewing/downloading the content.
    pub url: Ustr,
    /// Expiration timestamp (ISO 8601 format).
    pub expires_at: Ustr,
}

/// Status of the get view URL operation.
#[derive(Debug, Clone, Default)]
pub enum GetViewUrlStatus {
    #[default]
    Idle,
    Loading,
    Success(ViewUrlData),
    NotFound,
    Error(String),
}

/// Compute to track get view URL status.
#[derive(Default, Debug, Clone)]
pub struct GetViewUrlCompute {
    pub status: GetViewUrlStatus,
}

impl SnapshotClone for GetViewUrlCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for GetViewUrlCompute {
    fn deps(&self) -> ComputeDeps {
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // No-op, updated by command
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}

impl State for GetViewUrlCompute {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

/// Request body for view-url endpoint.
#[derive(Debug, Serialize)]
struct ViewUrlRequest {
    disposition: String,
}

/// Command to get a view URL for a content.
#[derive(Default, Debug)]
pub struct GetViewUrlCommand;

impl Command for GetViewUrlCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: GetViewUrlInput = snap.state::<GetViewUrlInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            if !auth.is_authenticated() {
                updater.set(GetViewUrlCompute {
                    status: GetViewUrlStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            if input.content_id.is_empty() {
                updater.set(GetViewUrlCompute {
                    status: GetViewUrlStatus::Error("Content ID is required".to_owned()),
                });
                return;
            }

            updater.set(GetViewUrlCompute {
                status: GetViewUrlStatus::Loading,
            });

            let token = auth.token().unwrap_or_default();
            let url = format!(
                "{}/v1/contents/{}/view-url",
                config.api_url(),
                input.content_id
            );

            let disposition = if input.disposition.is_empty() {
                "inline".to_owned()
            } else {
                input.disposition.to_string()
            };

            let request = match Client::post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&ViewUrlRequest { disposition })
            {
                Ok(r) => r,
                Err(e) => {
                    updater.set(GetViewUrlCompute {
                        status: GetViewUrlStatus::Error(format!("Failed to build request: {}", e)),
                    });
                    return;
                }
            };

            let request = if let Some(cf) = cf_token.token() {
                request.header("cf-access-token", cf)
            } else {
                request
            };

            match request.send().await {
                Ok(response) => {
                    if response.is_success() {
                        match response.json::<ViewUrlData>() {
                            Ok(data) => {
                                updater.set(GetViewUrlCompute {
                                    status: GetViewUrlStatus::Success(data),
                                });
                            }
                            Err(e) => {
                                updater.set(GetViewUrlCompute {
                                    status: GetViewUrlStatus::Error(format!(
                                        "Failed to parse response: {}",
                                        e
                                    )),
                                });
                            }
                        }
                    } else if response.status == 404 {
                        updater.set(GetViewUrlCompute {
                            status: GetViewUrlStatus::NotFound,
                        });
                    } else {
                        let error = response
                            .text()
                            .unwrap_or_else(|_| "Unknown error".to_owned());
                        updater.set(GetViewUrlCompute {
                            status: GetViewUrlStatus::Error(error),
                        });
                    }
                }
                Err(e) => {
                    updater.set(GetViewUrlCompute {
                        status: GetViewUrlStatus::Error(e.to_string()),
                    });
                }
            }
        })
    }
}
