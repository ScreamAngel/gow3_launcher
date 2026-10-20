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
    // Determine base_dir:
    // 1. If running as an AppImage, use the directory where the .AppImage file is located
    // 2. If running as standalone executable, use the folder containing the binary
    // 3. Fallback to current working directory
    let base_dir = if let Ok(appimage) = std::env::var("APPIMAGE") {
        PathBuf::from(appimage).parent().unwrap_or(&PathBuf::from(".")).to_path_buf()
    } else if let Ok(exe_path) = std::env::current_exe() {
        let exe_dir = exe_path.parent().unwrap_or(&PathBuf::from(".")).to_path_buf();
        let cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        if cur.ends_with("launcher") {
            cur.parent().unwrap_or(&cur).to_path_buf()
        } else if exe_dir.ends_with("target/release") || exe_dir.ends_with("target/debug") {
            cur
        } else {
            exe_dir
        }
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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

    // Try load window icon: check base_dir first, then fallback to embedded asset
    let icon_data = if let Ok(img) = image::open(base_dir.join("gow3.png")) {
        let width = img.width();
        let height = img.height();
        let rgba = img.to_rgba8().into_raw();
        Some(egui::IconData { rgba, width, height })
    } else if let Ok(img) = image::load_from_memory(include_bytes!("../../gow3.png")) {
        let width = img.width();
        let height = img.height();
        let rgba = img.to_rgba8().into_raw();
        Some(egui::IconData { rgba, width, height })
    } else {
        None
    };

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
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
