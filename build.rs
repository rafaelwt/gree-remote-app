//! Embeds the application icon into the Windows executable so Explorer,
//! the taskbar and shortcuts show it.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.png");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let ico_path = out_dir.join("icon.ico");

    let img = image::open("assets/icon.png").expect("failed to read assets/icon.png");
    let frames: Vec<image::codecs::ico::IcoFrame> = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&size| {
            let rgba = img
                .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            image::codecs::ico::IcoFrame::as_png(
                rgba.as_raw(),
                size,
                size,
                image::ExtendedColorType::Rgba8,
            )
            .expect("failed to encode icon frame")
        })
        .collect();
    let file = std::fs::File::create(&ico_path).expect("failed to create icon.ico");
    image::codecs::ico::IcoEncoder::new(file)
        .encode_images(&frames)
        .expect("failed to write icon.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ico_path.to_str().expect("non UTF-8 OUT_DIR"));
    res.set("ProductName", "Gree Remote");
    res.set("FileDescription", "Gree Remote");
    res.compile().expect("failed to embed Windows resources");
}
