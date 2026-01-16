//! List and get groups (collects) business operations.
//!
//! Provides commands and computes for listing user's groups and getting contents within a group.
//!
//! # Terminology
//! - "Group" in the API/database = "Collect" in user terminology
//! - Users want to see their "collects" which are Groups containing multiple content items

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
// Group Item (user-facing: "Collect")
// ============================================================================

/// A group (collect) item from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupItem {
    /// Unique identifier (UUID).
    pub id: Ustr,
    /// Group name.
    pub name: Ustr,
    /// Optional description.
    #[serde(default)]
    pub description: Option<Ustr>,
    /// Group visibility (private, public, restricted).
    pub visibility: Ustr,
    /// Group status (active, archived, trashed).
    pub status: Ustr,
    /// Timestamp when group was trashed (ISO 8601 format).
    #[serde(default)]
    pub trashed_at: Option<Ustr>,
    /// Timestamp when group was archived (ISO 8601 format).
    #[serde(default)]
    pub archived_at: Option<Ustr>,
    /// Timestamp when group was created (ISO 8601 format).
    pub created_at: Ustr,
    /// Timestamp when group was last updated (ISO 8601 format).
    pub updated_at: Ustr,
}

/// Response from listing groups.
#[derive(Debug, Clone, Deserialize)]
pub struct ListGroupsResponse {
    /// List of group items.
    pub items: Vec<GroupItem>,
    /// Total number of items returned.
    pub total: usize,
}

// ============================================================================
// Group Content Item (files attached to a collect)
// ============================================================================

/// A content item within a group (junction table data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupContentItem {
    /// Junction table row ID.
    pub id: Ustr,
    /// Group ID this content belongs to.
    pub group_id: Ustr,
    /// Content ID.
    pub content_id: Ustr,
    /// Sort order within the group.
    pub sort_order: i32,
    /// Timestamp when content was added to the group.
    pub added_at: Ustr,
}

/// Response from listing group contents.
#[derive(Debug, Clone, Deserialize)]
pub struct ListGroupContentsResponse {
    /// List of content items in the group.
    pub items: Vec<GroupContentItem>,
    /// Total number of items.
    pub total: usize,
}

// ============================================================================
// List Groups
// ============================================================================

/// Input parameters for listing groups.
#[derive(Default, Debug, Clone)]
pub struct ListGroupsInput {
    /// Maximum number of items to return (1-100, default 50).
    pub limit: Option<i32>,
    /// Offset for pagination.
    pub offset: Option<i32>,
    /// Filter by status: "active", "archived", "trashed".
    pub status: Option<Ustr>,
}

