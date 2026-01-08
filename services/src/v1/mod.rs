//! V1 API module containing all versioned API endpoints.
//!
//! This module organizes the V1 API into sub-modules by resource:
//! - `me` - User profile endpoint
//! - `uploads` - File upload endpoints
//! - `contents` - Content management endpoints
//! - `tags` - Tag management endpoints
//! - `groups` - Collection/group management endpoints
//! - `types` - Shared types (error responses, etc.)

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

/// Creates the V1 API router with all endpoints.
pub fn routes<S, U>() -> Router<AppState<S, U>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    Router::new()
        // Me endpoint
        .route("/me", get(me::handler::<S, U>))
        // Uploads endpoints
        .route("/uploads/init", post(uploads::init::<S, U>))
        // Contents endpoints
        .route("/contents", get(contents::list::<S, U>))
        .route("/contents/{id}", get(contents::get::<S, U>))
        .route("/contents/{id}", patch(contents::update::<S, U>))
        .route("/contents/{id}/trash", post(contents::trash::<S, U>))
        .route("/contents/{id}/restore", post(contents::restore::<S, U>))
        .route("/contents/{id}/archive", post(contents::archive::<S, U>))
        .route(
            "/contents/{id}/unarchive",
            post(contents::unarchive::<S, U>),
        )
        .route("/contents/{id}/view-url", post(uploads::view_url::<S, U>))
        // Content-Tags endpoints
        .route(
            "/contents/{id}/tags",
            get(tags::list_for_content::<S, U>).post(tags::attach::<S, U>),
        )
        .route("/contents/{id}/tags/{tag_id}", delete(tags::detach::<S, U>))
        // Tags endpoints
        .route("/tags", get(tags::list::<S, U>).post(tags::create::<S, U>))
        .route(
            "/tags/{id}",
            patch(tags::update::<S, U>).delete(tags::delete::<S, U>),
        )
        // Groups endpoints
        .route(
            "/groups",
            get(groups::list::<S, U>).post(groups::create::<S, U>),
        )
        .route(
            "/groups/{id}",
            get(groups::get::<S, U>).patch(groups::update::<S, U>),
        )
        .route("/groups/{id}/trash", post(groups::trash::<S, U>))
        .route("/groups/{id}/restore", post(groups::restore::<S, U>))
        .route("/groups/{id}/archive", post(groups::archive::<S, U>))
        .route("/groups/{id}/unarchive", post(groups::unarchive::<S, U>))
        .route(
            "/groups/{id}/contents",
            get(groups::contents_list::<S, U>).post(groups::contents_add::<S, U>),
        )
        .route(
            "/groups/{id}/contents/{content_id}",
            delete(groups::contents_remove::<S, U>),
        )
}
