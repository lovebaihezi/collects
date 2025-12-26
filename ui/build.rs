fn main() {
    #[cfg(target_os = "windows")]
    {
        use std::fs::File;
        use std::io::BufWriter;
        use std::path::Path;
        
        // Convert PNG to ICO
        let png_path = "assets/icon-256.png";
        let ico_path = "assets/icon.ico";
        
        // Only convert if ico doesn't exist or png is newer
        let should_convert = if !Path::new(ico_path).exists() {
            true
        } else {
            match (std::fs::metadata(png_path), std::fs::metadata(ico_path)) {
                (Ok(png_meta), Ok(ico_meta)) => {
                    match (png_meta.modified(), ico_meta.modified()) {
                        (Ok(png_time), Ok(ico_time)) => png_time > ico_time,
                        _ => true, // If we can't determine modification time, regenerate
                    }
                }
                _ => true, // If metadata is unavailable, regenerate
            }
        };
        
        if should_convert {
            println!("cargo:rerun-if-changed={}", png_path);
            
            // Load the PNG image
            let img = image::open(png_path).expect("Failed to open icon PNG");
            let img = img.to_rgba8();
            
            // Create ICO file
            let ico_file = File::create(ico_path).expect("Failed to create ICO file");
            let mut ico_writer = BufWriter::new(ico_file);
            
            let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
            let icon_image = ico::IconImage::from_rgba_data(
                img.width(),
                img.height(),
                img.into_raw(),
            );
            icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image).expect("Failed to encode icon"));
            icon_dir.write(&mut ico_writer).expect("Failed to write ICO file");
        }
        
        println!("cargo:rerun-if-changed={}", ico_path);
        
        // Create Windows resource file
        let rc_content = r#"
1 ICON "assets/icon.ico"
"#;
        std::fs::write("windows-resources.rc", rc_content).expect("Failed to write RC file");
        
        // Compile the resource file
        embed_resource::compile("windows-resources.rc", embed_resource::NONE);
    }
}