impl SnapshotClone for ListGroupsInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for ListGroupsInput {
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

/// Status of the list groups operation.
#[derive(Debug, Clone, Default)]
pub enum ListGroupsStatus {
    #[default]
    Idle,
    Loading,
    Success(Vec<GroupItem>),
    Error(String),
}

/// Compute to track list groups status.
#[derive(Default, Debug, Clone)]
pub struct ListGroupsCompute {
    pub status: ListGroupsStatus,
}

impl SnapshotClone for ListGroupsCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for ListGroupsCompute {
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

impl State for ListGroupsCompute {
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

/// Command to list groups (collects) for the authenticated user.
#[derive(Default, Debug)]
pub struct ListGroupsCommand;

impl Command for ListGroupsCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: ListGroupsInput = snap.state::<ListGroupsInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            if !auth.is_authenticated() {
                updater.set(ListGroupsCompute {
                    status: ListGroupsStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            updater.set(ListGroupsCompute {
                status: ListGroupsStatus::Loading,
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
                format!("{}/v1/groups", config.api_url())
            } else {
                format!("{}/v1/groups?{}", config.api_url(), params.join("&"))
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
                        match response.json::<ListGroupsResponse>() {
                            Ok(resp) => {
                                updater.set(ListGroupsCompute {
                                    status: ListGroupsStatus::Success(resp.items),
                                });
                            }
                            Err(e) => {
                                updater.set(ListGroupsCompute {
                                    status: ListGroupsStatus::Error(format!(
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
                        updater.set(ListGroupsCompute {
                            status: ListGroupsStatus::Error(error),
                        });
                    }
                }
                Err(e) => {
                    updater.set(ListGroupsCompute {
                        status: ListGroupsStatus::Error(e.to_string()),
                    });
                }
            }
        })
    }
}

// ============================================================================
// Get Group Contents (files in a collect)
// ============================================================================

/// Input for getting contents within a group.
#[derive(Default, Debug, Clone)]
pub struct GetGroupContentsInput {
    /// Group ID to get contents for.
    pub group_id: Option<Ustr>,
}

impl SnapshotClone for GetGroupContentsInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for GetGroupContentsInput {
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

/// Status of get group contents operation.
#[derive(Debug, Clone, Default)]
pub enum GetGroupContentsStatus {
    #[default]
    Idle,
    Loading,
    Success(Vec<GroupContentItem>),
    NotFound,
    Error(String),
}

/// Compute to track get group contents status.
#[derive(Default, Debug, Clone)]
pub struct GetGroupContentsCompute {
    pub status: GetGroupContentsStatus,
}

impl SnapshotClone for GetGroupContentsCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for GetGroupContentsCompute {
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

impl State for GetGroupContentsCompute {
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

/// Command to get contents within a group (files in a collect).
#[derive(Default, Debug)]
pub struct GetGroupContentsCommand;

impl Command for GetGroupContentsCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: GetGroupContentsInput = snap.state::<GetGroupContentsInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            let group_id = match &input.group_id {
                Some(id) => id.as_str(),
                None => {
                    updater.set(GetGroupContentsCompute {
                        status: GetGroupContentsStatus::Error("No group ID provided".to_owned()),
                    });
                    return;
                }
            };

            if !auth.is_authenticated() {
                updater.set(GetGroupContentsCompute {
                    status: GetGroupContentsStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            updater.set(GetGroupContentsCompute {
                status: GetGroupContentsStatus::Loading,
            });

            let token = auth.token().unwrap_or_default();
            let url = format!("{}/v1/groups/{}/contents", config.api_url(), group_id);

            let request = Client::get(&url).header("Authorization", format!("Bearer {token}"));

            let request = if let Some(cf) = cf_token.token() {
                request.header("cf-access-token", cf)
            } else {
                request
            };

            match request.send().await {
                Ok(response) => {
                    if response.is_success() {
                        match response.json::<ListGroupContentsResponse>() {
                            Ok(resp) => {
                                updater.set(GetGroupContentsCompute {
                                    status: GetGroupContentsStatus::Success(resp.items),
                                });
                            }
                            Err(e) => {
                                updater.set(GetGroupContentsCompute {
                                    status: GetGroupContentsStatus::Error(format!(
                                        "Failed to parse response: {}",
                                        e
                                    )),
                                });
                            }
                        }
                    } else if response.status == 404 {
                        updater.set(GetGroupContentsCompute {
                            status: GetGroupContentsStatus::NotFound,
                        });
                    } else {
                        let error = response
                            .text()
                            .unwrap_or_else(|_| "Unknown error".to_owned());
                        updater.set(GetGroupContentsCompute {
                            status: GetGroupContentsStatus::Error(error),
                        });
                    }
                }
                Err(e) => {
                    updater.set(GetGroupContentsCompute {
                        status: GetGroupContentsStatus::Error(e.to_string()),
                    });
                }
            }
        })
    }
}

// ============================================================================
// Create Group (Collect)
// ============================================================================

/// Input for creating a new group (collect).
#[derive(Default, Debug, Clone)]
pub struct CreateGroupInput {
    /// Group name.
    pub name: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Group visibility (private, public).
    pub visibility: Option<String>,
}

impl SnapshotClone for CreateGroupInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for CreateGroupInput {
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

/// Status of create group operation.
#[derive(Debug, Clone, Default)]
pub enum CreateGroupStatus {
    #[default]
    Idle,
    Creating,
    Success(GroupItem),
    Error(String),
}

/// Compute to track create group status.
#[derive(Default, Debug, Clone)]
pub struct CreateGroupCompute {
    pub status: CreateGroupStatus,
}

impl SnapshotClone for CreateGroupCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for CreateGroupCompute {
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

impl State for CreateGroupCompute {
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

#[derive(Serialize)]
struct CreateGroupRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub visibility: String,
}

/// Command to create a new group (collect).
#[derive(Default, Debug)]
pub struct CreateGroupCommand;

impl Command for CreateGroupCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: CreateGroupInput = snap.state::<CreateGroupInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            if !auth.is_authenticated() {
                updater.set(CreateGroupCompute {
                    status: CreateGroupStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            let name = match input.name {
                Some(name) => name.trim().to_owned(),
                None => {
                    updater.set(CreateGroupCompute {
                        status: CreateGroupStatus::Error("No group name provided".to_owned()),
                    });
                    return;
                }
            };

            if name.is_empty() {
                updater.set(CreateGroupCompute {
                    status: CreateGroupStatus::Error("Group name cannot be empty".to_owned()),
                });
                return;
            }

            updater.set(CreateGroupCompute {
                status: CreateGroupStatus::Creating,
            });

            let token = auth.token().unwrap_or_default();
            let visibility = input.visibility.unwrap_or_else(|| "private".to_owned());

            let url = format!("{}/v1/groups", config.api_url());
            let request = match Client::post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .json(&CreateGroupRequest {
                    name,
                    description: input.description,
                    visibility,
                }) {
                Ok(r) => r,
                Err(e) => {
                    updater.set(CreateGroupCompute {
                        status: CreateGroupStatus::Error(format!("Failed to build request: {}", e)),
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
                        match response.json::<GroupItem>() {
                            Ok(group) => {
                                updater.set(CreateGroupCompute {
                                    status: CreateGroupStatus::Success(group),
                                });
                            }
                            Err(e) => {
                                updater.set(CreateGroupCompute {
                                    status: CreateGroupStatus::Error(format!(
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
                        updater.set(CreateGroupCompute {
                            status: CreateGroupStatus::Error(error),
                        });
                    }
                }
                Err(e) => {
                    updater.set(CreateGroupCompute {
                        status: CreateGroupStatus::Error(e.to_string()),
                    });
                }
            }
        })
    }
}

// ============================================================================
// Add Contents to Group
// ============================================================================

/// Input for adding contents to a group.
#[derive(Default, Debug, Clone)]
pub struct AddGroupContentsInput {
    /// Group ID to add contents to.
    pub group_id: Option<Ustr>,
    /// Content IDs to add.
    pub content_ids: Vec<Ustr>,
}

impl SnapshotClone for AddGroupContentsInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for AddGroupContentsInput {
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

/// Status of add group contents operation.
#[derive(Debug, Clone, Default)]
pub enum AddGroupContentsStatus {
    #[default]
    Idle,
    Adding,
    Success {
        added: usize,
    },
    Error(String),
}

/// Compute to track add group contents status.
#[derive(Default, Debug, Clone)]
pub struct AddGroupContentsCompute {
    pub status: AddGroupContentsStatus,
}

impl SnapshotClone for AddGroupContentsCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for AddGroupContentsCompute {
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

impl State for AddGroupContentsCompute {
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

#[derive(Serialize)]
struct AddGroupContentRequest {
    pub content_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<i32>,
}

/// Command to add contents to a group.
#[derive(Default, Debug)]
pub struct AddGroupContentsCommand;

impl Command for AddGroupContentsCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: AddGroupContentsInput = snap.state::<AddGroupContentsInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            let group_id = match &input.group_id {
                Some(id) => id.as_str().to_owned(),
                None => {
                    updater.set(AddGroupContentsCompute {
                        status: AddGroupContentsStatus::Error("No group ID provided".to_owned()),
                    });
                    return;
                }
            };

            if input.content_ids.is_empty() {
                updater.set(AddGroupContentsCompute {
                    status: AddGroupContentsStatus::Error("No content IDs provided".to_owned()),
                });
                return;
            }

            if !auth.is_authenticated() {
                updater.set(AddGroupContentsCompute {
                    status: AddGroupContentsStatus::Error("Not authenticated".to_owned()),
                });
                return;
            }

            updater.set(AddGroupContentsCompute {
                status: AddGroupContentsStatus::Adding,
            });

            let token = auth.token().unwrap_or_default();
            let mut added = 0usize;

            for content_id in input.content_ids {
                let url = format!("{}/v1/groups/{}/contents", config.api_url(), group_id);
                let request = match Client::post(&url)
                    .header("Authorization", format!("Bearer {token}"))
                    .json(&AddGroupContentRequest {
                        content_id: content_id.to_string(),
                        sort_order: None,
                    }) {
                    Ok(r) => r,
                    Err(e) => {
                        updater.set(AddGroupContentsCompute {
                            status: AddGroupContentsStatus::Error(format!(
                                "Failed to build request: {}",
                                e
                            )),
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
                            added += 1;
                        } else {
                            let error = response
                                .text()
                                .unwrap_or_else(|_| "Unknown error".to_owned());
                            updater.set(AddGroupContentsCompute {
                                status: AddGroupContentsStatus::Error(format!(
                                    "Failed to add content {}: {}",
                                    content_id, error
                                )),
                            });
                            return;
                        }
                    }
                    Err(e) => {
                        updater.set(AddGroupContentsCompute {
                            status: AddGroupContentsStatus::Error(e.to_string()),
                        });
                        return;
                    }
                }
            }

            updater.set(AddGroupContentsCompute {
                status: AddGroupContentsStatus::Success { added },
            });
        })
    }
}
