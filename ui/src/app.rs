use crate::{pages, state::State, utils::clipboard, widgets};
use chrono::{Timelike, Utc};
use collects_business::{ApiStatus, AuthCompute, Route, ToggleApiStatusCommand};
use collects_states::Time;

/// Trait for handling paste operations, enabling mock implementations for testing.
///
/// This trait abstracts the paste shortcut detection and clipboard access,
/// allowing tests to inject mock clipboard providers.
pub trait PasteHandler {
    /// Handle paste shortcut and return clipboard image if available.
    fn handle_paste(&self, ctx: &egui::Context) -> Option<clipboard::ClipboardImage>;
}

/// Default paste handler using the system clipboard.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct SystemPasteHandler;

#[cfg(not(target_arch = "wasm32"))]
impl PasteHandler for SystemPasteHandler {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<clipboard::ClipboardImage> {
        clipboard::handle_paste_shortcut(ctx)
    }
}

/// Generic paste handler that wraps any ClipboardProvider for testing.
#[cfg(not(target_arch = "wasm32"))]
pub struct GenericPasteHandler<C: clipboard::ClipboardProvider> {
    clipboard: C,
}

#[cfg(not(target_arch = "wasm32"))]
impl<C: clipboard::ClipboardProvider> GenericPasteHandler<C> {
    /// Create a new paste handler with the given clipboard provider.
    pub fn new(clipboard: C) -> Self {
        Self { clipboard }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<C: clipboard::ClipboardProvider> PasteHandler for GenericPasteHandler<C> {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<clipboard::ClipboardImage> {
        clipboard::handle_paste_shortcut_with_clipboard(ctx, &self.clipboard)
    }
}

/// Stub paste handler for WASM target.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct SystemPasteHandler;

#[cfg(target_arch = "wasm32")]
impl PasteHandler for SystemPasteHandler {
    fn handle_paste(&self, _ctx: &egui::Context) -> Option<clipboard::ClipboardImage> {
        None
    }
}

/// Main application state and logic for the Collects app.
pub struct CollectsApp<P: PasteHandler = SystemPasteHandler> {
    /// The application state (public for testing access).
    pub state: State,
    paste_handler: P,
}

impl CollectsApp<SystemPasteHandler> {
    /// Called once before the first frame.
    pub fn new(state: State) -> Self {
        Self {
            state,
            paste_handler: SystemPasteHandler,
        }
    }
}

impl<P: PasteHandler> CollectsApp<P> {
    /// Create a new app with a custom paste handler (for testing).
    pub fn with_paste_handler(state: State, paste_handler: P) -> Self {
        Self {
            state,
            paste_handler,
        }
    }
}

impl<P: PasteHandler> eframe::App for CollectsApp<P> {
    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle paste shortcut (Ctrl+V / Cmd+V) for clipboard image
        // If an image was pasted, replace the current displayed image
        if let Some(clipboard_image) = self.paste_handler.handle_paste(ctx) {
            let image_state = self.state.ctx.state_mut::<widgets::ImagePreviewState>();
            image_state.set_image_rgba(
                ctx,
                clipboard_image.width,
                clipboard_image.height,
                clipboard_image.bytes,
            );
        }

        // Toggle API status display when F1 is pressed
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.state.ctx.dispatch::<ToggleApiStatusCommand>();
        }

        // Update Time state when second changes (chrono::Utc::now() is WASM-compatible)
        // This enables real-time updates for OTP countdown timers while avoiding
        // updates on every frame. Time-dependent computes (ApiStatus, InternalApiStatus)
        // have internal throttling to avoid unnecessary network requests.
        let now = Utc::now();
        let current_time = self.state.ctx.state_mut::<Time>();
        let current_second = current_time.as_ref().second();
        let new_second = now.second();
        if current_second != new_second {
            self.state.ctx.update::<Time>(|t| {
                *t.as_mut() = now;
            });
        }

        // Sync Compute for render
        self.state.ctx.sync_computes();

        // Poll for async responses (internal builds only)
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        widgets::poll_internal_users_responses(&mut self.state.ctx, ctx);

        // Update route based on authentication state
        self.update_route();

        // Show top panel with API status only when F1 is pressed (toggled)
        let show_api_status = self
            .state
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(false);
        if show_api_status {
            egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    // Label for accessibility and kittest queries
                    ui.label("API Status");
                    // API status dots (includes internal API for internal builds)
                    widgets::api_status(&self.state.ctx, ui);
                });
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Render the appropriate page based on current route
            self.render_page(ui);
        });

        // Run background jobs
        self.state.ctx.run_all_dirty();
    }
}

