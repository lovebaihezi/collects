use std::any::Any;

use serde::{Deserialize, Serialize};

use crate::http::Client;
use crate::BusinessConfig;
use crate::{
    cf_token_compute::CFTokenCompute,
    login_state::AuthCompute,
};
use collects_states::{
    assign_impl, state_assign_impl, Command, CommandSnapshot, Compute, ComputeDeps, Dep,
    LatestOnlyUpdater, SnapshotClone, State, Updater,
};

/// Attachment data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Input state for content creation.
#[derive(Default, Debug, Clone)]
pub struct CreateContentInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub body: Option<String>,
    pub attachments: Vec<Attachment>,
}

impl SnapshotClone for CreateContentInput {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl State for CreateContentInput {
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

/// Status of the content creation operation.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ContentCreationStatus {
    #[default]
    Idle,
    Uploading,
    Success(Vec<String>), // List of created content IDs or titles
    Error(String),
}

/// Compute to track content creation status.
#[derive(Default, Debug, Clone)]
pub struct CreateContentCompute {
    pub status: ContentCreationStatus,
}

impl SnapshotClone for CreateContentCompute {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))
    }
}

impl Compute for CreateContentCompute {
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

impl State for CreateContentCompute {
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

// Internal types for API requests
#[derive(Serialize)]
struct CreateContentRequest {
    pub title: String,
    pub description: Option<String>,
    pub visibility: String,
    pub content_type: String,
    pub body: String,
}

#[derive(Serialize)]
struct InitUploadRequest {
    pub filename: String,
    pub content_type: String,
    pub file_size: usize,
}

#[derive(Deserialize)]
struct InitUploadResponse {
    pub upload_id: String,
    pub upload_url: String,
    // other fields ignored
}

#[derive(Serialize)]
struct CompleteUploadRequest {
    pub upload_id: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct ContentItem {
    pub id: String,
    // other fields ignored
}

#[derive(Deserialize)]
struct CreateContentResponse {
    pub content: ContentItem,
}

#[derive(Deserialize)]
struct CompleteUploadResponse {
    pub content: ContentItem,
}

/// Command to create content (inline text or attachments).
#[derive(Default, Debug)]
pub struct CreateContentCommand;

impl Command for CreateContentCommand {
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: LatestOnlyUpdater,
        _cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        let input: CreateContentInput = snap.state::<CreateContentInput>().clone();
        let config: BusinessConfig = snap.state::<BusinessConfig>().clone();
        let auth: AuthCompute = snap.compute::<AuthCompute>().clone();
        let cf_token: CFTokenCompute = snap.compute::<CFTokenCompute>().clone();

        Box::pin(async move {
            if !auth.is_authenticated() {
                updater.set(CreateContentCompute {
                    status: ContentCreationStatus::Error("Not authenticated".to_string()),
                });
                return;
            }

            updater.set(CreateContentCompute {
                status: ContentCreationStatus::Uploading,
            });

            let token = auth.token().unwrap_or_default();
            let mut created_ids = Vec::new();

            // 1. Handle Text Body
            if let Some(body) = input.body {
                if !body.trim().is_empty() {
                    const MAX_INLINE_SIZE: usize = 64 * 1024;
                    if body.len() > MAX_INLINE_SIZE {
                        // Upload as text file
                        match upload_file(
                            &config,
                            token,
                            &cf_token,
                            "note.txt",
                            "text/plain",
                            body.into_bytes(),
                            input.title.clone(),
                            input.description.clone(),
                        )
                        .await
                        {
                            Ok(id) => created_ids.push(id),
                            Err(e) => {
                                updater.set(CreateContentCompute {
                                    status: ContentCreationStatus::Error(e),
                                });
                                return;
                            }
                        }
                    } else {
                        // Create inline content
                        match create_inline_content(
                            &config,
                            token,
                            &cf_token,
                            input.title.unwrap_or_else(|| "Note".to_string()),
                            input.description.clone(),
                            body,
                        )
                        .await
                        {
                            Ok(id) => created_ids.push(id),
                            Err(e) => {
                                updater.set(CreateContentCompute {
                                    status: ContentCreationStatus::Error(e),
                                });
                                return;
                            }
                        }
                    }
                }
            }

            // 2. Handle Attachments
            for attachment in input.attachments {
                match upload_file(
                    &config,
                    token,
                    &cf_token,
                    &attachment.filename,
                    &attachment.mime_type,
                    attachment.data,
                    None, // Use filename as title or default
                    None,
                )
                .await
                {
                    Ok(id) => created_ids.push(id),
                    Err(e) => {
                        updater.set(CreateContentCompute {
                            status: ContentCreationStatus::Error(format!(
                                "Failed to upload {}: {}",
                                attachment.filename, e
                            )),
                        });
                        return;
                    }
                }
            }

            updater.set(CreateContentCompute {
                status: ContentCreationStatus::Success(created_ids),
            });
        })
    }
}

async fn create_inline_content(
    config: &BusinessConfig,
    token: &str,
    cf_token: &CFTokenCompute,
    title: String,
    description: Option<String>,
    body: String,
) -> Result<String, String> {
    let url = format!("{}/v1/contents", config.api_url());
    let request = Client::post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&CreateContentRequest {
            title,
            description,
            visibility: "private".to_string(),
            content_type: "text/plain".to_string(),
            body,
        })
        .map_err(|e| e.to_string())?;

