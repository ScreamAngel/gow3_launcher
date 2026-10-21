use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use crate::config::{DisplayMode, KeyboardBindings, LauncherConfig};
use crate::crash_analyzer::{analyze_post_game, CrashDiagnosis};
use crate::i18n::{text, Language};
use crate::scanner::{is_valid_game_dir, scan_for_game, update_launch_scripts, GameCandidate};
use crate::system::{detect_hardware, is_gamemode_available};
use crate::tuner::apply_settings;
use crate::updater::{apply_github_update, check_github_update, UpdateInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Game,
    Config,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyBindingTarget {
    Triangle,
    Circle,
    Cross,
    Square,
    L1,
    R1,
    L2,
    R2,
    L3,
    R3,
    Options,
    TouchpadCenter,
    PadUp,
    PadDown,
    PadLeft,
    PadRight,
    AxisLeftYMinus,
    AxisLeftYPlus,
    AxisLeftXMinus,
    AxisLeftXPlus,
    AxisRightYMinus,
    AxisRightYPlus,
    AxisRightXMinus,
    AxisRightXPlus,
}

pub fn egui_key_to_shadps4(key: egui::Key) -> Option<&'static str> {
    match key {
        egui::Key::ArrowDown => Some("down"),
        egui::Key::ArrowLeft => Some("left"),
        egui::Key::ArrowRight => Some("right"),
        egui::Key::ArrowUp => Some("up"),
        egui::Key::Tab => Some("tab"),
        egui::Key::Backspace => Some("backspace"),
        egui::Key::Enter => Some("enter"),
        egui::Key::Space => Some("space"),
        egui::Key::Insert => Some("insert"),
        egui::Key::Delete => Some("delete"),
        egui::Key::Home => Some("home"),
        egui::Key::End => Some("end"),
        egui::Key::PageUp => Some("pageup"),
        egui::Key::PageDown => Some("pagedown"),
        egui::Key::Num0 => Some("0"),
        egui::Key::Num1 => Some("1"),
        egui::Key::Num2 => Some("2"),
        egui::Key::Num3 => Some("3"),
        egui::Key::Num4 => Some("4"),
        egui::Key::Num5 => Some("5"),
        egui::Key::Num6 => Some("6"),
        egui::Key::Num7 => Some("7"),
        egui::Key::Num8 => Some("8"),
        egui::Key::Num9 => Some("9"),
        egui::Key::A => Some("a"),
        egui::Key::B => Some("b"),
        egui::Key::C => Some("c"),
        egui::Key::D => Some("d"),
        egui::Key::E => Some("e"),
        egui::Key::F => Some("f"),
        egui::Key::G => Some("g"),
        egui::Key::H => Some("h"),
        egui::Key::I => Some("i"),
        egui::Key::J => Some("j"),
        egui::Key::K => Some("k"),
        egui::Key::L => Some("l"),
        egui::Key::M => Some("m"),
        egui::Key::N => Some("n"),
        egui::Key::O => Some("o"),
        egui::Key::P => Some("p"),
        egui::Key::Q => Some("q"),
        egui::Key::R => Some("r"),
        egui::Key::S => Some("s"),
        egui::Key::T => Some("t"),
        egui::Key::U => Some("u"),
        egui::Key::V => Some("v"),
        egui::Key::W => Some("w"),
        egui::Key::X => Some("x"),
        egui::Key::Y => Some("y"),
        egui::Key::Z => Some("z"),
        egui::Key::F1 => Some("f1"),
        egui::Key::F2 => Some("f2"),
        egui::Key::F3 => Some("f3"),
        egui::Key::F4 => Some("f4"),
        egui::Key::F5 => Some("f5"),
        egui::Key::F6 => Some("f6"),
        egui::Key::F7 => Some("f7"),
        egui::Key::F8 => Some("f8"),
        egui::Key::F9 => Some("f9"),
        egui::Key::F10 => Some("f10"),
        egui::Key::F11 => Some("f11"),
        egui::Key::F12 => Some("f12"),
        egui::Key::ShiftLeft | egui::Key::ShiftRight => Some("shift"),
        egui::Key::ControlLeft | egui::Key::ControlRight => Some("ctrl"),
        egui::Key::AltLeft | egui::Key::AltRight => Some("alt"),
        _ => None,
    }
}

pub fn format_key_display(key_str: &str) -> String {
    match key_str.to_lowercase().as_str() {
        "kp0" => "Numpad 0".to_string(),
        "kp1" => "Numpad 1".to_string(),
        "kp2" => "Numpad 2".to_string(),
        "kp3" => "Numpad 3".to_string(),
        "kp4" => "Numpad 4".to_string(),
        "kp5" => "Numpad 5".to_string(),
        "kp6" => "Numpad 6".to_string(),
        "kp7" => "Numpad 7".to_string(),
        "kp8" => "Numpad 8".to_string(),
        "kp9" => "Numpad 9".to_string(),
        "up" => "↑ (Yukarı)".to_string(),
        "down" => "↓ (Aşağı)".to_string(),
        "left" => "← (Sol)".to_string(),
        "right" => "→ (Sağ)".to_string(),
        "space" => "Space".to_string(),
        "enter" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "escape" => "Esc".to_string(),
        "shift" => "Shift".to_string(),
        "ctrl" => "Ctrl".to_string(),
        "alt" => "Alt".to_string(),
        s if s.len() == 1 => s.to_uppercase(),
        s => s.to_uppercase(),
    }
}

fn render_binding_row(
    ui: &mut egui::Ui,
    label: &str,
    target: KeyBindingTarget,
    current_key: &str,
    listening_for: Option<KeyBindingTarget>,
    next_listening: &mut Option<KeyBindingTarget>,
) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.5));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let is_listening = listening_for == Some(target);
            let btn_text = if is_listening {
                "👉 [ Basın... / ESC ]".to_string()
            } else {
                format_key_display(current_key)
            };

            let text_color = if is_listening {
                egui::Color32::from_rgb(255, 220, 80)
            } else {
                egui::Color32::WHITE
            };

            let bg_color = if is_listening {
                egui::Color32::from_rgb(180, 50, 50)
            } else {
                egui::Color32::from_rgb(46, 50, 62)
            };

            let btn = egui::Button::new(
                egui::RichText::new(btn_text).strong().size(12.0).color(text_color),
            )
            .min_size(egui::vec2(130.0, 26.0))
            .fill(bg_color)
            .corner_radius(egui::CornerRadius::same(5));

            if ui.add(btn).clicked() {
                if is_listening {
                    *next_listening = None;
                } else {
                    *next_listening = Some(target);
                }
            }
        });
    });
}

