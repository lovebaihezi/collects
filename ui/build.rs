use std::env;

#[path = "build/mod.rs"]
mod build;

use build::icon;

fn main() {
    let png_path = "assets/icon-256.png";
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    // Print cargo rerun directives
    icon::print_rerun_directives(png_path);

    // Generate the icon PNG (and apply env-specific transformations)
    let processed_img = icon::generate_icon(png_path, &out_dir);

    // Windows-specific: generate ICO and compile resource
    #[cfg(target_os = "windows")]
    {
        let ico_path = icon::generate_windows_ico(processed_img, &out_dir);
        icon::compile_windows_resource(&ico_path, &out_dir);
    }

    // Suppress unused variable warning on non-Windows
    #[cfg(not(target_os = "windows"))]
    let _ = processed_img;
}
