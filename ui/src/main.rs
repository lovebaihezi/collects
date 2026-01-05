#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use collects_ui::state::State;
use collects_ui::utils::fonts::add_font;

#[cfg(not(target_arch = "wasm32"))]
mod alloc {
    #[global_allocator]
    static MALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    use std::fs;

    // Log to stderr (if you run with `RUST_LOG=debug`).
    // Filter out egui_winit clipboard errors - they occur when clipboard content
    // is not in a supported text format (e.g., when copying images from browser)
    env_logger::Builder::from_env(env_logger::Env::default())
        .filter_module("egui_winit::clipboard", log::LevelFilter::Off)
        .init();

    let native_options = eframe::NativeOptions {
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_min_inner_size([300.0, 220.0])
            .with_drag_and_drop(true)
            .with_icon(
                // Icon is generated at build time based on environment features
                // (original for prod, grayscale for non-prod, inverted grayscale for internal)
                eframe::icon_data::from_png_bytes(include_bytes!(concat!(
                    env!("OUT_DIR"),
                    "/icon.png"
                )))
                .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    let data: Vec<u8> =
        fs::read("assets/fonts/SourceHanSerifCN-VF.ttf").expect("Failed to Open font file");

    eframe::run_native(
        "Collects",
        native_options,
        Box::new(move |cc| {
            add_font(&cc.egui_ctx, data);

            let state = State::default();
            let app = collects_ui::CollectsApp::new(state);
            Ok(Box::new(app))
        }),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::js_sys::{ArrayBuffer, Uint8Array};
    use web_sys::{Request, RequestInit, RequestMode, Response};

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("egui_canvas")
            .expect("Failed to find egui_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("egui_canvas was not a HtmlCanvasElement");

        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);

        let url = format!("./SourceHanSerifCN-VF.ttf");

        let request = Request::new_with_str_and_init(&url, &opts).unwrap();

        request.headers().set("Accept", "font/ttf").unwrap();

        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .expect("failed to fetch");

        // `resp_value` is a `Response` object.
        assert!(resp_value.is_instance_of::<Response>());
        let resp: Response = resp_value.dyn_into().unwrap();

        let arr_buf_value = JsFuture::from(resp.array_buffer().unwrap())
            .await
            .expect("failed to get array buffer");
        assert!(arr_buf_value.is_instance_of::<ArrayBuffer>());

        let arr_buf = arr_buf_value.dyn_into::<ArrayBuffer>().unwrap();
        let uin8_arr = Uint8Array::new(&arr_buf);
        let len = uin8_arr.length() as usize;
        let mut font_data = vec![0; len];
        uin8_arr.copy_to(&mut font_data[..]);

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    add_font(&cc.egui_ctx, font_data);

                    let state = State::default();
                    let app = collects_ui::CollectsApp::new(state);
                    Ok(Box::new(app))
                }),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
