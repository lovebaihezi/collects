//! v1 API endpoints module.
//!
//! This module contains all v1 API handlers organized by domain:
//! - `contents` - Content management endpoints
//! - `content_tags` - Content-tag relationship endpoints
//! - `groups` - Group management endpoints
//! - `me` - Current user information
//! - `tags` - Tag management endpoints
//! - `types` - Shared types for API request/response
//! - `uploads` - File upload endpoints

pub mod content_tags;
pub mod contents;
pub mod groups;
pub mod me;
pub mod tags;
pub mod types;
pub mod uploads;

use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::storage::UserStorage;
use axum::{
    Router,
    routing::{delete, get, patch, post},
};

/// Creates the v1 API routes.
pub fn create_routes<S, U>() -> Router<AppState<S, U>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/me", get(me::v1_me::<S, U>))
        .route("/uploads/init", post(uploads::v1_uploads_init::<S, U>))
        .route(
            "/uploads/complete",
            post(uploads::v1_uploads_complete::<S, U>),
        )
        // Contents endpoints
        .route(
            "/contents",
            get(contents::v1_contents_list::<S, U>).post(contents::v1_contents_create::<S, U>),
        )
        .route("/contents/{id}", get(contents::v1_contents_get::<S, U>))
        .route(
            "/contents/{id}",
            patch(contents::v1_contents_update::<S, U>),
        )
        .route(
            "/contents/{id}/trash",
            post(contents::v1_contents_trash::<S, U>),
        )
        .route(
            "/contents/{id}/restore",
            post(contents::v1_contents_restore::<S, U>),
        )
        .route(
            "/contents/{id}/archive",
            post(contents::v1_contents_archive::<S, U>),
        )
        .route(
            "/contents/{id}/unarchive",
            post(contents::v1_contents_unarchive::<S, U>),
        )
        .route(
            "/contents/{id}/view-url",
            post(contents::v1_contents_view_url::<S, U>),
        )
        // Content-Tags endpoints
        .route(
            "/contents/{id}/tags",
            get(content_tags::v1_content_tags_list::<S, U>)
                .post(content_tags::v1_content_tags_attach::<S, U>),
        )
        .route(
            "/contents/{id}/tags/{tag_id}",
            delete(content_tags::v1_content_tags_detach::<S, U>),
        )
        // Tags endpoints
        .route(
            "/tags",
            get(tags::v1_tags_list::<S, U>).post(tags::v1_tags_create::<S, U>),
        )
        .route(
            "/tags/{id}",
            patch(tags::v1_tags_update::<S, U>).delete(tags::v1_tags_delete::<S, U>),
        )
        // Groups endpoints
        .route(
            "/groups",
            get(groups::v1_groups_list::<S, U>).post(groups::v1_groups_create::<S, U>),
        )
        .route(
            "/groups/{id}",
            get(groups::v1_groups_get::<S, U>).patch(groups::v1_groups_update::<S, U>),
        )
        .route("/groups/{id}/trash", post(groups::v1_groups_trash::<S, U>))
        .route(
            "/groups/{id}/restore",
            post(groups::v1_groups_restore::<S, U>),
        )
        .route(
            "/groups/{id}/archive",
            post(groups::v1_groups_archive::<S, U>),
        )
        .route(
            "/groups/{id}/unarchive",
            post(groups::v1_groups_unarchive::<S, U>),
        )
        .route(
            "/groups/{id}/contents",
            get(groups::v1_groups_contents_list::<S, U>)
                .post(groups::v1_groups_contents_add::<S, U>),
        )
        .route(
            "/groups/{id}/contents/{content_id}",
            delete(groups::v1_groups_contents_remove::<S, U>),
        )
        .route(
            "/groups/{id}/contents/reorder",
            patch(groups::v1_groups_contents_reorder::<S, U>),
        )
}
