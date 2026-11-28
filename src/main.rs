#![forbid(unsafe_code)]

mod app;
mod gree;
mod tray;
mod ui;

use std::net::Ipv4Addr;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--scan") {
        let manual_ip = args
            .iter()
            .position(|a| a == "--ip")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<Ipv4Addr>().ok());

        println!("Scanning for Gree AC devices...");
        match gree::client::scan_and_bind(manual_ip) {
            Ok(devices) if devices.is_empty() => {
                println!("No devices found.");
            }
            Ok(devices) => {
                for dev in devices {
                    println!(
                        "Found: {} | IP={} | MAC={} | ver={} | enc={} | bound={}",
                        dev.display_name(),
                        dev.ip,
                        dev.id,
                        dev.ver,
                        dev.encryption,
                        dev.key.is_some()
                    );
                    if dev.key.is_some() {
                        match gree::client::get_status(&dev) {
                            Ok(st) => println!("  Status: {st:#?}"),
                            Err(e) => println!("  Status error: {e}"),
                        }
                    }
                }
            }
            Err(e) => eprintln!("Scan failed: {e}"),
        }
        return Ok(());
    }

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_app_id("gree-remote-app")
        .with_inner_size([494.0, 750.0])
        .with_min_inner_size([460.0, 680.0])
        .with_title("Gree Remote");

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")) {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Gree Remote",
        options,
        Box::new(|cc| Ok(Box::new(app::GreeRemoteApp::new(cc)))),
    )
}
