//! Image diagnostics window widget.
//!
//! This module provides a diagnostic window for debugging image paste and
//! drag-and-drop functionality across different environments.

use collects_business::{ClearImageDiagCommand, ImageDiagState, ImageEventStatus};
use collects_states::StateCtx;
use egui::{Color32, RichText, ScrollArea, Ui};

use crate::utils::colors::{COLOR_AMBER, COLOR_GREEN, COLOR_RED};

/// Renders the image diagnostics window content.
///
/// This widget displays a history of paste and drop events with their
/// status, dimensions, and any error information.
///
/// # Arguments
///
/// * `state_ctx` - The state context for reading diagnostic state
/// * `ui` - The egui UI to render into
pub fn image_diag_window(state_ctx: &mut StateCtx, ui: &mut Ui) {
    let diag = state_ctx.cached::<ImageDiagState>();
    let events = diag
        .as_ref()
        .map(|d| d.events().to_vec())
        .unwrap_or_default();
    let has_events = !events.is_empty();

    ui.horizontal(|ui| {
        ui.heading("Image Event History");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear").clicked() {
                state_ctx.dispatch::<ClearImageDiagCommand>();
            }
        });
    });

    ui.separator();

    if !has_events {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(RichText::new("No events recorded").color(Color32::GRAY));
            ui.add_space(8.0);
            ui.label(RichText::new("Try pasting (Ctrl+V) or dropping an image").size(12.0));
            ui.add_space(20.0);
        });
    } else {
        ScrollArea::vertical()
            .max_height(300.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for event in &events {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            // Event type indicator
                            let type_color = match event.event_type {
                                collects_business::ImageEventType::Paste => COLOR_GREEN,
                                collects_business::ImageEventType::Drop => COLOR_AMBER,
                            };
                            ui.label(
                                RichText::new(format!("[{}]", event.event_type))
                                    .color(type_color)
                                    .strong(),
                            );

                            // Timestamp
                            ui.label(
                                RichText::new(event.timestamp.format("%H:%M:%S").to_string())
                                    .size(11.0)
                                    .color(Color32::GRAY),
                            );
                        });

                        // Status
                        let status_color = match &event.status {
                            ImageEventStatus::Success => COLOR_GREEN,
                            ImageEventStatus::Failed(_) => COLOR_RED,
                        };
                        ui.label(RichText::new(format!("{}", event.status)).color(status_color));

                        // Dimensions (if available)
                        if let (Some(w), Some(h)) = (event.width, event.height) {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Size:").size(11.0));
                                ui.label(RichText::new(format!("{}×{}", w, h)).size(11.0));
                                if let Some(bytes) = event.bytes {
                                    ui.label(
                                        RichText::new(format!("({} bytes)", bytes))
                                            .size(11.0)
                                            .color(Color32::GRAY),
                                    );
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);
                }
            });
    }

    ui.separator();

    // Footer with instructions
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Press F2 to close")
                .size(11.0)
                .color(Color32::GRAY),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use kittest::Queryable;

    #[test]
    fn test_image_diag_window_renders_empty() {
        use collects_states::StateCtx;

        let mut ctx = StateCtx::new();
        ctx.record_compute(ImageDiagState::default());

        let harness = Harness::new_ui_state(
            |ui, ctx: &mut StateCtx| {
                image_diag_window(ctx, ui);
            },
            ctx,
        );

        // Should show the empty state message
        assert!(
            harness.query_by_label_contains("No events").is_some(),
            "Should show 'No events' when history is empty"
        );
    }

    #[test]
    fn test_image_diag_window_shows_heading() {
        use collects_states::StateCtx;

        let mut ctx = StateCtx::new();
        ctx.record_compute(ImageDiagState::default());

        let harness = Harness::new_ui_state(
            |ui, ctx: &mut StateCtx| {
                image_diag_window(ctx, ui);
            },
            ctx,
        );

        // Should show the heading
        assert!(
            harness
                .query_by_label_contains("Image Event History")
                .is_some(),
            "Should show 'Image Event History' heading"
        );
    }
}
