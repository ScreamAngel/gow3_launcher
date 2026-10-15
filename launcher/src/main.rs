mod app;
mod config;
mod crash_analyzer;
mod i18n;
mod scanner;
mod system;
mod tuner;
mod updater;
mod runner;

use app::LauncherApp;
use eframe::egui;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    // Determine base_dir: If running inside 'launcher' folder, go up to parent, else use current dir
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let base_dir = if current_dir.ends_with("launcher") {
        current_dir.parent().unwrap_or(&current_dir).to_path_buf()
    } else {
        current_dir
    };

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--play" || a == "--start" || a == "-p") {
        let config = config::LauncherConfig::load(&base_dir);
        let gamemode_avail = system::is_gamemode_available();
        let use_gamemode = gamemode_avail && config.use_gamemode;
        let _ = tuner::apply_settings(
            &base_dir,
            config.target_fps,
            config.patch_delta_time,
            config.patch_readbacks,
            config.vram_gc_budget_mb,
            config.dmem_mb,
        );
        match runner::launch_gow3(&base_dir, &config, use_gamemode) {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("Error launching God of War III: {}", e);
                std::process::exit(1);
            }
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("God of War® III Remastered - Launcher")
        .with_inner_size([800.0, 720.0])
        .with_min_inner_size([650.0, 550.0])
        .with_resizable(true);

    // Try load window icon
    let icon_path = base_dir.join("gow3.png");
    if icon_path.is_file() {
        if let Ok(img) = image::open(&icon_path) {
            let width = img.width();
            let height = img.height();
            let rgba = img.to_rgba8().into_raw();
            viewport = viewport.with_icon(egui::IconData {
                rgba,
                width,
                height,
            });
        }
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let base_dir_clone = base_dir.clone();
    eframe::run_native(
        "God of War III Launcher",
        native_options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(LauncherApp::new(cc, base_dir_clone)))
        }),
    )
}