/// A custom animated pill toggle switch matching modern mobile/desktop toggle designs.
pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let desired_size = egui::vec2(42.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let radius = 0.5 * rect.height();

        let bg_r = (62.0 + (175.0 - 62.0) * how_on) as u8;
        let bg_g = (64.0 + (195.0 - 64.0) * how_on) as u8;
        let bg_b = (71.0 + (248.0 - 71.0) * how_on) as u8;
        let bg_color = egui::Color32::from_rgb(bg_r, bg_g, bg_b);

        ui.painter().rect_filled(rect, radius, bg_color);

        let padding = 2.5;
        let knob_radius = radius - padding;
        let left_x = rect.left() + radius;
        let right_x = rect.right() - radius;
        let knob_x = egui::lerp(left_x..=right_x, how_on);
        let knob_center = egui::pos2(knob_x, rect.center().y);

        let knob_r = (255.0 + (28.0 - 255.0) * how_on) as u8;
        let knob_g = (255.0 + (35.0 - 255.0) * how_on) as u8;
        let knob_b = (255.0 + (58.0 - 255.0) * how_on) as u8;
        let knob_color = egui::Color32::from_rgb(knob_r, knob_g, knob_b);

        ui.painter().circle_filled(knob_center, knob_radius, knob_color);
    }

    response
}

pub struct LauncherApp {
    base_dir: PathBuf,
    config: LauncherConfig,
    current_lang: Language,
    is_game_valid: bool,
    status_message: Option<(String, bool)>, // (message, is_error)
    has_gamemode: bool,

    // Active Navigation Tab
    active_tab: ActiveTab,
    listening_key_for: Option<KeyBindingTarget>,

    // Scanning state
    is_scanning: bool,
    scan_rx: Option<Receiver<Vec<GameCandidate>>>,
    found_candidates_modal: Option<Vec<GameCandidate>>,
    show_game_not_found_modal: bool,
    scan_failed_empty: bool,

    // Updater state
    is_checking_update: bool,
    update_rx: Option<Receiver<Result<UpdateInfo, String>>>,
    update_modal: Option<UpdateInfo>,

    // Game Running & Monitoring State
    is_game_running: bool,
    game_exit_rx: Option<Receiver<Option<i32>>>,
    running_pid: Arc<AtomicU32>,

    // Python dependency state
    has_python: bool,
    is_downloading_python: bool,
    python_dl_rx: Option<Receiver<Result<String, String>>>,

    // Engine dependency state
    has_engine: bool,
    is_downloading_engine: bool,
    engine_dl_rx: Option<Receiver<Result<String, String>>>,

    // Modals
    show_first_launch_modal: bool,
    active_crash_diagnosis: Option<CrashDiagnosis>,
    show_log_modal: bool,
    log_content: String,

    // Textures
    bg_texture: Option<egui::TextureHandle>,
    logo_texture: Option<egui::TextureHandle>,
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>, base_dir: PathBuf) -> Self {
        let mut config = LauncherConfig::load(&base_dir);

        // Ensure runtime assets are unpacked (.runtime/scripts and .runtime/patches)
        let _ = crate::runner::ensure_runtime_assets(&base_dir);

        // Always scan hardware on startup and cache
        let hw = detect_hardware();
        config.cached_hardware = hw;
        let _ = config.save(&base_dir);
        let _ = apply_settings(&base_dir, &config);

        let lang = Language::from_code(&config.language);
        let is_first = !config.first_launch_done;
        let game_valid = is_valid_game_dir(Path::new(&config.game_dir));
        let gamemode_avail = is_gamemode_available();
        let has_python = crate::runner::find_python(&base_dir).is_ok();
        let has_engine = crate::runner::find_game_engine(&base_dir).is_some();

        let mut app = Self {
            base_dir,
            config,
            current_lang: lang,
            is_game_valid: game_valid,
            status_message: None,
            has_gamemode: gamemode_avail,
            active_tab: ActiveTab::Game,
            listening_key_for: None,
            is_scanning: false,
            scan_rx: None,
            found_candidates_modal: None,
            show_game_not_found_modal: false,
            scan_failed_empty: false,
            is_checking_update: false,
            update_rx: None,
            update_modal: None,
            is_game_running: false,
            game_exit_rx: None,
            running_pid: Arc::new(AtomicU32::new(0)),
            has_python,
            is_downloading_python: false,
            python_dl_rx: None,
            has_engine,
            is_downloading_engine: false,
            engine_dl_rx: None,
            show_first_launch_modal: is_first,
            active_crash_diagnosis: None,
            show_log_modal: false,
            log_content: String::new(),
            bg_texture: None,
            logo_texture: None,
        };

        app.load_textures(&cc.egui_ctx);
        app
    }

    fn t(&self, key: &str) -> &'static str {
        text(self.current_lang, key)
    }

    fn load_textures(&mut self, ctx: &egui::Context) {
        let mut candidates = vec![
            self.base_dir.join("pic1.png"),
            self.base_dir.join("gow3-bg.png"),
        ];

        if !self.config.game_dir.is_empty() {
            let p = Path::new(&self.config.game_dir);
            candidates.push(p.join("sce_sys/pic1.png"));
            candidates.push(p.join("sce_sys/pic0.png"));
            candidates.push(p.join("pic1.png"));
        }

        candidates.push(self.base_dir.join("CUSA01623/sce_sys/pic1.png"));
        candidates.push(self.base_dir.join("CUSA01715/sce_sys/pic1.png"));

        if let Some(parent) = self.base_dir.parent() {
            candidates.push(parent.join("CUSA01623/sce_sys/pic1.png"));
            candidates.push(parent.join("CUSA01715/sce_sys/pic1.png"));
            candidates.push(parent.join("pic1.png"));
        }

        for path in &candidates {
            if path.is_file() {
                if let Ok(img) = image::open(path) {
                    let size = [img.width() as _, img.height() as _];
                    let img_buffer = img.to_rgba8();
                    let pixels = img_buffer.as_flat_samples();
                    let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                    self.bg_texture = Some(ctx.load_texture("gow3_full_bg", color_img, Default::default()));
                    break;
                }
            }
        }

        let logo_path = self.base_dir.join("gow3-remastered.png");
        let logo_img = if logo_path.is_file() {
            image::open(&logo_path).ok()
        } else {
            image::load_from_memory(include_bytes!("../../gow3-remastered.png")).ok()
        };
        if let Some(img) = logo_img {
            let size = [img.width() as _, img.height() as _];
            let img_buffer = img.to_rgba8();
            let pixels = img_buffer.as_flat_samples();
            let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.logo_texture = Some(ctx.load_texture("gow3_remastered_logo", color_img, Default::default()));
        }
    }

    fn check_game_validity(&mut self) {
        self.is_game_valid = is_valid_game_dir(Path::new(&self.config.game_dir));
        if self.is_game_valid {
            self.scan_failed_empty = false;
            if let Some((_, is_err)) = self.status_message {
                if is_err {
                    self.status_message = Some((self.t("game_valid").to_string(), false));
                }
            }
        }
    }

    fn set_game_directory(&mut self, path: &Path, ctx: &egui::Context) {
        self.config.game_dir = path.to_string_lossy().to_string();
        let _ = update_launch_scripts(&self.base_dir, path);
        let _ = self.config.save(&self.base_dir);
        self.check_game_validity();
        self.load_textures(ctx);
        self.scan_failed_empty = false;
        if self.is_game_valid {
            self.status_message = Some((self.t("game_valid").to_string(), false));
        } else {
            self.status_message = Some((self.t("game_invalid").to_string(), true));
        }
    }

    fn apply_all_optimizations(&mut self) {
        let res = apply_settings(&self.base_dir, &self.config);

        match res {
            Ok(_) => {
                let _ = self.config.save(&self.base_dir);
                let msg = match self.current_lang {
                    Language::Turkish => "✅ Tüm ayarlar ve yapılandırma başarıyla kaydedildi & uygulandı!",
                    Language::English => "✅ All settings and configuration saved & applied successfully!",
                    Language::German => "✅ Alle Einstellungen und Konfigurationen erfolgreich gespeichert & angewendet!",
                    Language::Spanish => "✅ ¡Todos los ajustes y configuración guardados y aplicados con éxito!",
                };
                self.status_message = Some((msg.to_string(), false));
            }
            Err(e) => {
                self.status_message = Some((format!("❌ Error: {}", e), true));
            }
        }
    }

    fn start_game_scan(&mut self) {
        self.is_scanning = true;
        self.scan_failed_empty = false;
        let (tx, rx): (Sender<Vec<GameCandidate>>, Receiver<Vec<GameCandidate>>) = channel();
        self.scan_rx = Some(rx);

        thread::spawn(move || {
            let results = scan_for_game();
            let _ = tx.send(results);
        });
    }

    fn start_update_check(&mut self) {
        if self.is_checking_update {
            return;
        }
        self.is_checking_update = true;
        let (tx, rx) = channel();
        self.update_rx = Some(rx);

        let current_commit = self.config.last_git_commit.clone();
        thread::spawn(move || {
            let res = check_github_update(&current_commit);
            let _ = tx.send(res);
        });
    }

    fn start_python_download(&mut self) {
        if self.is_downloading_python {
            return;
        }
        self.is_downloading_python = true;
        let (tx, rx) = channel();
        self.python_dl_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        thread::spawn(move || {
            let res = crate::runner::download_portable_python(&base_dir_clone);
            let _ = tx.send(res);
        });
    }

    fn start_engine_download(&mut self) {
        if self.is_downloading_engine {
            return;
        }
        self.is_downloading_engine = true;
        let (tx, rx) = channel();
        self.engine_dl_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        thread::spawn(move || {
            let res = crate::runner::download_engine(&base_dir_clone);
            let _ = tx.send(res);
        });
    }

    fn force_quit_game(&mut self) {
        let pid = self.running_pid.load(Ordering::SeqCst);
        crate::runner::kill_game_processes(pid);
        self.running_pid.store(0, Ordering::SeqCst);
        self.is_game_running = false;
        self.game_exit_rx = None;
        self.status_message = Some((self.t("force_quit_status").to_string(), false));
    }

    fn kill_leftover_processes(&mut self) {
        crate::runner::kill_game_processes(0);
        self.status_message = Some((self.t("force_quit_status").to_string(), false));
    }

    fn launch_game(&mut self) {
        self.apply_all_optimizations();

        self.is_game_running = true;
        let (tx, rx) = channel();
        self.game_exit_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        let config_clone = self.config.clone();
        let use_gamemode = self.has_gamemode && self.config.use_gamemode;
        let pid_holder = self.running_pid.clone();

        thread::spawn(move || {
            match crate::runner::launch_gow3(&base_dir_clone, &config_clone, use_gamemode, Some(pid_holder)) {
                Ok(code) => {
                    let _ = tx.send(Some(code));
                }
                Err(err) => {
                    eprintln!("Launch error: {}", err);
                    let log_path = base_dir_clone.join("gow3_log.txt");
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                        use std::io::Write;
                        let _ = writeln!(f, "\n[LAUNCH ERROR] {}", err);
                    }
                    let _ = tx.send(Some(-1));
                }
            }
        });
    }
}

