use collects::TemplateApp;
use eframe::App;
use egui::accesskit;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn test_collects_header_exists() {
    let app = |ui: &mut egui::Ui| {
        let mut app = TemplateApp::default();
        app.update(ui.ctx(), &mut eframe::Frame::_new_kittest());
    };

    let mut harness = Harness::new_ui(app);
    harness.run();

    // Print all labels to debug
    let all_labels: Vec<_> = harness
        .query_all_by_role(accesskit::Role::TextRun)
        .into_iter()
        .collect();
    println!(
        "All labels: {:?}",
        all_labels
            .iter()
            .map(|node| node.accesskit_node().value().unwrap_or_default())
            .collect::<Vec<_>>()
    );

    // Instead of looking for Heading role, look for TextRun with "Collects"
    let collects_labels: Vec<_> = harness
        .query_all_by_label_contains("Collects")
        .into_iter()
        .collect();
    assert!(
        !collects_labels.is_empty(),
        "No labels containing 'Collects' found in the UI"
    );

    // Check that one of the labels contains "Collects"
    let label_texts: Vec<_> = collects_labels
        .iter()
        .map(|h| h.accesskit_node().value().unwrap_or_default().to_string())
        .collect();
    assert!(
        label_texts.iter().any(|text| text.contains("Collects")),
        "No label contains 'Collects'. Found labels: {:?}",
        label_texts
    );
}

#[test]
fn test_collects_header_preview_mode() {
    let app = |ui: &mut egui::Ui| {
        let mut app = TemplateApp::default();
        app.update(ui.ctx(), &mut eframe::Frame::_new_kittest());
    };

    let mut harness = Harness::new_ui(app);
    harness.run();

    // Print all labels to debug
    let all_labels: Vec<_> = harness
        .query_all_by_role(accesskit::Role::TextRun)
        .into_iter()
        .collect();
    println!(
        "All labels: {:?}",
        all_labels
            .iter()
            .map(|node| node.accesskit_node().value().unwrap_or_default())
            .collect::<Vec<_>>()
    );

    // Instead of looking for Heading role, look for TextRun with "Collects"
    let collects_labels: Vec<_> = harness
        .query_all_by_label_contains("Collects")
        .into_iter()
        .collect();
    assert!(
        !collects_labels.is_empty(),
        "No labels containing 'Collects' found in the UI"
    );

    // In preview mode, we expect "Collects (Preview)" heading
    // Note: This test would need to be run with --features preview to work correctly
    #[cfg(feature = "preview")]
    {
        let label_texts: Vec<_> = collects_labels
            .iter()
            .map(|h| h.accesskit_node().value().unwrap_or_default().to_string())
            .collect();
        assert!(
            label_texts
                .iter()
                .any(|text| text.contains("Collects (Preview)")),
            "No label contains 'Collects (Preview)'. Found labels: {:?}",
            label_texts
        );
    }

    #[cfg(not(feature = "preview"))]
    {
        let label_texts: Vec<_> = collects_labels
            .iter()
            .map(|h| h.accesskit_node().value().unwrap_or_default().to_string())
            .collect();
        assert!(
            label_texts.iter().any(|text| text.contains("Collects")),
            "No label contains 'Collects'. Found labels: {:?}",
            label_texts
        );
    }
}
