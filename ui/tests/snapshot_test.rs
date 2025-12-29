use crate::common::TestCtx;

mod common;

/// Check if a GPU adapter is available for rendering.
/// Returns true if a GPU (including software renderer) is available.
fn is_gpu_available() -> bool {
    // Try to create a wgpu instance and check for adapters
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    // Check if any adapter is available
    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).is_ok()
}

/// Snapshot test for the main application UI.
///
/// This test captures the application UI to a PNG image for visual regression testing.
/// The snapshot is saved to `tests/snapshots/app_ui.png`.
///
/// **Note**: This test requires a GPU or software renderer (e.g., lavapipe/llvmpipe).
/// In CI environments without GPU support, this test will be skipped.
///
/// To update the snapshot locally, run:
/// `UPDATE_SNAPSHOTS=1 cargo test test_app_ui_snapshot --test snapshot_test`
///
/// To install a software renderer on Linux:
/// `sudo apt-get install mesa-vulkan-drivers`
#[tokio::test]
async fn test_app_ui_snapshot() {
    // Skip test if no GPU adapter is available
    if !is_gpu_available() {
        eprintln!(
            "Skipping snapshot test: No GPU adapter found. \
             Install mesa-vulkan-drivers for software rendering support."
        );
        return;
    }

    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Run multiple steps to ensure the initial UI is fully rendered
    for _ in 0..5 {
        harness.step();
    }

    // Try to capture the UI snapshot
    match harness.try_snapshot("app_ui") {
        Ok(()) => {
            // Snapshot captured successfully
        }
        Err(egui_kittest::SnapshotError::RenderError { err }) => {
            if err.contains("No adapter found") {
                eprintln!(
                    "Skipping snapshot test: No GPU adapter found. \
                     Install mesa-vulkan-drivers for software rendering support."
                );
                return;
            }
            panic!("Snapshot render error: {err}");
        }
        Err(err) => {
            panic!("Snapshot error: {err}");
        }
    }
}
