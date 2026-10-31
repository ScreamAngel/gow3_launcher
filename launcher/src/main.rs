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
        let _ = tuner::apply_settings(&base_dir, &config);
        match runner::launch_gow3(&base_dir, &config, use_gamemode, None) {
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

    if let Some(icon) = load_window_icon(&base_dir) {
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

fn load_window_icon(base_dir: &std::path::Path) -> Option<egui::IconData> {
    // 1. Try to load gow3.png from base_dir, current_dir, exe_dir, or fallback to embedded asset
    let img = if let Ok(img) = image::open(base_dir.join("gow3.png")) {
        Some(img)
    } else if let Ok(cur) = std::env::current_dir() {
        if let Ok(img) = image::open(cur.join("gow3.png")) {
            Some(img)
        } else {
            None
        }
    } else {
        None
    }
    .or_else(|| {
        if let Ok(exe_p) = std::env::current_exe() {
            if let Some(parent) = exe_p.parent() {
                if let Ok(img) = image::open(parent.join("gow3.png")) {
                    return Some(img);
                }
            }
        }
        image::load_from_memory(include_bytes!("../../gow3.png")).ok()
    })?;

    // Downscale to 256x256 if larger:
    // Windows Win32 User32 CreateIcon fails on dimensions > 256, causing taskbar icon to revert to generic.
    // Scaling to 256x256 ensures smooth High-DPI rendering and 100% Win32 / Wayland / X11 taskbar compatibility.
    let (w, h) = (img.width(), img.height());
    let resized = if w > 256 || h > 256 || w != h {
        img.resize_exact(256, 256, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let width = resized.width();
    let height = resized.height();
    let rgba = resized.to_rgba8().into_raw();
    Some(egui::IconData { rgba, width, height })
}
