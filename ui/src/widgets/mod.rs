//! UI widgets module.
//!
//! This module re-exports the top-level widgets used by the application.
//!
//! ## TODO(internal auth token / Zero Trust UX)
//! Internal builds call `/api/internal/*` endpoints which are protected by Cloudflare Zero Trust.
//! The services middleware accepts tokens from either `cf-authorization` (preferred) or
//! `Authorization` headers. On the UI/business side we standardized on `cf-authorization`.
//!
//! Today, the token is **manually set** (copy/paste) and stored in business state via
//! `CFTokenInput` -> `SetCFTokenCommand` -> `CFTokenCompute`. This is designed so we can
//! replace the acquisition mechanism later (PKCE) without changing every internal command.
//!
//! ### UX tasks (manual token)
//! - Add an "Internal Auth" section in the internal panel that lets you:
//!   - paste/set token (writes `CFTokenInput.token`, dispatches `SetCFTokenCommand`)
//!   - clear token (set empty/None, dispatch `SetCFTokenCommand`)
//!   - show status: token set/unset, and token length only (never render token value)
//! - Add a small indicator next to internal widgets (Users / Create User) showing whether
//!   internal calls are authorized (token present) vs likely to fail (token missing).
//! - Persist token securely:
//!   - Decide persistence strategy for desktop builds (e.g., OS keychain) vs web builds.
//!   - Avoid storing in plain-text persistence if the app uses egui persistence.
//!
//! ### Behavior tasks (request plumbing)
//! - Ensure all internal commands attach `cf-authorization` when `CFTokenCompute` is set.
//!   (Create user already does; list users / internal status may need the same treatment.)
//! - Add consistent error handling for 401/403:
//!   - Interpret as "missing/invalid token" and guide the user to set token.
//!
//! ### Future: PKCE integration (replace manual set)
//! - Replace `SetCFTokenCommand`'s input source with a PKCE flow:
//!   - obtain authorization code -> exchange for token -> store to `CFTokenCompute`
//! - Keep the same storage boundary (`CFTokenCompute`) so internal commands remain unchanged.
//! - Add token refresh/expiry handling if applicable:
//!   - extend `CFTokenResult` to include metadata (e.g. expires_at) and surface it in UI.
//!
//! ### Testing tasks
//! - UI integration tests should assert `cf-authorization` header is present when token is set.
//! - Services integration tests already validate Zero Trust gating; add an end-to-end internal
//!   flow test when we have a stable token acquisition story (manual or PKCE).
mod api_status;
mod footer;
mod image_diag;
mod image_preview;
mod internal;
mod login;

pub use internal::{InternalUsersState, UserAction, internal_users_panel};

pub use api_status::api_status;
pub use footer::powered_by_egui_and_eframe;
pub use image_diag::{ImageDiagAction, image_diag_window};
pub use image_preview::{ImagePreviewState, image_preview, image_preview_fullscreen};
pub use login::{login_widget, show_signed_in_header};