    let request = if let Some(cf) = cf_token.token() {
        request.header("cf-access-token", cf)
    } else {
        request
    };

    let response = request.send().await.map_err(|e| e.to_string())?;

    if response.is_success() {
        let resp: CreateContentResponse = response.json().map_err(|e| e.to_string())?;
        Ok(resp.content.id)
    } else {
        Err(response.text().unwrap_or_default())
    }
}

async fn upload_file(
    config: &BusinessConfig,
    token: &str,
    cf_token: &CFTokenCompute,
    filename: &str,
    content_type: &str,
    data: Vec<u8>,
    title: Option<String>,
    description: Option<String>,
) -> Result<String, String> {
    // 1. Init Upload
    let url = format!("{}/v1/uploads/init", config.api_url());
    let request = Client::post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&InitUploadRequest {
            filename: filename.to_string(),
            content_type: content_type.to_string(),
            file_size: data.len(),
        })
        .map_err(|e| e.to_string())?;

    let request = if let Some(cf) = cf_token.token() {
        request.header("cf-access-token", cf)
    } else {
        request
    };

    let response = request.send().await.map_err(|e| e.to_string())?;

    if !response.is_success() {
        return Err(format!("Init upload failed: {}", response.text().unwrap_or_default()));
    }

    let init_resp: InitUploadResponse = response.json().map_err(|e| e.to_string())?;

    // 2. Upload to R2 (Direct PUT)
    // Note: This URL is presigned, so we don't need auth headers for THIS request,
    // but the Client might need to be careful if it adds them automatically.
    // Our Client adds headers explicitly, so it's fine.
    let upload_request = Client::put(&init_resp.upload_url)
        .header("Content-Type", content_type)
        .body(data);

    let upload_response = upload_request.send().await.map_err(|e| e.to_string())?;

    if !upload_response.is_success() {
        return Err(format!("Upload to storage failed: status {}", upload_response.status));
    }

    // 3. Complete Upload
    let complete_url = format!("{}/v1/uploads/complete", config.api_url());
    let request = Client::post(&complete_url)
        .header("Authorization", format!("Bearer {}", token))
        .json(&CompleteUploadRequest {
            upload_id: init_resp.upload_id,
            title,
            description,
        })
        .map_err(|e| e.to_string())?;

    let request = if let Some(cf) = cf_token.token() {
        request.header("cf-access-token", cf)
    } else {
        request
    };

    let complete_response = request.send().await.map_err(|e| e.to_string())?;

    if complete_response.is_success() {
        let resp: CompleteUploadResponse = complete_response.json().map_err(|e| e.to_string())?;
        Ok(resp.content.id)
    } else {
        Err(complete_response.text().unwrap_or_default())
    }
}