impl<P: PasteHandler> CollectsApp<P> {
    /// Updates the route based on authentication state.
    fn update_route(&mut self) {
        let is_authenticated = self
            .state
            .ctx
            .cached::<AuthCompute>()
            .is_some_and(|c| c.is_authenticated());

        let current_route = self.state.ctx.state_mut::<Route>().clone();

        let new_route = if is_authenticated {
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            {
                Route::Internal
            }
            #[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
            {
                Route::Home
            }
        } else {
            Route::Login
        };

        if current_route != new_route {
            self.state.ctx.update::<Route>(|route| {
                *route = new_route;
            });
        }
    }

    /// Renders the appropriate page based on the current route.
    fn render_page(&mut self, ui: &mut egui::Ui) {
        let route = self.state.ctx.state_mut::<Route>().clone();

        match route {
            Route::Login => {
                pages::login_page(&mut self.state, ui);
            }
            Route::Home => {
                pages::home_page(&mut self.state, ui);
            }
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            Route::Internal => {
                pages::internal_page(&mut self.state, ui);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock paste handler that always returns None (no image in clipboard)
    struct MockPasteHandlerEmpty;

    impl PasteHandler for MockPasteHandlerEmpty {
        fn handle_paste(&self, _ctx: &egui::Context) -> Option<clipboard::ClipboardImage> {
            None
        }
    }

    /// Mock paste handler that returns a predefined image
    struct MockPasteHandlerWithImage {
        image: clipboard::ClipboardImage,
    }

    impl PasteHandler for MockPasteHandlerWithImage {
        fn handle_paste(&self, _ctx: &egui::Context) -> Option<clipboard::ClipboardImage> {
            Some(self.image.clone())
        }
    }

    #[test]
    fn test_app_with_mock_paste_handler_empty() {
        // This test demonstrates how to create an app with a mock paste handler
        // that returns no image, for testing paste behavior without system clipboard
        let state = State::default();
        let _app = CollectsApp::with_paste_handler(state, MockPasteHandlerEmpty);
        // The app can now be tested without relying on system clipboard
    }

    #[test]
    fn test_app_with_mock_paste_handler_with_image() {
        // This test demonstrates how to create an app with a mock paste handler
        // that returns a predefined image, for testing image paste functionality
        let state = State::default();
        let mock = MockPasteHandlerWithImage {
            image: clipboard::ClipboardImage {
                width: 100,
                height: 100,
                bytes: vec![255u8; 100 * 100 * 4],
            },
        };
        let _app = CollectsApp::with_paste_handler(state, mock);
        // The app can now be tested with predictable image paste behavior
    }

    #[test]
    fn test_generic_paste_handler_with_mock_clipboard() {
        // Test that GenericPasteHandler correctly wraps a ClipboardProvider
        use crate::utils::clipboard::{ClipboardError, ClipboardImage, ClipboardProvider};

        struct MockClipboard {
            image: Option<ClipboardImage>,
        }

        impl ClipboardProvider for MockClipboard {
            fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
                Ok(self.image.clone())
            }
        }

        let mock_clipboard = MockClipboard {
            image: Some(ClipboardImage {
                width: 50,
                height: 50,
                bytes: vec![128u8; 50 * 50 * 4],
            }),
        };

        let paste_handler = GenericPasteHandler::new(mock_clipboard);
        let ctx = egui::Context::default();

        // The handler won't return an image because no paste key event occurred,
        // but this verifies the generic type composition works correctly
        let result = paste_handler.handle_paste(&ctx);
        assert!(result.is_none()); // No key event, so no paste triggered
    }
}