impl eframe::App for LauncherApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // 1. Process key capture if user clicked a keybinding button
        if let Some(target) = self.listening_key_for {
            let mut captured_key = None;
            let mut cancel = false;

            ctx.input(|i| {
                for event in &i.events {
                    if let egui::Event::Key { key, pressed: true, .. } = event {
                        if *key == egui::Key::Escape {
                            cancel = true;
                        } else if let Some(shad_name) = egui_key_to_shadps4(*key) {
                            captured_key = Some(shad_name.to_string());
                        }
                    }
                }
            });

            if cancel {
                self.listening_key_for = None;
            } else if let Some(new_key) = captured_key {
                match target {
                    KeyBindingTarget::Triangle => self.config.keyboard_bindings.triangle = new_key,
                    KeyBindingTarget::Circle => self.config.keyboard_bindings.circle = new_key,
                    KeyBindingTarget::Cross => self.config.keyboard_bindings.cross = new_key,
                    KeyBindingTarget::Square => self.config.keyboard_bindings.square = new_key,
                    KeyBindingTarget::L1 => self.config.keyboard_bindings.l1 = new_key,
                    KeyBindingTarget::R1 => self.config.keyboard_bindings.r1 = new_key,
                    KeyBindingTarget::L2 => self.config.keyboard_bindings.l2 = new_key,
                    KeyBindingTarget::R2 => self.config.keyboard_bindings.r2 = new_key,
                    KeyBindingTarget::L3 => self.config.keyboard_bindings.l3 = new_key,
                    KeyBindingTarget::R3 => self.config.keyboard_bindings.r3 = new_key,
                    KeyBindingTarget::Options => self.config.keyboard_bindings.options = new_key,
                    KeyBindingTarget::TouchpadCenter => self.config.keyboard_bindings.touchpad_center = new_key,
                    KeyBindingTarget::PadUp => self.config.keyboard_bindings.pad_up = new_key,
                    KeyBindingTarget::PadDown => self.config.keyboard_bindings.pad_down = new_key,
                    KeyBindingTarget::PadLeft => self.config.keyboard_bindings.pad_left = new_key,
                    KeyBindingTarget::PadRight => self.config.keyboard_bindings.pad_right = new_key,
                    KeyBindingTarget::AxisLeftYMinus => self.config.keyboard_bindings.axis_left_y_minus = new_key,
                    KeyBindingTarget::AxisLeftYPlus => self.config.keyboard_bindings.axis_left_y_plus = new_key,
                    KeyBindingTarget::AxisLeftXMinus => self.config.keyboard_bindings.axis_left_x_minus = new_key,
                    KeyBindingTarget::AxisLeftXPlus => self.config.keyboard_bindings.axis_left_x_plus = new_key,
                    KeyBindingTarget::AxisRightYMinus => self.config.keyboard_bindings.axis_right_y_minus = new_key,
                    KeyBindingTarget::AxisRightYPlus => self.config.keyboard_bindings.axis_right_y_plus = new_key,
                    KeyBindingTarget::AxisRightXMinus => self.config.keyboard_bindings.axis_right_x_minus = new_key,
                    KeyBindingTarget::AxisRightXPlus => self.config.keyboard_bindings.axis_right_x_plus = new_key,
                }
                self.listening_key_for = None;
            }
        }

        // 2. Check background scanner thread
        if let Some(ref rx) = self.scan_rx {
            if let Ok(candidates) = rx.try_recv() {
                self.is_scanning = false;
                self.scan_rx = None;
                if candidates.is_empty() {
                    let msg = self.t("scan_not_found_inline");
                    self.status_message = Some((msg.to_string(), true));
                    self.scan_failed_empty = true;
                    self.show_game_not_found_modal = true;
                } else {
                    self.scan_failed_empty = false;
                    self.found_candidates_modal = Some(candidates);
                }
            }
        }

        // 3. Check game process exit & analyze crash
        if let Some(ref rx) = self.game_exit_rx {
            if let Ok(exit_code) = rx.try_recv() {
                self.is_game_running = false;
                self.game_exit_rx = None;

                if let Some(diagnosis) = analyze_post_game(&self.base_dir, exit_code) {
                    self.active_crash_diagnosis = Some(diagnosis);
                }
            }
        }

        // 4. Check background update checker thread
        if let Some(ref rx) = self.update_rx {
            if let Ok(res) = rx.try_recv() {
                self.is_checking_update = false;
                self.update_rx = None;
                match res {
                    Ok(info) => {
                        self.update_modal = Some(info);
                    }
                    Err(e) => {
                        self.status_message = Some((format!("❌ Güncelleme hatası: {}", e), true));
                    }
                }
            }
        }

        // 5. Check Python downloader thread
        if let Some(ref rx) = self.python_dl_rx {
            if let Ok(res) = rx.try_recv() {
                self.is_downloading_python = false;
                self.python_dl_rx = None;
                match res {
                    Ok(_) => {
                        self.has_python = true;
                        self.status_message = Some((self.t("python_download_success").to_string(), false));
                    }
                    Err(e) => {
                        self.status_message = Some((format!("{}{}", self.t("python_download_err"), e), true));
                    }
                }
            }
        }

        // 6. Check Engine downloader thread
        if let Some(ref rx) = self.engine_dl_rx {
            if let Ok(res) = rx.try_recv() {
                self.is_downloading_engine = false;
                self.engine_dl_rx = None;
                match res {
                    Ok(_) => {
                        self.has_engine = true;
                        self.has_python = crate::runner::find_python(&self.base_dir).is_ok();
                        self.status_message = Some((self.t("engine_download_success").to_string(), false));
                    }
                    Err(e) => {
                        self.status_message = Some((format!("❌ {}", e), true));
                    }
                }
            }
        }

        // --- FULL SCREEN BACKGROUND WALLPAPER ---
        if let Some(ref tex) = self.bg_texture {
            let painter = ui.painter();
            let rect = ui.max_rect();
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            painter.image(
                tex.id(),
                rect,
                uv,
                egui::Color32::from_white_alpha(170),
            );
            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(10, 10, 14, 150),
            );
        }

        let mut visuals = egui::Visuals::dark();
        visuals.window_corner_radius = egui::CornerRadius::same(10);
        visuals.override_text_color = Some(egui::Color32::from_rgb(240, 240, 240));
        ctx.set_visuals(visuals);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);

            // --- TOP UTILITY BAR (UPDATE CHECKER & LANGUAGE SELECTOR) ---
            ui.horizontal(|ui| {
                if self.is_checking_update {
                    ui.add_enabled(
                        false,
                        egui::Button::new(
                            egui::RichText::new(self.t("checking_updates"))
                                .color(egui::Color32::from_rgb(180, 180, 190))
                                .size(13.0),
                        )
                        .corner_radius(egui::CornerRadius::same(6)),
                    );
                } else {
                    let update_btn = egui::Button::new(
                        egui::RichText::new(self.t("check_updates"))
                            .color(egui::Color32::from_rgb(220, 220, 230))
                            .size(13.0),
                    )
                    .fill(egui::Color32::from_rgb(42, 44, 56))
                    .corner_radius(egui::CornerRadius::same(6));

                    if ui.add(update_btn).clicked() {
                        self.start_update_check();
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mut selected_lang = self.current_lang;
                    egui::ComboBox::from_id_salt("lang_selector")
                        .selected_text(selected_lang.display_name())
                        .show_ui(ui, |ui| {
                            for lang in Language::all() {
                                if ui.selectable_value(&mut selected_lang, *lang, lang.display_name()).clicked() {
                                    self.current_lang = *lang;
                                    self.config.language = lang.code().to_string();
                                    let _ = self.config.save(&self.base_dir);
                                }
                            }
                        });
                });
            });

            // --- CENTERED HEADER LOGO & SUBTITLE ---
            ui.vertical_centered(|ui| {
                if let Some(ref logo) = self.logo_texture {
                    ui.add(egui::Image::new(logo).max_height(90.0));
                } else {
                    ui.heading(
                        egui::RichText::new(self.t("app_title"))
                            .size(26.0)
                            .color(egui::Color32::from_rgb(235, 60, 50))
                            .strong(),
                    );
                }
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(self.t("app_subtitle"))
                        .size(12.0)
                        .color(egui::Color32::from_rgb(205, 205, 215)),
                );
            });

            // Status message
            if let Some((ref msg, is_err)) = self.status_message {
                let color = if is_err {
                    egui::Color32::from_rgb(245, 90, 85)
                } else {
                    egui::Color32::from_rgb(105, 225, 110)
                };
                ui.label(egui::RichText::new(msg).color(color).strong());
            }

            // --- NAVIGATION TABS ---
            ui.horizontal(|ui| {
                let game_active = self.active_tab == ActiveTab::Game;
                let config_active = self.active_tab == ActiveTab::Config;

                let tab_btn = |text: &str, active: bool| {
                    let text_color = if active {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(180, 185, 200)
                    };
                    let bg_color = if active {
                        egui::Color32::from_rgb(215, 45, 45)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(35, 38, 50, 200)
                    };

                    egui::Button::new(
                        egui::RichText::new(text).size(14.0).strong().color(text_color),
                    )
                    .fill(bg_color)
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(210.0, 36.0))
                };

                if ui.add(tab_btn(self.t("tab_game_perf"), game_active)).clicked() {
                    self.active_tab = ActiveTab::Game;
                }
                ui.add_space(8.0);
                if ui.add(tab_btn(self.t("tab_config_controls"), config_active)).clicked() {
                    self.active_tab = ActiveTab::Config;
                }
            });

            let card_frame = egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgba_unmultiplied(18, 18, 24, 215))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 50, 50, 75)))
                .inner_margin(12.0)
                .corner_radius(egui::CornerRadius::same(10));

            // ==========================================
            // TAB 1: GAME & PERFORMANCE
            // ==========================================
            if self.active_tab == ActiveTab::Game {
                // 1. SYSTEM HARDWARE INFO CARD
                card_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            egui::RichText::new(self.t("sys_hardware"))
                                .size(16.0)
                                .color(egui::Color32::from_rgb(150, 205, 255)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new(self.t("locked_auto")).color(egui::Color32::GRAY).size(11.0));
                        });
                    });
                    ui.separator();

                    let hw = &self.config.cached_hardware;

                    let (vram_color, vram_badge) = if hw.vram_mb >= 6144 {
                        (egui::Color32::from_rgb(102, 225, 106), format!("• {}", self.t("vram_safe")))
                    } else if hw.vram_mb >= 4096 {
                        (egui::Color32::from_rgb(255, 213, 79), "• Orta Seviye".to_string())
                    } else {
                        (egui::Color32::from_rgb(239, 83, 80), format!("• {}", self.t("vram_warning")))
                    };

                    let (ram_color, ram_badge) = if hw.ram_mb >= 8192 {
                        (egui::Color32::from_rgb(102, 225, 106), format!("• {}", self.t("ram_safe")))
                    } else {
                        (egui::Color32::from_rgb(255, 180, 50), format!("• {}", self.t("ram_warning")))
                    };

                    ui.columns(4, |cols| {
                        cols[0].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("gpu")).color(egui::Color32::LIGHT_GRAY).size(11.0));
                            ui.label(egui::RichText::new(&hw.gpu_name).strong());
                        });
                        cols[1].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("vram")).color(egui::Color32::LIGHT_GRAY).size(11.0));
                            ui.label(egui::RichText::new(format!("{} MB (~{} GB)", hw.vram_mb, hw.vram_mb / 1024)).color(vram_color).strong());
                            ui.label(egui::RichText::new(vram_badge).color(vram_color).size(10.0));
                        });
                        cols[2].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("ram")).color(egui::Color32::LIGHT_GRAY).size(11.0));
                            ui.label(egui::RichText::new(format!("{} MB (~{} GB)", hw.ram_mb, (hw.ram_mb as f32 / 1024.0).round())).color(ram_color).strong());
                            ui.label(egui::RichText::new(ram_badge).color(ram_color).size(10.0));
                        });
                        cols[3].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("refresh_rate")).color(egui::Color32::LIGHT_GRAY).size(11.0));
                            ui.label(egui::RichText::new(format!("{} Hz", hw.refresh_rate_hz)).color(egui::Color32::from_rgb(130, 205, 135)).strong());
                        });
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.t("python_dep")).color(egui::Color32::LIGHT_GRAY).size(12.0));
                        if self.has_python {
                            ui.label(
                                egui::RichText::new(self.t("python_ready"))
                                    .color(egui::Color32::from_rgb(102, 225, 106))
                                    .strong()
                                    .size(12.0),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(self.t("python_missing"))
                                    .color(egui::Color32::from_rgb(239, 83, 80))
                                    .strong()
                                    .size(12.0),
                            );

                            ui.add_space(8.0);
                            if self.is_downloading_python {
                                ui.spinner();
                                ui.label(
                                    egui::RichText::new(self.t("python_downloading"))
                                        .color(egui::Color32::from_rgb(255, 213, 79))
                                        .size(12.0),
                                );
                            } else {
                                let dl_btn = egui::Button::new(
                                    egui::RichText::new(self.t("python_download"))
                                        .size(11.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(45, 120, 185))
                                .corner_radius(egui::CornerRadius::same(5));

                                if ui.add(dl_btn).clicked() {
                                    self.start_python_download();
                                }
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.t("engine_label")).color(egui::Color32::LIGHT_GRAY).size(12.0));
                        if self.has_engine {
                            ui.label(
                                egui::RichText::new(self.t("engine_ready"))
                                    .color(egui::Color32::from_rgb(102, 225, 106))
                                    .strong()
                                    .size(12.0),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(self.t("engine_missing"))
                                    .color(egui::Color32::from_rgb(239, 83, 80))
                                    .strong()
                                    .size(12.0),
                            );

                            ui.add_space(8.0);
                            if self.is_downloading_engine {
                                ui.spinner();
                                ui.label(
                                    egui::RichText::new(self.t("engine_downloading"))
                                        .color(egui::Color32::from_rgb(255, 213, 79))
                                        .size(12.0),
                                );
                            } else {
                                let dl_btn = egui::Button::new(
                                    egui::RichText::new(self.t("engine_download"))
                                        .size(11.0)
                                        .color(egui::Color32::WHITE)
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(180, 80, 45))
                                .corner_radius(egui::CornerRadius::same(5));

                                if ui.add(dl_btn).clicked() {
                                    self.start_engine_download();
                                }
                            }
                        }
                    });
                });

                // 2. GAME DIRECTORY CARD
                card_frame.show(ui, |ui| {
                    ui.heading(
                        egui::RichText::new(self.t("game_location"))
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 205, 255)),
                    );
                    ui.separator();

                    ui.horizontal(|ui| {
                        let dir_display = if self.config.game_dir.is_empty() {
                            self.t("no_folder_selected")
                        } else {
                            &self.config.game_dir
                        };

                        ui.label(egui::RichText::new(dir_display).monospace());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new(self.t("manual_select")).strong()).clicked() {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.set_game_directory(&folder, &ctx);
                                }
                            }

                            if self.is_scanning {
                                ui.spinner();
                                ui.label(self.t("scanning"));
                            } else if ui.button(egui::RichText::new(self.t("auto_scan")).strong()).clicked() {
                                self.start_game_scan();
                            }
                        });
                    });

                    ui.add_space(4.0);
                    if self.is_game_valid {
                        ui.label(
                            egui::RichText::new(self.t("game_valid"))
                                .color(egui::Color32::from_rgb(105, 225, 110))
                                .strong(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(self.t("game_invalid"))
                                .color(egui::Color32::from_rgb(245, 90, 85))
                                .strong(),
                        );
                    }

                    if self.scan_failed_empty {
                        ui.add_space(6.0);
                        egui::Frame::group(ui.style())
                            .fill(egui::Color32::from_rgba_unmultiplied(65, 20, 20, 215))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(245, 80, 80)))
                            .inner_margin(10.0)
                            .corner_radius(egui::CornerRadius::same(6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("⚠️").size(18.0));
                                    ui.label(
                                        egui::RichText::new(self.t("scan_not_found_inline"))
                                            .color(egui::Color32::from_rgb(255, 140, 140))
                                            .strong(),
                                    );
                                });
                            });
                    }
                });

                // 3. PATCHES & OPTIMIZATIONS CARD
                card_frame.show(ui, |ui| {
                    ui.heading(
                        egui::RichText::new(self.t("patches_header"))
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 205, 255)),
                    );
                    ui.separator();

                    let hz_label = self.t("hz_patch");
                    let target_fps_label = self.t("target_fps");
                    let hz_desc = self.t("hz_desc");
                    let readback_label = self.t("readback_patch");
                    let readback_desc = self.t("readback_desc");
                    let skip_cinematics_label = self.t("skip_cinematics");
                    let skip_cinematics_desc = self.t("skip_cinematics_desc");
                    let gamemode_label = self.t("gamemode_toggle");
                    let gamemode_desc = self.t("gamemode_desc");

                    // Refresh Rate / Hz Sync Toggle
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.patch_delta_time);
                        if ui.add(egui::Label::new(egui::RichText::new(hz_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.patch_delta_time = !self.config.patch_delta_time;
                        }
                        ui.add_space(8.0);
                        ui.label(target_fps_label);
                        let current_text = if self.config.target_fps == 60 {
                            "60 FPS (Orijinal Hız - Önerilen)".to_string()
                        } else {
                            format!("{} FPS (Deneysel)", self.config.target_fps)
                        };
                        egui::ComboBox::from_id_salt("hz_combo")
                            .selected_text(current_text)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.config.target_fps, 60, "60 FPS (Orijinal Hız - Kararlı & Önerilen)");
                                ui.selectable_value(&mut self.config.target_fps, 120, "120 FPS (Deneysel Yüksek Hız)");
                                ui.selectable_value(&mut self.config.target_fps, 144, "144 FPS (Deneysel Yüksek Hız)");
                                ui.selectable_value(&mut self.config.target_fps, 165, "165 FPS (Deneysel Yüksek Hız)");
                                ui.selectable_value(&mut self.config.target_fps, 240, "240 FPS (Deneysel Yüksek Hız)");
                            });
                    });
                    ui.label(egui::RichText::new(hz_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);
                    // Yellow Texture Readback Patch Toggle
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.patch_readbacks);
                        if ui.add(egui::Label::new(egui::RichText::new(readback_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.patch_readbacks = !self.config.patch_readbacks;
                        }
                    });
                    ui.label(egui::RichText::new(readback_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);
                    // Skip Cinematics with 'X' Button Toggle
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.skip_cinematics);
                        if ui.add(egui::Label::new(egui::RichText::new(skip_cinematics_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.skip_cinematics = !self.config.skip_cinematics;
                        }
                    });
                    ui.label(egui::RichText::new(skip_cinematics_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    // Feral GameMode Toggle
                    if self.has_gamemode {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            toggle_switch(ui, &mut self.config.use_gamemode);
                            if ui.add(egui::Label::new(egui::RichText::new(gamemode_label).strong()).sense(egui::Sense::click())).clicked() {
                                self.config.use_gamemode = !self.config.use_gamemode;
                            }
                        });
                        ui.label(egui::RichText::new(gamemode_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new(self.t("save_apply")).strong()).clicked() {
                            self.apply_all_optimizations();
                        }
                        if ui.button(egui::RichText::new(self.t("view_log"))).clicked() {
                            let log_path = self.base_dir.join("gow3_log.txt");
                            self.log_content = std::fs::read_to_string(&log_path)
                                .unwrap_or_else(|_| "Log file is empty or not created yet.".to_string());
                            self.show_log_modal = true;
                        }
                        let kill_btn = egui::Button::new(
                            egui::RichText::new(self.t("kill_leftover_btn"))
                                .color(egui::Color32::from_rgb(255, 140, 140))
                                .size(12.0),
                        );
                        if ui.add(kill_btn).on_hover_text(self.t("kill_leftover_tooltip")).clicked() {
                            self.kill_leftover_processes();
                        }
                    });
                });

                // 4. PLAY GAME ACTION AREA
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if self.is_game_running {
                        ui.horizontal(|ui| {
                            let total_w = 210.0 + 12.0 + 190.0;
                            let available_w = ui.available_width();
                            if available_w > total_w {
                                ui.add_space((available_w - total_w) / 2.0);
                            }

                            ui.add_enabled(
                                false,
                                egui::Button::new(
                                    egui::RichText::new(self.t("play_running"))
                                        .size(17.0)
                                        .color(egui::Color32::from_rgb(130, 225, 140))
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(32, 50, 38))
                                .corner_radius(egui::CornerRadius::same(10))
                                .min_size(egui::vec2(210.0, 56.0)),
                            );

                            let force_quit_btn = egui::Button::new(
                                egui::RichText::new(self.t("force_quit_btn"))
                                    .size(17.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(215, 45, 45))
                            .corner_radius(egui::CornerRadius::same(10))
                            .min_size(egui::vec2(190.0, 56.0));

                            if ui.add(force_quit_btn).on_hover_text(self.t("force_quit_tooltip")).clicked() {
                                self.force_quit_game();
                            }
                        });
                    } else if self.is_game_valid {
                        let play_btn = egui::Button::new(
                            egui::RichText::new(self.t("play_ready"))
                                .size(22.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(215, 45, 45))
                        .corner_radius(egui::CornerRadius::same(10))
                        .min_size(egui::vec2(340.0, 56.0));

                        if ui.add(play_btn).clicked() {
                            self.launch_game();
                        }
                        ui.label(
                            egui::RichText::new(self.t("play_subtitle"))
                                .color(egui::Color32::from_rgb(105, 225, 110))
                                .size(12.0),
                        );
                    } else {
                        let play_btn = egui::Button::new(
                            egui::RichText::new(self.t("play_ready"))
                                .size(22.0)
                                .color(egui::Color32::from_rgb(140, 140, 140))
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(55, 55, 62))
                        .corner_radius(egui::CornerRadius::same(10))
                        .min_size(egui::vec2(340.0, 56.0));

                        ui.add_enabled(false, play_btn);

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(self.t("play_disabled_warn"))
                                .color(egui::Color32::from_rgb(245, 90, 85))
                                .strong()
                                .size(13.0),
                        );
                        ui.label(
                            egui::RichText::new(self.t("play_disabled_help"))
                                .color(egui::Color32::from_rgb(210, 210, 215))
                                .size(11.0),
                        );
                    }
                });
            } else {
                // ==========================================
                // TAB 2: CONFIGURATION & CONTROLS
                // ==========================================

                // --- 1. DISPLAY MODE & SCREEN SETTINGS ---
                card_frame.show(ui, |ui| {
                    ui.heading(
                        egui::RichText::new(self.t("display_header"))
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 205, 255)),
                    );
                    ui.separator();

                    let mode_borderless = self.t("display_borderless");
                    let mode_fullscreen = self.t("display_fullscreen");
                    let mode_windowed = self.t("display_windowed");

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.t("display_mode")).strong());

                        let current_display_text = match self.config.display_mode {
                            DisplayMode::Borderless => mode_borderless,
                            DisplayMode::Fullscreen => mode_fullscreen,
                            DisplayMode::Windowed => mode_windowed,
                        };

                        egui::ComboBox::from_id_salt("display_mode_combo")
                            .selected_text(current_display_text)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.config.display_mode,
                                    DisplayMode::Borderless,
                                    mode_borderless,
                                );
                                ui.selectable_value(
                                    &mut self.config.display_mode,
                                    DisplayMode::Fullscreen,
                                    mode_fullscreen,
                                );
                                ui.selectable_value(
                                    &mut self.config.display_mode,
                                    DisplayMode::Windowed,
                                    mode_windowed,
                                );
                            });
                    });
                    ui.label(
                        egui::RichText::new(self.t("display_desc"))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(11.0),
                    );

                    ui.add_space(8.0);
                    // V-Sync Toggle
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.vsync);
                        let vsync_label = self.t("vsync");
                        if ui.add(egui::Label::new(egui::RichText::new(vsync_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.vsync = !self.config.vsync;
                        }
                    });
                    ui.label(
                        egui::RichText::new(self.t("vsync_desc"))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(11.0),
                    );
                });

                // --- 2. GAMEPAD / CONTROLLER SETTINGS ---
                card_frame.show(ui, |ui| {
                    ui.heading(
                        egui::RichText::new(self.t("controller_header"))
                            .size(16.0)
                            .color(egui::Color32::from_rgb(150, 205, 255)),
                    );
                    ui.separator();

                    // Background input toggle
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.background_controller_input);
                        let bg_lbl = self.t("bg_input");
                        if ui.add(egui::Label::new(egui::RichText::new(bg_lbl).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.background_controller_input = !self.config.background_controller_input;
                        }
                    });
                    ui.label(
                        egui::RichText::new(self.t("bg_input_desc"))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(11.0),
                    );

                    ui.add_space(8.0);
                    // Motion controls toggle
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.is_motion_controls_enabled);
                        let motion_lbl = self.t("motion_controls");
                        if ui.add(egui::Label::new(egui::RichText::new(motion_lbl).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.is_motion_controls_enabled = !self.config.is_motion_controls_enabled;
                        }
                    });
                    ui.label(
                        egui::RichText::new(self.t("motion_controls_desc"))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(11.0),
                    );

                    ui.add_space(8.0);
                    // Analog Stick Deadzones
                    ui.columns(2, |cols| {
                        cols[0].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("deadzone_left")).strong());
                            ui.add(egui::Slider::new(&mut self.config.deadzone_left, 1..=40).text(""));
                        });
                        cols[1].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("deadzone_right")).strong());
                            ui.add(egui::Slider::new(&mut self.config.deadzone_right, 1..=40).text(""));
                        });
                    });
                    ui.label(
                        egui::RichText::new(self.t("deadzone_desc"))
                            .color(egui::Color32::LIGHT_GRAY)
                            .size(11.0),
                    );
                });

                // --- 3. KEYBOARD BINDINGS (INTERACTIVE & CUSTOMIZABLE) ---
                card_frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.heading(
                            egui::RichText::new(self.t("keyboard_header"))
                                .size(16.0)
                                .color(egui::Color32::from_rgb(150, 205, 255)),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(egui::RichText::new(self.t("reset_defaults")).strong()).clicked() {
                                self.config.keyboard_bindings = KeyboardBindings::wasd_cvbn();
                            }

                            // Preset selection buttons
                            if ui.button(self.t("preset_numpad")).clicked() {
                                self.config.keyboard_bindings = KeyboardBindings::wasd_numpad();
                            }
                            if ui.button(self.t("preset_standard")).clicked() {
                                self.config.keyboard_bindings = KeyboardBindings::wasd_cvbn();
                            }
                            ui.label(self.t("preset_label"));
                        });
                    });
                    ui.separator();

                    ui.label(
                        egui::RichText::new(self.t("keyboard_desc"))
                            .color(egui::Color32::from_rgb(255, 215, 100))
                            .size(11.5),
                    );
                    ui.add_space(6.0);

                    let mut next_listening = self.listening_key_for;
                    let cur_listening = self.listening_key_for;
                    let kb = &self.config.keyboard_bindings;

                    ui.columns(2, |cols| {
                        // Left Column: Action Buttons & Shoulder Triggers
                        cols[0].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("sec_face_buttons")).color(egui::Color32::from_rgb(255, 150, 150)).strong());
                            render_binding_row(ui, self.t("act_triangle"), KeyBindingTarget::Triangle, &kb.triangle, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_square"), KeyBindingTarget::Square, &kb.square, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_circle"), KeyBindingTarget::Circle, &kb.circle, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_cross"), KeyBindingTarget::Cross, &kb.cross, cur_listening, &mut next_listening);

                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(self.t("sec_triggers")).color(egui::Color32::from_rgb(255, 150, 150)).strong());
                            render_binding_row(ui, self.t("act_l1"), KeyBindingTarget::L1, &kb.l1, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_r1"), KeyBindingTarget::R1, &kb.r1, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_l2"), KeyBindingTarget::L2, &kb.l2, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_r2"), KeyBindingTarget::R2, &kb.r2, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_l3"), KeyBindingTarget::L3, &kb.l3, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_r3"), KeyBindingTarget::R3, &kb.r3, cur_listening, &mut next_listening);
                        });

                        // Right Column: D-Pad / Menu & Analog Sticks
                        cols[1].vertical(|ui| {
                            ui.label(egui::RichText::new(self.t("sec_dpad_menu")).color(egui::Color32::from_rgb(255, 150, 150)).strong());
                            render_binding_row(ui, self.t("act_options"), KeyBindingTarget::Options, &kb.options, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_touchpad"), KeyBindingTarget::TouchpadCenter, &kb.touchpad_center, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_dpad_up"), KeyBindingTarget::PadUp, &kb.pad_up, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_dpad_down"), KeyBindingTarget::PadDown, &kb.pad_down, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_dpad_left"), KeyBindingTarget::PadLeft, &kb.pad_left, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_dpad_right"), KeyBindingTarget::PadRight, &kb.pad_right, cur_listening, &mut next_listening);

                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(self.t("sec_analog_sticks")).color(egui::Color32::from_rgb(255, 150, 150)).strong());
                            render_binding_row(ui, self.t("act_move_up"), KeyBindingTarget::AxisLeftYMinus, &kb.axis_left_y_minus, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_move_down"), KeyBindingTarget::AxisLeftYPlus, &kb.axis_left_y_plus, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_move_left"), KeyBindingTarget::AxisLeftXMinus, &kb.axis_left_x_minus, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_move_right"), KeyBindingTarget::AxisLeftXPlus, &kb.axis_left_x_plus, cur_listening, &mut next_listening);

                            render_binding_row(ui, self.t("act_cam_up"), KeyBindingTarget::AxisRightYMinus, &kb.axis_right_y_minus, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_cam_down"), KeyBindingTarget::AxisRightYPlus, &kb.axis_right_y_plus, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_cam_left"), KeyBindingTarget::AxisRightXMinus, &kb.axis_right_x_minus, cur_listening, &mut next_listening);
                            render_binding_row(ui, self.t("act_cam_right"), KeyBindingTarget::AxisRightXPlus, &kb.axis_right_x_plus, cur_listening, &mut next_listening);
                        });
                    });

                    self.listening_key_for = next_listening;
                });

                // --- 4. CONFIG ACTION AREA ---
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(
                        egui::RichText::new(self.t("save_apply"))
                            .size(15.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(45, 135, 75))
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(220.0, 42.0));

                    if ui.add(save_btn).clicked() {
                        self.apply_all_optimizations();
                    }

                    if ui.button(egui::RichText::new(self.t("view_log")).size(14.0)).clicked() {
                        let log_path = self.base_dir.join("gow3_log.txt");
                        self.log_content = std::fs::read_to_string(&log_path)
                            .unwrap_or_else(|_| "Log file is empty or not created yet.".to_string());
                        self.show_log_modal = true;
                    }
                });
            }

            ui.add_space(10.0);
        });

        // --- MODAL 1: CANDIDATES SELECTION (WHEN AUTO SCAN FINISHES) ---
        if let Some(ref candidates) = self.found_candidates_modal.clone() {
            egui::Window::new(format!("{} ({} adet)", self.t("candidates_modal_title"), candidates.len()))
                .collapsible(false)
                .resizable(true)
                .default_width(620.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    ui.label(
                        egui::RichText::new("Bulunan oyun klasörlerinden kullanmak istediğinizi seçin:")
                            .size(14.0)
                            .strong(),
                    );
                    ui.separator();

                    egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                        for (i, c) in candidates.iter().enumerate() {
                            egui::Frame::group(ui.style())
                                .fill(egui::Color32::from_rgb(28, 28, 36))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(egui::RichText::new(format!("#{}:", i + 1)).strong());
                                        ui.vertical(|ui| {
                                            ui.label(egui::RichText::new(&c.title).color(egui::Color32::from_rgb(150, 205, 255)).strong());
                                            ui.label(format!("ID: {} | Sürüm: {} | eboot.bin: {:.2} MB", c.title_id, c.version, c.eboot_size_mb));
                                            ui.label(egui::RichText::new(c.path.display().to_string()).monospace().size(11.0));
                                        });

                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button(egui::RichText::new(self.t("select_this")).color(egui::Color32::from_rgb(105, 225, 110)).strong()).clicked() {
                                                self.set_game_directory(&c.path, &ctx);
                                                self.found_candidates_modal = None;
                                                let selected_name = c.title.clone();
                                                self.status_message = Some((format!("✅ {} seçildi ve kaydedildi!", selected_name), false));
                                            }
                                        });
                                    });
                                });
                            ui.add_space(4.0);
                        }
                    });

                    ui.add_space(6.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(self.t("close")).clicked() {
                            self.found_candidates_modal = None;
                        }
                    });
                });
        }

        // --- MODAL: GAME NOT FOUND WARNING ---
        if self.show_game_not_found_modal {
            egui::Window::new(self.t("game_not_found_modal_title"))
                .collapsible(false)
                .resizable(false)
                .default_width(520.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("⚠️")
                                .size(36.0)
                                .color(egui::Color32::from_rgb(255, 175, 50)),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(self.t("game_not_found_title"))
                                .size(17.0)
                                .strong()
                                .color(egui::Color32::from_rgb(255, 105, 105)),
                        );
                        ui.add_space(6.0);
                    });

                    let box_frame = egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgba_unmultiplied(35, 22, 22, 225))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(230, 60, 60, 110)))
                        .inner_margin(12.0)
                        .corner_radius(egui::CornerRadius::same(8));

                    box_frame.show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(self.t("game_not_found_desc"))
                                .size(13.0)
                                .color(egui::Color32::from_rgb(225, 225, 230)),
                        );
                    });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let manual_btn = egui::Button::new(
                            egui::RichText::new(self.t("manual_select"))
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(45, 105, 205))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(130.0, 32.0));

                        if ui.add(manual_btn).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.set_game_directory(&folder, &ctx);
                                self.show_game_not_found_modal = false;
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_btn = egui::Button::new(self.t("close"))
                                .min_size(egui::vec2(90.0, 32.0));
                            if ui.add(close_btn).clicked() {
                                self.show_game_not_found_modal = false;
                            }
                        });
                    });
                    ui.add_space(4.0);
                });
        }

        // --- MODAL 2: FIRST LAUNCH SETUP ---
        if self.show_first_launch_modal {
            egui::Window::new(self.t("first_launch_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                    ui.label(egui::RichText::new(self.t("first_launch_prompt")).size(14.0));

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button(egui::RichText::new(self.t("first_launch_auto")).color(egui::Color32::from_rgb(105, 225, 110)).strong()).clicked() {
                            self.config.first_launch_done = true;
                            self.show_first_launch_modal = false;
                            let _ = self.config.save(&self.base_dir);
                            self.start_game_scan();
                            self.apply_all_optimizations();
                        }

                        if ui.button(self.t("first_launch_manual")).clicked() {
                            self.config.first_launch_done = true;
                            self.show_first_launch_modal = false;
                            let _ = self.config.save(&self.base_dir);
                        }
                    });
                });
        }

        // --- MODAL 3: CRASH / TEXTURE ERROR DIAGNOSIS POP-UP ---
        if let Some(ref diagnosis) = self.active_crash_diagnosis.clone() {
            egui::Window::new(self.t("diagnosis_title"))
                .collapsible(false)
                .resizable(true)
                .default_width(520.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    ui.heading(egui::RichText::new(&diagnosis.title).color(egui::Color32::from_rgb(245, 90, 85)));
                    ui.separator();

                    ui.label(egui::RichText::new(self.t("diagnosis_desc")).strong());
                    ui.label(&diagnosis.description);

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(self.t("diagnosis_solution")).color(egui::Color32::from_rgb(255, 213, 79)).strong());
                    ui.label(&diagnosis.recommendation);

                    ui.add_space(4.0);
                    ui.collapsing(self.t("diagnosis_log"), |ui| {
                        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            ui.label(egui::RichText::new(&diagnosis.log_snippet).monospace().size(11.0));
                        });
                    });

                    ui.add_space(10.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new(self.t("diagnosis_ack")).strong()).clicked() {
                            self.active_crash_diagnosis = None;
                        }
                    });
                });
        }

        // --- MODAL 4: LOG MODAL ---
        if self.show_log_modal {
            egui::Window::new("📜 gow3_log.txt")
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(600.0, 400.0))
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(egui::RichText::new(&self.log_content).monospace().size(11.0));
                    });
                    if ui.button(self.t("close")).clicked() {
                        self.show_log_modal = false;
                    }
                });
        }

        // --- MODAL 5: UPDATE CHECKER & INTEGRATION MODAL ---
        if let Some(info) = self.update_modal.clone() {
            let title = if info.is_new {
                self.t("update_modal_title")
            } else {
                self.t("up_to_date_title")
            };

            egui::Window::new(title)
                .collapsible(false)
                .resizable(false)
                .default_width(520.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);

                    if info.is_new {
                        ui.heading(
                            egui::RichText::new(self.t("update_available_notice"))
                                .color(egui::Color32::from_rgb(105, 225, 110))
                                .size(16.0),
                        );
                    } else {
                        ui.heading(
                            egui::RichText::new(self.t("up_to_date_notice"))
                                .color(egui::Color32::from_rgb(100, 200, 255))
                                .size(16.0),
                        );
                    }

                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(26, 27, 34))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(12.0)
                        .show(ui, |ui| {
                            let short_sha = if info.commit_sha.len() > 7 {
                                &info.commit_sha[..7]
                            } else {
                                &info.commit_sha
                            };

                            ui.label(
                                egui::RichText::new(format!("{} {}", self.t("latest_commit"), short_sha))
                                    .color(egui::Color32::from_rgb(255, 215, 100))
                                    .strong(),
                            );
                            ui.label(egui::RichText::new(&info.commit_message).italics());

                            if !info.commit_date.is_empty() {
                                ui.label(format!("{} {}", self.t("commit_date"), info.commit_date));
                            }
                        });

                    ui.label(
                        egui::RichText::new(self.t("update_backup_info"))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(190, 190, 200)),
                    );

                    ui.separator();

                    ui.horizontal(|ui| {
                        let btn_text = if info.is_new {
                            self.t("apply_update")
                        } else {
                            self.t("force_sync")
                        };

                        let apply_btn = egui::Button::new(
                            egui::RichText::new(btn_text)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(45, 135, 75))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(160.0, 32.0));

                        if ui.add(apply_btn).clicked() {
                            match apply_github_update(&self.base_dir, &info, &mut self.config) {
                                Ok(success_msg) => {
                                    self.status_message = Some((success_msg, false));
                                    self.update_modal = None;
                                }
                                Err(e) => {
                                    self.status_message = Some((format!("❌ Güncelleme hatası: {}", e), true));
                                }
                            }
                        }

                        if ui.button(self.t("view_on_github")).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/ScreamAngel/gow3_pclauncher"));
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(self.t("close")).clicked() {
                                self.update_modal = None;
                            }
                        });
                    });
                });
        }
    }
}
