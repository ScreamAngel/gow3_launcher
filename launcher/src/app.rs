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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    MainMenu,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Graphics,
    Mods,
    SystemSpecs,
    GameLocation,
    Controls,
    Shaders,
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

#[cfg(target_os = "windows")]
pub fn check_physical_numpad() -> Option<&'static str> {
    unsafe extern "system" {
        fn GetAsyncKeyState(vKey: i32) -> i16;
    }
    unsafe {
        let is_down = |vk: i32| -> bool { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 };
        if is_down(0x60) { return Some("kp0"); }
        if is_down(0x61) { return Some("kp1"); }
        if is_down(0x62) { return Some("kp2"); }
        if is_down(0x63) { return Some("kp3"); }
        if is_down(0x64) { return Some("kp4"); }
        if is_down(0x65) { return Some("kp5"); }
        if is_down(0x66) { return Some("kp6"); }
        if is_down(0x67) { return Some("kp7"); }
        if is_down(0x68) { return Some("kp8"); }
        if is_down(0x69) { return Some("kp9"); }
        if is_down(0x6A) { return Some("kpasterisk"); }
        if is_down(0x6B) { return Some("kpplus"); }
        if is_down(0x6D) { return Some("kpminus"); }
        if is_down(0x6E) { return Some("kpperiod"); }
        if is_down(0x6F) { return Some("kpslash"); }
    }
    None
}

#[cfg(target_os = "linux")]
pub fn check_physical_numpad() -> Option<&'static str> {
    use std::ffi::{c_char, c_void};
    use std::sync::atomic::{AtomicBool, Ordering};

    static X11_FAILED: AtomicBool = AtomicBool::new(false);
    if X11_FAILED.load(Ordering::Relaxed) {
        return None;
    }

    type XOpenDisplayFn = unsafe extern "C" fn(*const c_char) -> *mut c_void;
    type XCloseDisplayFn = unsafe extern "C" fn(*mut c_void) -> i32;
    type XQueryKeymapFn = unsafe extern "C" fn(*mut c_void, *mut u8) -> i32;

    unsafe {
        let lib = libc::dlopen(b"libX11.so.6\0".as_ptr() as *const c_char, libc::RTLD_LAZY);
        if lib.is_null() {
            X11_FAILED.store(true, Ordering::Relaxed);
            return None;
        }

        let open_sym = libc::dlsym(lib, b"XOpenDisplay\0".as_ptr() as *const c_char);
        let close_sym = libc::dlsym(lib, b"XCloseDisplay\0".as_ptr() as *const c_char);
        let query_sym = libc::dlsym(lib, b"XQueryKeymap\0".as_ptr() as *const c_char);

        if open_sym.is_null() || close_sym.is_null() || query_sym.is_null() {
            libc::dlclose(lib);
            X11_FAILED.store(true, Ordering::Relaxed);
            return None;
        }

        let open_disp: XOpenDisplayFn = std::mem::transmute(open_sym);
        let close_disp: XCloseDisplayFn = std::mem::transmute(close_sym);
        let query_keymap: XQueryKeymapFn = std::mem::transmute(query_sym);

        let disp = open_disp(std::ptr::null());
        if disp.is_null() {
            libc::dlclose(lib);
            return None;
        }

        let mut keys = [0u8; 32];
        query_keymap(disp, keys.as_mut_ptr());
        close_disp(disp);
        libc::dlclose(lib);

        let is_pressed = |kc: usize| -> bool {
            (keys[kc / 8] & (1 << (kc % 8))) != 0
        };

        if is_pressed(90) { return Some("kp0"); }
        if is_pressed(87) { return Some("kp1"); }
        if is_pressed(88) { return Some("kp2"); }
        if is_pressed(89) { return Some("kp3"); }
        if is_pressed(83) { return Some("kp4"); }
        if is_pressed(84) { return Some("kp5"); }
        if is_pressed(85) { return Some("kp6"); }
        if is_pressed(79) { return Some("kp7"); }
        if is_pressed(80) { return Some("kp8"); }
        if is_pressed(81) { return Some("kp9"); }
        if is_pressed(86) { return Some("kpplus"); }
        if is_pressed(82) { return Some("kpminus"); }
        if is_pressed(63) { return Some("kpasterisk"); }
        if is_pressed(106) { return Some("kpslash"); }
        if is_pressed(91) { return Some("kpperiod"); }
        if is_pressed(104) { return Some("kpenter"); }
    }
    None
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
pub fn check_physical_numpad() -> Option<&'static str> {
    None
}

pub fn apply_binding_to_config(config: &mut LauncherConfig, target: KeyBindingTarget, new_key: String) {
    match target {
        KeyBindingTarget::Triangle => config.keyboard_bindings.triangle = new_key,
        KeyBindingTarget::Circle => config.keyboard_bindings.circle = new_key,
        KeyBindingTarget::Cross => config.keyboard_bindings.cross = new_key,
        KeyBindingTarget::Square => config.keyboard_bindings.square = new_key,
        KeyBindingTarget::L1 => config.keyboard_bindings.l1 = new_key,
        KeyBindingTarget::R1 => config.keyboard_bindings.r1 = new_key,
        KeyBindingTarget::L2 => config.keyboard_bindings.l2 = new_key,
        KeyBindingTarget::R2 => config.keyboard_bindings.r2 = new_key,
        KeyBindingTarget::L3 => config.keyboard_bindings.l3 = new_key,
        KeyBindingTarget::R3 => config.keyboard_bindings.r3 = new_key,
        KeyBindingTarget::Options => config.keyboard_bindings.options = new_key,
        KeyBindingTarget::TouchpadCenter => config.keyboard_bindings.touchpad_center = new_key,
        KeyBindingTarget::PadUp => config.keyboard_bindings.pad_up = new_key,
        KeyBindingTarget::PadDown => config.keyboard_bindings.pad_down = new_key,
        KeyBindingTarget::PadLeft => config.keyboard_bindings.pad_left = new_key,
        KeyBindingTarget::PadRight => config.keyboard_bindings.pad_right = new_key,
        KeyBindingTarget::AxisLeftYMinus => config.keyboard_bindings.axis_left_y_minus = new_key,
        KeyBindingTarget::AxisLeftYPlus => config.keyboard_bindings.axis_left_y_plus = new_key,
        KeyBindingTarget::AxisLeftXMinus => config.keyboard_bindings.axis_left_x_minus = new_key,
        KeyBindingTarget::AxisLeftXPlus => config.keyboard_bindings.axis_left_x_plus = new_key,
        KeyBindingTarget::AxisRightYMinus => config.keyboard_bindings.axis_right_y_minus = new_key,
        KeyBindingTarget::AxisRightYPlus => config.keyboard_bindings.axis_right_y_plus = new_key,
        KeyBindingTarget::AxisRightXMinus => config.keyboard_bindings.axis_right_x_minus = new_key,
        KeyBindingTarget::AxisRightXPlus => config.keyboard_bindings.axis_right_x_plus = new_key,
    }
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
    match key_str.trim().to_lowercase().as_str() {
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
        "kpplus" | "kp_plus" => "Numpad +".to_string(),
        "kpminus" | "kp_minus" => "Numpad -".to_string(),
        "kpasterisk" | "kp_multiply" => "Numpad *".to_string(),
        "kpslash" | "kp_divide" => "Numpad /".to_string(),
        "kpenter" | "kp_enter" => "Numpad Enter".to_string(),
        "kpperiod" | "kp_period" => "Numpad .".to_string(),
        "kpcomma" => "Numpad ,".to_string(),
        "kpequals" => "Numpad =".to_string(),

        // Mouse buttons
        "leftbutton" | "mouse1" => "🖱 Fare Sol Tık".to_string(),
        "rightbutton" | "mouse2" => "🖱 Fare Sağ Tık".to_string(),
        "middlebutton" | "mouse3" => "🖱 Fare Orta Tık".to_string(),
        "sidebuttonback" | "mouse4" => "🖱 Yan 1 (Geri)".to_string(),
        "sidebuttonforward" | "mouse5" => "🖱 Yan 2 (İleri)".to_string(),
        "mousewheelup" => "🖱 Tekerlek Yukarı".to_string(),
        "mousewheeldown" => "🖱 Tekerlek Aşağı".to_string(),

        // Direction & Navigation
        "up" => "Yukarı (Up)".to_string(),
        "down" => "Aşağı (Down)".to_string(),
        "left" => "Sol (Left)".to_string(),
        "right" => "Sağ (Right)".to_string(),
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

#[allow(dead_code)]
fn render_hero_pill_badge(
    ui: &mut egui::Ui,
    icon: Option<&egui::TextureHandle>,
    text: &str,
    is_crimson: bool,
    is_pulsing_glow: bool,
) {
    let (bg, border, text_col) = if is_crimson {
        (
            egui::Color32::from_rgba_unmultiplied(220, 38, 38, 40),
            egui::Color32::from_rgba_unmultiplied(239, 68, 68, 180),
            egui::Color32::from_rgb(255, 110, 110),
        )
    } else {
        (
            egui::Color32::from_rgba_unmultiplied(24, 26, 36, 220),
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30),
            egui::Color32::from_rgb(205, 210, 225),
        )
    };

    let font_id = egui::FontId::proportional(11.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font_id, text_col);
    let icon_size = if icon.is_some() { 13.0 } else { 0.0 };
    let icon_gap = if icon.is_some() { 6.0 } else { 0.0 };
    let padding = egui::vec2(22.0, 10.0);
    let content_width = icon_size + icon_gap + galley.rect.width();
    let desired_size = egui::vec2(content_width + padding.x, galley.rect.height().max(icon_size) + padding.y);

    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter();

    // Glowing aura if release is newer or update available
    if is_pulsing_glow {
        let time = ui.ctx().input(|i| i.time);
        let pulse = ((time * 3.6).sin() * 0.5 + 0.5) as f32;
        ui.ctx().request_repaint();

        painter.rect_filled(
            rect.expand(2.0 + 3.0 * pulse),
            14.0,
            egui::Color32::from_rgba_unmultiplied(239, 68, 68, (50.0 + 75.0 * pulse) as u8),
        );
    }

    let is_hovered = response.hovered();
    let hover_anim = ui.ctx().animate_bool(response.id, is_hovered);
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let current_bg = if hover_anim > 0.01 {
        egui::Color32::from_rgb(
            egui::lerp((bg.r() as f32)..=50.0, hover_anim) as u8,
            egui::lerp((bg.g() as f32)..=32.0, hover_anim) as u8,
            egui::lerp((bg.b() as f32)..=38.0, hover_anim) as u8,
        )
    } else {
        bg
    };

    let stroke = egui::Stroke::new(
        1.0 + 0.5 * hover_anim,
        if hover_anim > 0.01 {
            egui::Color32::from_rgb(255, 110, 110)
        } else {
            border
        },
    );

    let draw_rect = if hover_anim > 0.01 {
        rect.expand(0.8 * hover_anim)
    } else {
        rect
    };

    painter.rect_filled(draw_rect, 12.0, current_bg);
    painter.rect_stroke(draw_rect, 12.0, stroke, egui::StrokeKind::Inside);

    let start_x = draw_rect.left() + (draw_rect.width() - content_width) / 2.0;
    let mut current_x = start_x;

    if let Some(tex) = icon {
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(current_x, draw_rect.center().y - icon_size / 2.0),
            egui::vec2(icon_size, icon_size),
        );
        let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(tex.id(), icon_rect, uv, egui::Color32::WHITE);
        current_x += icon_size + icon_gap;
    }

    let text_pos = egui::pos2(
        current_x,
        draw_rect.top() + (draw_rect.height() - galley.rect.height()) / 2.0,
    );
    painter.galley(text_pos, galley, text_col);
}

#[allow(dead_code)]
fn render_nav_item(
    ui: &mut egui::Ui,
    id_str: &str,
    icon: &str,
    label: &str,
    is_active: bool,
    is_child: bool,
) -> bool {
    let id = ui.make_persistent_id(id_str);
    let height = if is_child { 32.0 } else { 38.0 };
    let font_size = if is_child { 12.0 } else { 13.5 };
    let desired_size = egui::vec2(ui.available_width(), height);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let is_hovered = response.hovered();
    let hover_anim = ui.ctx().animate_bool(id.with("hov"), is_hovered);
    let active_anim = ui.ctx().animate_bool(id.with("act"), is_active);

    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let is_pressed = response.is_pointer_button_down_on();
    let scale_expand = if is_pressed { -1.0 } else { hover_anim * 1.5 };
    let draw_rect = rect.expand(scale_expand);

    let painter = ui.painter();

    // Background color smoothly interpolated
    let bg_color = if active_anim > 0.01 {
        egui::Color32::from_rgba_unmultiplied(
            egui::lerp(18.0..=65.0, active_anim) as u8,
            egui::lerp(20.0..=20.0, active_anim) as u8,
            egui::lerp(28.0..=26.0, active_anim) as u8,
            egui::lerp(140.0..=240.0, active_anim) as u8,
        )
    } else if hover_anim > 0.01 {
        egui::Color32::from_rgba_unmultiplied(
            egui::lerp(18.0..=45.0, hover_anim) as u8,
            egui::lerp(20.0..=26.0, hover_anim) as u8,
            egui::lerp(28.0..=34.0, hover_anim) as u8,
            egui::lerp(100.0..=200.0, hover_anim) as u8,
        )
    } else {
        egui::Color32::from_rgba_unmultiplied(18, 20, 28, 120)
    };

    // Border stroke
    let border_stroke = if active_anim > 0.01 {
        egui::Stroke::new(
            1.2,
            egui::Color32::from_rgba_unmultiplied(
                235,
                egui::lerp(68.0..=180.0, active_anim) as u8,
                egui::lerp(68.0..=75.0, active_anim) as u8,
                220,
            ),
        )
    } else if hover_anim > 0.01 {
        egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(239, 68, 68, (120.0 + 130.0 * hover_anim) as u8),
        )
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 18))
    };

    painter.rect_filled(draw_rect, 6.0, bg_color);
    painter.rect_stroke(draw_rect, 6.0, border_stroke, egui::StrokeKind::Inside);

    // Left indicator accent bar (Golden flame for active, fiery red for hover)
    if active_anim > 0.05 || hover_anim > 0.05 {
        let bar_width = 3.5;
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(draw_rect.min.x + 1.0, draw_rect.min.y + 4.0),
            egui::pos2(draw_rect.min.x + 1.0 + bar_width, draw_rect.max.y - 4.0),
        );
        let bar_color = if active_anim > 0.05 {
            egui::Color32::from_rgb(245, 190, 80)
        } else {
            egui::Color32::from_rgb(239, 68, 68)
        };
        painter.rect_filled(bar_rect, 2.0, bar_color);
    }

    // Text & icon
    let indent = if is_child { 18.0 } else { 12.0 };
    let text_color = if active_anim > 0.01 {
        egui::Color32::from_rgb(255, 225, 130)
    } else if hover_anim > 0.01 {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(195, 200, 215)
    };

    let full_text = if icon.is_empty() {
        label.to_string()
    } else {
        format!("{} {}", icon, label)
    };

    let font_id = egui::FontId::proportional(font_size);
    let galley = painter.layout_no_wrap(full_text, font_id, text_color);
    let text_pos = egui::pos2(
        draw_rect.min.x + indent,
        draw_rect.center().y - galley.rect.height() / 2.0,
    );
    painter.galley(text_pos, galley, text_color);

    response.clicked()
}

fn render_console_action_button(
    ui: &mut egui::Ui,
    id_source: &str,
    icon: &str,
    label: &str,
    font_size: f32,
    is_gold_accent: bool,
) -> bool {
    let id = ui.make_persistent_id(id_source);
    let full_text = format!("{} {}", icon, label);
    let font_id = egui::FontId::proportional(font_size);
    let galley = ui.painter().layout_no_wrap(full_text.clone(), font_id, egui::Color32::WHITE);
    let padding = egui::vec2(16.0, 8.0);
    let desired_size = egui::vec2(galley.rect.width() + padding.x * 2.0, font_size + padding.y * 2.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let is_hovered = resp.hovered();
    let hov_anim = ui.ctx().animate_bool(id.with("hov"), is_hovered);
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let is_pressed = resp.is_pointer_button_down_on();
    let scale = if is_pressed { -1.0 } else { hov_anim * 1.5 };
    let draw_rect = rect.expand(scale);
    let painter = ui.painter();

    let bg_color = egui::Color32::from_rgba_unmultiplied(
        (12.0 + 30.0 * hov_anim) as u8,
        (14.0 + 20.0 * hov_anim) as u8,
        (20.0 + 15.0 * hov_anim) as u8,
        (100.0 + 100.0 * hov_anim) as u8,
    );
    let border_stroke = if hov_anim > 0.01 {
        egui::Stroke::new(
            1.2 + 0.5 * hov_anim,
            if is_gold_accent {
                egui::Color32::from_rgba_unmultiplied(255, 215, 100, (140.0 + 115.0 * hov_anim) as u8)
            } else {
                egui::Color32::from_rgba_unmultiplied(239, 68, 68, (140.0 + 115.0 * hov_anim) as u8)
            },
        )
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 35))
    };

    painter.rect_filled(draw_rect, 6.0, bg_color);
    painter.rect_stroke(draw_rect, 6.0, border_stroke, egui::StrokeKind::Inside);

    let text_col = if is_hovered {
        if is_gold_accent {
            egui::Color32::from_rgb(255, 225, 120)
        } else {
            egui::Color32::WHITE
        }
    } else {
        egui::Color32::from_rgb(215, 220, 230)
    };

    let text_pos = egui::pos2(
        draw_rect.min.x + padding.x,
        draw_rect.center().y - galley.rect.height() / 2.0,
    );
    painter.galley(text_pos, galley, text_col);

    resp.clicked()
}

fn render_settings_tab_btn(
    ui: &mut egui::Ui,
    id_source: &str,
    icon: &str,
    label: &str,
    is_active: bool,
) -> bool {
    let id = ui.make_persistent_id(id_source);
    let full_text = format!("{} {}", icon, label);
    let font_id = egui::FontId::proportional(14.0);
    let galley = ui.painter().layout_no_wrap(full_text.clone(), font_id, egui::Color32::WHITE);
    let desired_size = egui::vec2(galley.rect.width() + 28.0, 36.0);
    let (rect, resp) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    let is_hovered = resp.hovered();
    let hov_anim = ui.ctx().animate_bool(id.with("hov"), is_hovered);
    let act_anim = ui.ctx().animate_bool(id.with("act"), is_active);
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let is_pressed = resp.is_pointer_button_down_on();
    let scale = if is_pressed { -1.0 } else { hov_anim * 1.5 };
    let draw_rect = rect.expand(scale);
    let painter = ui.painter();

    let bg_color = if act_anim > 0.01 {
        egui::Color32::from_rgba_unmultiplied(
            egui::lerp(20.0..=180.0, act_anim) as u8,
            egui::lerp(22.0..=35.0, act_anim) as u8,
            egui::lerp(30.0..=35.0, act_anim) as u8,
            egui::lerp(120.0..=230.0, act_anim) as u8,
        )
    } else if hov_anim > 0.01 {
        egui::Color32::from_rgba_unmultiplied(35, 38, 52, (180.0 * hov_anim) as u8)
    } else {
        egui::Color32::from_rgba_unmultiplied(16, 18, 25, 140)
    };

    let border_stroke = if act_anim > 0.01 {
        egui::Stroke::new(1.4, egui::Color32::from_rgb(255, 215, 100))
    } else if hov_anim > 0.01 {
        egui::Stroke::new(1.0, egui::Color32::from_rgb(239, 68, 68))
    } else {
        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 30))
    };

    painter.rect_filled(draw_rect, 7.0, bg_color);
    painter.rect_stroke(draw_rect, 7.0, border_stroke, egui::StrokeKind::Inside);

    let text_col = if is_active {
        egui::Color32::WHITE
    } else if is_hovered {
        egui::Color32::from_rgb(255, 225, 120)
    } else {
        egui::Color32::from_rgb(190, 195, 210)
    };

    let text_pos = egui::pos2(
        draw_rect.center().x - galley.rect.width() / 2.0,
        draw_rect.center().y - galley.rect.height() / 2.0,
    );
    painter.galley(text_pos, galley, text_col);

    resp.clicked()
}

fn paint_turkish_flag(painter: &egui::Painter, rect: egui::Rect) {
    // Red field
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(227, 10, 23));

    let h = rect.height();
    let cy = rect.center().y;

    // Crescent (Hilal)
    // Outer white circle
    let c1_x = rect.left() + rect.width() * 0.38;
    let r1 = h * 0.30;
    painter.circle_filled(egui::pos2(c1_x, cy), r1, egui::Color32::WHITE);

    // Inner red circle cutting the crescent
    let c2_x = rect.left() + rect.width() * 0.44;
    let r2 = h * 0.24;
    painter.circle_filled(egui::pos2(c2_x, cy), r2, egui::Color32::from_rgb(227, 10, 23));

    // Star (Yıldız) - 5-pointed star
    let star_cx = rect.left() + rect.width() * 0.68;
    let star_r_outer = h * 0.16;
    let star_r_inner = star_r_outer * 0.42;

    let mut star_points = Vec::with_capacity(10);
    for i in 0..10 {
        let angle = std::f32::consts::PI * (i as f32) / 5.0 - std::f32::consts::PI / 2.0;
        let r = if i % 2 == 0 { star_r_outer } else { star_r_inner };
        star_points.push(egui::pos2(star_cx + r * angle.cos(), cy + r * angle.sin()));
    }
    painter.add(egui::Shape::convex_polygon(
        star_points,
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
}

fn paint_uk_flag(painter: &egui::Painter, rect: egui::Rect) {
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(1, 33, 105));
    let w = rect.width();
    let h = rect.height();

    // White diagonal saltire
    painter.line_segment([rect.left_top(), rect.right_bottom()], egui::Stroke::new(h * 0.28, egui::Color32::WHITE));
    painter.line_segment([rect.left_bottom(), rect.right_top()], egui::Stroke::new(h * 0.28, egui::Color32::WHITE));

    // Red diagonal saltire
    painter.line_segment([rect.left_top(), rect.right_bottom()], egui::Stroke::new(h * 0.14, egui::Color32::from_rgb(200, 16, 46)));
    painter.line_segment([rect.left_bottom(), rect.right_top()], egui::Stroke::new(h * 0.14, egui::Color32::from_rgb(200, 16, 46)));

    // White central cross
    let cross_w = w * 0.32;
    let cross_h = h * 0.32;
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(rect.center().x - cross_w / 2.0, rect.min.y), egui::pos2(rect.center().x + cross_w / 2.0, rect.max.y)),
        0.0,
        egui::Color32::WHITE,
    );
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.center().y - cross_h / 2.0), egui::pos2(rect.max.x, rect.center().y + cross_h / 2.0)),
        0.0,
        egui::Color32::WHITE,
    );

    // Red central cross
    let red_w = w * 0.18;
    let red_h = h * 0.18;
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(rect.center().x - red_w / 2.0, rect.min.y), egui::pos2(rect.center().x + red_w / 2.0, rect.max.y)),
        0.0,
        egui::Color32::from_rgb(200, 16, 46),
    );
    painter.rect_filled(
        egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.center().y - red_h / 2.0), egui::pos2(rect.max.x, rect.center().y + red_h / 2.0)),
        0.0,
        egui::Color32::from_rgb(200, 16, 46),
    );
}

fn paint_german_flag(painter: &egui::Painter, rect: egui::Rect) {
    let h = rect.height() / 3.0;
    // Top stripe: Black
    let r1 = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + h));
    painter.rect_filled(r1, egui::CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, egui::Color32::from_rgb(10, 10, 10));
    // Mid stripe: Red
    let r2 = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + h), egui::pos2(rect.max.x, rect.min.y + h * 2.0));
    painter.rect_filled(r2, 0.0, egui::Color32::from_rgb(221, 0, 0));
    // Bottom stripe: Gold
    let r3 = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + h * 2.0), rect.max);
    painter.rect_filled(r3, egui::CornerRadius { nw: 0, ne: 0, sw: 2, se: 2 }, egui::Color32::from_rgb(255, 206, 0));
}

fn paint_spanish_flag(painter: &egui::Painter, rect: egui::Rect) {
    let h = rect.height();
    // Top stripe: Red (25%)
    let r1 = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + h * 0.25));
    painter.rect_filled(r1, egui::CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, egui::Color32::from_rgb(170, 21, 27));
    // Mid stripe: Yellow (50%)
    let r2 = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + h * 0.25), egui::pos2(rect.max.x, rect.min.y + h * 0.75));
    painter.rect_filled(r2, 0.0, egui::Color32::from_rgb(241, 191, 0));
    // Bottom stripe: Red (25%)
    let r3 = egui::Rect::from_min_max(egui::pos2(rect.min.x, rect.min.y + h * 0.75), rect.max);
    painter.rect_filled(r3, egui::CornerRadius { nw: 0, ne: 0, sw: 2, se: 2 }, egui::Color32::from_rgb(170, 21, 27));
    // Coat of arms marker in yellow band
    let mark_x = rect.left() + rect.width() * 0.32;
    let mark_cy = rect.center().y;
    painter.rect_filled(egui::Rect::from_center_size(egui::pos2(mark_x, mark_cy), egui::vec2(h * 0.22, h * 0.28)), 1.0, egui::Color32::from_rgb(170, 21, 27));
}

fn render_button_feedback(
    ui: &mut egui::Ui,
    id_source: &str,
    text: &str,
    min_size: egui::Vec2,
    is_danger: bool,
) -> egui::Response {
    let id = ui.make_persistent_id(id_source);
    let font_id = egui::FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font_id.clone(), egui::Color32::WHITE);
    let needed_w = (galley.rect.width() + 28.0).max(min_size.x);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(needed_w, min_size.y), egui::Sense::click());
    let is_hovered = resp.hovered();
    let hov_anim = ui.ctx().animate_bool(id.with("hov"), is_hovered);
    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let is_pressed = resp.is_pointer_button_down_on();
    let scale = if is_pressed { -1.0 } else { hov_anim * 1.5 };
    let draw_rect = rect.expand(scale);

    let (bg, border, text_color) = if is_danger {
        if hov_anim > 0.01 {
            (
                egui::Color32::from_rgb(
                    egui::lerp(45.0..=65.0, hov_anim) as u8,
                    egui::lerp(20.0..=24.0, hov_anim) as u8,
                    egui::lerp(22.0..=26.0, hov_anim) as u8,
                ),
                egui::Color32::from_rgb(239, 68, 68),
                egui::Color32::from_rgb(255, 180, 180),
            )
        } else {
            (
                egui::Color32::from_rgb(45, 20, 22),
                egui::Color32::from_rgba_unmultiplied(239, 68, 68, 80),
                egui::Color32::from_rgb(255, 140, 140),
            )
        }
    } else {
        if hov_anim > 0.01 {
            (
                egui::Color32::from_rgb(
                    egui::lerp(28.0..=45.0, hov_anim) as u8,
                    egui::lerp(30.0..=40.0, hov_anim) as u8,
                    egui::lerp(42.0..=55.0, hov_anim) as u8,
                ),
                egui::Color32::from_rgba_unmultiplied(239, 68, 68, (140.0 * hov_anim) as u8),
                egui::Color32::WHITE,
            )
        } else {
            (
                egui::Color32::from_rgb(28, 30, 42),
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                egui::Color32::from_rgb(200, 205, 220),
            )
        }
    };

    let painter = ui.painter();
    painter.rect_filled(draw_rect, 6.0, bg);
    painter.rect_stroke(draw_rect, 6.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
    painter.text(
        draw_rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(12.0),
        text_color,
    );

    resp
}

fn render_section_tag(ui: &mut egui::Ui, tag: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(tag.to_uppercase())
            .size(16.0)
            .color(egui::Color32::from_rgb(255, 95, 95))
            .strong(),
    );
    ui.add_space(4.0);
}

fn render_stat_tile(
    ui: &mut egui::Ui,
    tag: &str,
    tag_color: egui::Color32,
    title: &str,
    value: &str,
    badge: Option<(&str, egui::Color32)>,
) {
    egui::Frame::group(ui.style())
        .fill(egui::Color32::from_rgba_unmultiplied(16, 17, 25, 235))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 60)))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .corner_radius(egui::CornerRadius::same(8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(tag_color.r(), tag_color.g(), tag_color.b(), 35))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(tag_color.r(), tag_color.g(), tag_color.b(), 120)))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(5, 2))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(tag).color(tag_color).size(11.0).strong());
                    });
                ui.label(egui::RichText::new(title).color(egui::Color32::from_rgb(175, 180, 195)).size(12.5));
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(value).color(egui::Color32::WHITE).strong().size(15.0));
            if let Some((b_text, b_color)) = badge {
                ui.add_space(3.0);
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(b_color.r(), b_color.g(), b_color.b(), 35))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(b_color.r(), b_color.g(), b_color.b(), 110)))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(b_text).color(b_color).size(11.5).strong());
                    });
            }
        });
}

fn paint_ps_badge(ui: &mut egui::Ui, target: KeyBindingTarget) {
    let size = egui::vec2(22.0, 20.0);
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    let center = rect.center();
    let stroke_width = 2.0;

    match target {
        KeyBindingTarget::Triangle => {
            let color = egui::Color32::from_rgb(0, 230, 118); // PS Green
            let r = 7.0;
            let p1 = egui::pos2(center.x, center.y - r);
            let p2 = egui::pos2(center.x + r * 0.866, center.y + r * 0.5);
            let p3 = egui::pos2(center.x - r * 0.866, center.y + r * 0.5);
            painter.line_segment([p1, p2], egui::Stroke::new(stroke_width, color));
            painter.line_segment([p2, p3], egui::Stroke::new(stroke_width, color));
            painter.line_segment([p3, p1], egui::Stroke::new(stroke_width, color));
        }
        KeyBindingTarget::Circle => {
            let color = egui::Color32::from_rgb(255, 61, 0); // PS Red
            painter.circle_stroke(center, 6.0, egui::Stroke::new(stroke_width, color));
        }
        KeyBindingTarget::Cross => {
            let color = egui::Color32::from_rgb(41, 121, 255); // PS Blue
            let r = 5.0;
            painter.line_segment([center + egui::vec2(-r, -r), center + egui::vec2(r, r)], egui::Stroke::new(stroke_width, color));
            painter.line_segment([center + egui::vec2(-r, r), center + egui::vec2(r, -r)], egui::Stroke::new(stroke_width, color));
        }
        KeyBindingTarget::Square => {
            let color = egui::Color32::from_rgb(255, 64, 129); // PS Pink
            let r = 5.5;
            let sq_rect = egui::Rect::from_center_size(center, egui::vec2(r * 2.0, r * 2.0));
            painter.rect_stroke(sq_rect, 1.0, egui::Stroke::new(stroke_width, color), egui::StrokeKind::Inside);
        }
        KeyBindingTarget::L1 | KeyBindingTarget::L2 | KeyBindingTarget::L3 => {
            let badge_rect = egui::Rect::from_center_size(center, egui::vec2(20.0, 15.0));
            painter.rect_filled(badge_rect, 3.0, egui::Color32::from_rgb(55, 45, 20));
            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 215, 0)), egui::StrokeKind::Inside);
            let text = match target {
                KeyBindingTarget::L1 => "L1",
                KeyBindingTarget::L2 => "L2",
                _ => "L3",
            };
            painter.text(center, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(9.0), egui::Color32::from_rgb(255, 220, 80));
        }
        KeyBindingTarget::R1 | KeyBindingTarget::R2 | KeyBindingTarget::R3 => {
            let badge_rect = egui::Rect::from_center_size(center, egui::vec2(20.0, 15.0));
            painter.rect_filled(badge_rect, 3.0, egui::Color32::from_rgb(55, 30, 20));
            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 140, 0)), egui::StrokeKind::Inside);
            let text = match target {
                KeyBindingTarget::R1 => "R1",
                KeyBindingTarget::R2 => "R2",
                _ => "R3",
            };
            painter.text(center, egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(9.0), egui::Color32::from_rgb(255, 180, 80));
        }
        KeyBindingTarget::PadUp | KeyBindingTarget::PadDown | KeyBindingTarget::PadLeft | KeyBindingTarget::PadRight => {
            let color = egui::Color32::from_rgb(180, 210, 255);
            let badge_rect = egui::Rect::from_center_size(center, egui::vec2(20.0, 15.0));
            painter.rect_filled(badge_rect, 3.0, egui::Color32::from_rgb(25, 35, 55));
            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(1.0, color), egui::StrokeKind::Inside);
            let label = match target {
                KeyBindingTarget::PadUp => "D-U",
                KeyBindingTarget::PadDown => "D-D",
                KeyBindingTarget::PadLeft => "D-L",
                _ => "D-R",
            };
            painter.text(center, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(8.0), color);
        }
        KeyBindingTarget::Options | KeyBindingTarget::TouchpadCenter => {
            let badge_rect = egui::Rect::from_center_size(center, egui::vec2(20.0, 14.0));
            painter.rect_filled(badge_rect, 2.0, egui::Color32::from_rgb(35, 38, 48));
            painter.rect_stroke(badge_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(160, 170, 190)), egui::StrokeKind::Inside);
            let label = if target == KeyBindingTarget::Options { "OPT" } else { "PAD" };
            painter.text(center, egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(8.0), egui::Color32::WHITE);
        }
        KeyBindingTarget::AxisLeftYMinus | KeyBindingTarget::AxisLeftYPlus | KeyBindingTarget::AxisLeftXMinus | KeyBindingTarget::AxisLeftXPlus => {
            let badge_rect = egui::Rect::from_center_size(center, egui::vec2(20.0, 14.0));
            painter.rect_filled(badge_rect, 3.0, egui::Color32::from_rgb(20, 40, 55));
            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 180, 240)), egui::StrokeKind::Inside);
            let dir = match target {
                KeyBindingTarget::AxisLeftYMinus => "L-U",
                KeyBindingTarget::AxisLeftYPlus => "L-D",
                KeyBindingTarget::AxisLeftXMinus => "L-L",
                _ => "L-R",
            };
            painter.text(center, egui::Align2::CENTER_CENTER, dir, egui::FontId::proportional(8.0), egui::Color32::from_rgb(140, 220, 255));
        }
        KeyBindingTarget::AxisRightYMinus | KeyBindingTarget::AxisRightYPlus | KeyBindingTarget::AxisRightXMinus | KeyBindingTarget::AxisRightXPlus => {
            let badge_rect = egui::Rect::from_center_size(center, egui::vec2(20.0, 14.0));
            painter.rect_filled(badge_rect, 3.0, egui::Color32::from_rgb(45, 25, 60));
            painter.rect_stroke(badge_rect, 3.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 100, 255)), egui::StrokeKind::Inside);
            let dir = match target {
                KeyBindingTarget::AxisRightYMinus => "R-U",
                KeyBindingTarget::AxisRightYPlus => "R-D",
                KeyBindingTarget::AxisRightXMinus => "R-L",
                _ => "R-R",
            };
            painter.text(center, egui::Align2::CENTER_CENTER, dir, egui::FontId::proportional(8.0), egui::Color32::from_rgb(230, 160, 255));
        }
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
        paint_ps_badge(ui, target);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(label).size(14.5).color(egui::Color32::from_rgb(240, 240, 245)).strong());

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let is_listening = listening_for == Some(target);
            let btn_text = if is_listening {
                "[ Tuşa / Fareye basın... ]".to_string()
            } else {
                format_key_display(current_key)
            };

            let text_color = if is_listening {
                egui::Color32::from_rgb(255, 230, 120)
            } else {
                egui::Color32::WHITE
            };

            let (bg_color, stroke) = if is_listening {
                let time = ui.ctx().input(|i| i.time);
                let pulse = ((time * 4.0).sin() * 0.5 + 0.5) as f32;
                ui.ctx().request_repaint();
                let r = (160.0 + 50.0 * pulse) as u8;
                let stroke_col = egui::Color32::from_rgb(255, (200.0 + 55.0 * pulse) as u8, 80);
                (egui::Color32::from_rgb(r, 45, 45), egui::Stroke::new(1.5, stroke_col))
            } else {
                (egui::Color32::from_rgb(38, 42, 54), egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25)))
            };

            let btn = egui::Button::new(
                egui::RichText::new(btn_text).strong().size(13.5).color(text_color),
            )
            .min_size(egui::vec2(145.0, 28.0))
            .fill(bg_color)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(6));

            let res = ui.add(btn);
            if res.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if res.clicked() {
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
    let desired_size = egui::vec2(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let radius = 0.5 * rect.height();

        // Background transition: dark slate when off -> vibrant spartan red when on
        let off_bg = egui::Color32::from_rgb(46, 50, 62);
        let on_bg = egui::Color32::from_rgb(215, 45, 45);
        let bg_color = egui::Color32::from_rgb(
            egui::lerp((off_bg.r() as f32)..=(on_bg.r() as f32), how_on) as u8,
            egui::lerp((off_bg.g() as f32)..=(on_bg.g() as f32), how_on) as u8,
            egui::lerp((off_bg.b() as f32)..=(on_bg.b() as f32), how_on) as u8,
        );

        let stroke = egui::Stroke::new(
            1.0,
            egui::Color32::from_rgba_unmultiplied(
                255,
                (100.0 * how_on) as u8,
                (100.0 * how_on) as u8,
                (40.0 + 80.0 * how_on) as u8,
            ),
        );
        ui.painter().rect_filled(rect, radius, bg_color);
        ui.painter().rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);

        let padding = 2.5;
        let knob_radius = radius - padding;
        let left_x = rect.left() + radius;
        let right_x = rect.right() - radius;
        let knob_x = egui::lerp(left_x..=right_x, how_on);
        let knob_center = egui::pos2(knob_x, rect.center().y);

        // Knob halo when ON
        if how_on > 0.05 {
            ui.painter().circle_filled(
                knob_center,
                knob_radius + 2.5 * how_on,
                egui::Color32::from_rgba_unmultiplied(255, 80, 70, (45.0 * how_on) as u8),
            );
        }

        let knob_color = egui::Color32::from_rgb(250, 250, 252);
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

    // View Mode & Navigation
    view_mode: ViewMode,
    settings_tab: SettingsTab,
    view_transition_start: std::time::Instant,
    show_release_notes_modal: bool,
    listening_key_for: Option<KeyBindingTarget>,

    // Scanning state
    is_scanning: bool,
    scan_rx: Option<Receiver<Vec<GameCandidate>>>,
    found_candidates_modal: Option<Vec<GameCandidate>>,
    show_game_not_found_modal: bool,
    scan_failed_empty: bool,

    // Updater state
    is_checking_update: bool,
    silent_update_check: bool,
    has_new_update: bool,
    update_rx: Option<Receiver<Result<UpdateInfo, String>>>,
    update_modal: Option<UpdateInfo>,
    update_dismissed_this_session: bool,
    is_applying_update: bool,
    update_apply_rx: Option<Receiver<(Result<String, String>, LauncherConfig)>>,
    cached_update_info: Option<UpdateInfo>,

    // GitHub Release Notes
    github_release_notes: Option<Vec<crate::updater::GithubReleaseNote>>,
    is_fetching_release_notes: bool,
    release_notes_rx: Option<Receiver<Result<Vec<crate::updater::GithubReleaseNote>, String>>>,

    // Game Running & Monitoring State
    is_game_running: bool,
    game_exit_rx: Option<Receiver<Result<i32, String>>>,
    running_pid: Arc<AtomicU32>,

    // Python dependency state
    has_python: bool,
    is_downloading_python: bool,
    python_dl_rx: Option<Receiver<Result<String, String>>>,

    // Engine dependency state
    has_engine: bool,
    is_downloading_engine: bool,
    engine_dl_rx: Option<Receiver<Result<String, String>>>,

    // Shader Cache Pre-caching state
    is_installing_shaders: bool,
    shader_install_status: Option<String>,
    shader_install_rx: Option<Receiver<Result<usize, String>>>,
    cached_shader_count: usize,

    // Modals
    show_first_launch_modal: bool,
    active_crash_diagnosis: Option<CrashDiagnosis>,
    show_log_modal: bool,
    log_content: String,

    // Error & Diagnostics Modal
    error_modal: Option<ErrorModalData>,
    error_copied_feedback: Option<std::time::Instant>,

    // Launcher Log Modal
    show_launcher_log_modal: bool,
    launcher_log_content: String,

    // Textures
    bg_texture: Option<egui::TextureHandle>,
    logo_texture: Option<egui::TextureHandle>,
    icon_texture: Option<egui::TextureHandle>,
    win_texture: Option<egui::TextureHandle>,
    tux_texture: Option<egui::TextureHandle>,
    rage_empty_texture: Option<egui::TextureHandle>,
    rage_full_texture: Option<egui::TextureHandle>,

    // Shader Loading Screen
    shader_loading: Option<ShaderLoadingState>,

    // Dynamic Launcher Version
    launcher_version: String,

    // Installed Game Mods
    installed_mods: Vec<crate::mods_manager::InstalledMod>,

    // Discord Rich Presence
    pub discord_rpc: crate::discord_rpc::DiscordRpc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderLoadingPhase {
    Compiling,
    WaitingForGameWindow,
}

#[derive(Debug, Clone)]
pub struct ShaderProgressMsg {
    pub current: u32,
    pub total: u32,
    pub progress: f32,
    pub is_complete: bool,
    #[allow(dead_code)]
    pub game_window_ready: bool,
}

pub struct ShaderLoadingState {
    pub phase: ShaderLoadingPhase,
    pub progress: f32,
    pub target_progress: f32,
    pub current_shaders: u32,
    pub total_shaders: u32,
    pub is_complete: bool,
    pub completed_at: Option<std::time::Instant>,
    pub phase_changed_at: Option<std::time::Instant>,
    #[allow(dead_code)]
    pub game_ready_at: Option<std::time::Instant>,
    pub start_time: std::time::Instant,
    pub rx: Receiver<ShaderProgressMsg>,
}

#[derive(Debug, Clone)]
pub struct ErrorModalData {
    pub title: String,
    pub summary: String,
    pub details: String,
    pub log_hint: Option<String>,
}

impl LauncherApp {
    pub fn new(cc: &eframe::CreationContext<'_>, base_dir: PathBuf) -> Self {
        let mut config = LauncherConfig::load(&base_dir);
        let mut launcher_version = crate::updater::detect_launcher_version(&base_dir, &config);
        if crate::updater::compare_version_strings(&launcher_version, crate::updater::BUILTIN_VERSION) < std::cmp::Ordering::Equal {
            launcher_version = crate::updater::BUILTIN_VERSION.to_string();
        }
        if crate::updater::compare_version_strings(&config.launcher_version, &launcher_version) < std::cmp::Ordering::Equal {
            config.launcher_version = launcher_version.clone();
        }

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
        let installed_mods = crate::mods_manager::scan_installed_mods(&base_dir, &config.disabled_mods);

        let discord_rpc = crate::discord_rpc::DiscordRpc::new(config.discord_rpc_enabled);
        discord_rpc.set_in_menu(&config.language);

        let mut app = Self {
            base_dir: base_dir.clone(),
            config,
            current_lang: lang,
            is_game_valid: game_valid,
            status_message: None,
            has_gamemode: gamemode_avail,
            view_mode: ViewMode::MainMenu,
            settings_tab: SettingsTab::Graphics,
            view_transition_start: std::time::Instant::now(),
            show_release_notes_modal: false,
            listening_key_for: None,
            is_scanning: false,
            scan_rx: None,
            found_candidates_modal: None,
            show_game_not_found_modal: false,
            scan_failed_empty: false,
            is_checking_update: false,
            silent_update_check: true,
            has_new_update: false,
            update_rx: None,
            update_modal: None,
            update_dismissed_this_session: false,
            is_applying_update: false,
            update_apply_rx: None,
            cached_update_info: None,
            github_release_notes: None,
            is_fetching_release_notes: false,
            release_notes_rx: None,
            is_game_running: false,
            game_exit_rx: None,
            running_pid: Arc::new(AtomicU32::new(0)),
            has_python,
            is_downloading_python: false,
            python_dl_rx: None,
            has_engine,
            is_downloading_engine: false,
            engine_dl_rx: None,
            is_installing_shaders: false,
            shader_install_status: None,
            shader_install_rx: None,
            cached_shader_count: crate::shader_downloader::count_cached_shaders(&base_dir),
            show_first_launch_modal: is_first,
            active_crash_diagnosis: None,
            show_log_modal: false,
            log_content: String::new(),
            error_modal: None,
            error_copied_feedback: None,
            show_launcher_log_modal: false,
            launcher_log_content: String::new(),
            bg_texture: None,
            logo_texture: None,
            icon_texture: None,
            win_texture: None,
            tux_texture: None,
            rage_empty_texture: None,
            rage_full_texture: None,
            shader_loading: None,
            launcher_version,
            installed_mods,
            discord_rpc,
        };

        crate::log_info!("Launcher UI initialized successfully. Version: {}", app.launcher_version);
        crate::log_info!(
            "Hardware Specs: CPU: '{}' ({} cores, {} threads) -> Grade: {:?}, GPU: '{}' ({} MB VRAM), RAM: {} MB, Refresh Rate: {} Hz",
            app.config.cached_hardware.cpu_name,
            app.config.cached_hardware.cpu_cores,
            app.config.cached_hardware.cpu_threads,
            crate::system::evaluate_cpu(&app.config.cached_hardware.cpu_name, app.config.cached_hardware.cpu_cores, app.config.cached_hardware.cpu_threads),
            app.config.cached_hardware.gpu_name,
            app.config.cached_hardware.vram_mb,
            app.config.cached_hardware.ram_mb,
            app.config.cached_hardware.refresh_rate_hz
        );
        crate::log_info!(
            "Environment Status: Python available: {}, Game Engine available: {}, Game Directory valid: {}",
            app.has_python,
            app.has_engine,
            app.is_game_valid
        );

        app.load_textures(&cc.egui_ctx);
        app.start_silent_update_check();
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

        if self.bg_texture.is_none() {
            if let Ok(img) = image::load_from_memory(include_bytes!("../../pic1.png")) {
                let size = [img.width() as _, img.height() as _];
                let img_buffer = img.to_rgba8();
                let pixels = img_buffer.as_flat_samples();
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                self.bg_texture = Some(ctx.load_texture("gow3_full_bg", color_img, Default::default()));
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

        let icon_path = self.base_dir.join("gow3.png");
        let icon_img = if icon_path.is_file() {
            image::open(&icon_path).ok()
        } else {
            image::load_from_memory(include_bytes!("../../gow3.png")).ok()
        };
        if let Some(img) = icon_img {
            let size = [img.width() as _, img.height() as _];
            let img_buffer = img.to_rgba8();
            let pixels = img_buffer.as_flat_samples();
            let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.icon_texture = Some(ctx.load_texture("gow3_icon_omega", color_img, Default::default()));
        }

        let win_path = self.base_dir.join("win.png");
        let win_img = if win_path.is_file() {
            image::open(&win_path).ok()
        } else {
            image::load_from_memory(include_bytes!("../win.png")).ok()
        };
        if let Some(img) = win_img {
            let size = [img.width() as _, img.height() as _];
            let img_buffer = img.to_rgba8();
            let pixels = img_buffer.as_flat_samples();
            let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.win_texture = Some(ctx.load_texture("gow3_win_icon", color_img, Default::default()));
        }

        let tux_path = self.base_dir.join("tux.png");
        let tux_img = if tux_path.is_file() {
            image::open(&tux_path).ok()
        } else {
            image::load_from_memory(include_bytes!("../tux.png")).ok()
        };
        if let Some(img) = tux_img {
            let size = [img.width() as _, img.height() as _];
            let img_buffer = img.to_rgba8();
            let pixels = img_buffer.as_flat_samples();
            let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.tux_texture = Some(ctx.load_texture("gow3_tux_icon", color_img, Default::default()));
        }

        if self.rage_empty_texture.is_none() {
            if let Ok(img) = image::load_from_memory(include_bytes!("../assets/rage_empty.png")) {
                let size = [img.width() as _, img.height() as _];
                let img_buffer = img.to_rgba8();
                let pixels = img_buffer.as_flat_samples();
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                self.rage_empty_texture = Some(ctx.load_texture("rage_empty", color_img, Default::default()));
            }
        }

        if self.rage_full_texture.is_none() {
            if let Ok(img) = image::load_from_memory(include_bytes!("../assets/rage_full.png")) {
                let size = [img.width() as _, img.height() as _];
                let img_buffer = img.to_rgba8();
                let pixels = img_buffer.as_flat_samples();
                let color_img = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                self.rage_full_texture = Some(ctx.load_texture("rage_full", color_img, Default::default()));
            }
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
        self.discord_rpc.update_config(self.config.discord_rpc_enabled);
        let res = apply_settings(&self.base_dir, &self.config);

        match res {
            Ok(_) => {
                let _ = self.config.save(&self.base_dir);
                let msg = match self.current_lang {
                    Language::Turkish => "Tüm ayarlar ve yapılandırma başarıyla kaydedildi & uygulandı!",
                    Language::English => "All settings and configuration saved & applied successfully!",
                    Language::German => "Alle Einstellungen und Konfigurationen erfolgreich gespeichert & angewendet!",
                    Language::Spanish => "¡Todos los ajustes y configuración guardados y aplicados con éxito!",
                };
                self.status_message = Some((msg.to_string(), false));
            }
            Err(e) => {
                self.status_message = Some((format!("Error: {}", e), true));
            }
        }
    }

    pub fn refresh_installed_mods(&mut self) {
        self.installed_mods = crate::mods_manager::scan_installed_mods(&self.base_dir, &self.config.disabled_mods);
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

    fn start_silent_update_check(&mut self) {
        if self.is_checking_update {
            return;
        }
        self.is_checking_update = true;
        self.silent_update_check = true;
        let (tx, rx) = channel();
        self.update_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        let current_commit = self.config.last_git_commit.clone();
        let current_ver = self.launcher_version.clone();
        thread::spawn(move || {
            let res = check_github_update(&base_dir_clone, &current_commit, &current_ver);
            let _ = tx.send(res);
        });
    }

    fn start_update_check(&mut self) {
        if self.is_checking_update {
            return;
        }
        self.is_checking_update = true;
        self.silent_update_check = false;
        let (tx, rx) = channel();
        self.update_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        let current_commit = self.config.last_git_commit.clone();
        let current_ver = self.launcher_version.clone();
        thread::spawn(move || {
            let res = check_github_update(&base_dir_clone, &current_commit, &current_ver);
            let _ = tx.send(res);
        });
    }

    pub fn start_fetch_release_notes(&mut self) {
        if self.is_fetching_release_notes || self.github_release_notes.is_some() {
            return;
        }
        self.is_fetching_release_notes = true;
        let (tx, rx) = channel();
        self.release_notes_rx = Some(rx);

        thread::spawn(move || {
            let res = crate::updater::fetch_github_release_notes();
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

    fn start_engine_download(&mut self, is_alternative: bool) {
        if self.is_downloading_engine {
            return;
        }
        self.is_downloading_engine = true;
        let (tx, rx) = channel();
        self.engine_dl_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        thread::spawn(move || {
            let res = crate::runner::download_engine(&base_dir_clone, is_alternative);
            let _ = tx.send(res);
        });
    }

    fn start_shader_install(&mut self) {
        if self.is_installing_shaders {
            return;
        }
        self.is_installing_shaders = true;
        self.shader_install_status = Some(self.t("shader_downloading").to_string());
        let (tx, rx) = channel();
        self.shader_install_rx = Some(rx);

        let base_dir_clone = self.base_dir.clone();
        thread::spawn(move || {
            let res = crate::shader_downloader::download_and_install_shaders(&base_dir_clone);
            let _ = tx.send(res);
        });
    }

    fn force_quit_game(&mut self, ctx: Option<&egui::Context>) {
        if let Some(c) = ctx {
            c.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
            c.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        let pid = self.running_pid.load(Ordering::SeqCst);
        crate::runner::kill_game_processes(pid);
        self.running_pid.store(0, Ordering::SeqCst);
        self.is_game_running = false;
        self.game_exit_rx = None;
        self.shader_loading = None;
        self.status_message = Some((self.t("force_quit_status").to_string(), false));
    }

    fn kill_leftover_processes(&mut self, ctx: Option<&egui::Context>) {
        if let Some(c) = ctx {
            c.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
            c.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
        crate::runner::kill_game_processes(0);
        self.shader_loading = None;
        self.status_message = Some((self.t("force_quit_status").to_string(), false));
    }

    fn launch_game(&mut self, ctx: &egui::Context) {
        self.apply_all_optimizations();

        self.is_game_running = true;
        let (tx, rx) = channel();
        self.game_exit_rx = Some(rx);

        if self.config.launch_while_compiling_shaders {
            // "Shader'lar derlenirken oyun açılsın": Fast launch immediately while compiling in the background
            self.shader_loading = None;
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
            let is_modded = self.installed_mods.iter().any(|m| m.is_enabled);
            self.discord_rpc.set_in_game(self.base_dir.clone(), now, self.config.target_fps, is_modded, &self.config.language);
        } else {
            self.discord_rpc.set_compiling_shaders(Some(0.0), &self.config.language);
            // Pre-warm all shaders to 100% with the full Spartan Rage bar before game window is displayed
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            #[cfg(target_os = "windows")]
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));

            let initial_total = if self.cached_shader_count > 0 && self.cached_shader_count < 5000 {
                self.cached_shader_count as u32
            } else {
                2302u32
            };

            // Truncate previous log so monitor reads this session fresh
            let log_path = self.base_dir.join("gow3_log.txt");
            let _ = std::fs::remove_file(&log_path);

            // Initialize God of War III Shader Loading Screen
            let (progress_tx, progress_rx) = channel();
            self.shader_loading = Some(ShaderLoadingState {
                phase: ShaderLoadingPhase::Compiling,
                progress: 0.0,
                target_progress: 0.06,
                current_shaders: 0,
                total_shaders: initial_total,
                is_complete: false,
                completed_at: None,
                phase_changed_at: None,
                game_ready_at: None,
                start_time: std::time::Instant::now(),
                rx: progress_rx,
            });

            // Spawn background log monitor to track shader warm-up progress and window presentation in real time
            let base_dir_monitor = self.base_dir.clone();
            let running_pid_monitor = self.running_pid.clone();
            thread::spawn(move || {
                let log_path = base_dir_monitor.join("gow3_log.txt");
                let user_log_path = base_dir_monitor.join("user/log/shad_log.txt");
                let mut last_pos: u64 = 0;
                let mut user_last_pos: u64 = 0;
                let start = std::time::Instant::now();
                let detected_total = initial_total;
                let mut pid_seen = false;
                let mut warmup_done = false;

                while start.elapsed().as_secs() < 120 {
                    let pid = running_pid_monitor.load(Ordering::SeqCst);
                    if pid > 0 {
                        pid_seen = true;
                    } else if pid_seen && start.elapsed().as_millis() > 3000 {
                        // Process had started and then died
                        break;
                    }

                    if let Ok(mut f) = std::fs::File::open(&log_path) {
                        use std::io::{BufRead, BufReader, Seek, SeekFrom};
                        if let Ok(meta) = f.metadata() {
                            if meta.len() < last_pos {
                                last_pos = 0;
                            }
                        }
                        if f.seek(SeekFrom::Start(last_pos)).is_ok() {
                            let reader = BufReader::new(f);
                            for line in reader.lines().flatten() {
                                // 1. Match "Pipeline warm-up: 2301 of 2302 pipelines in 1.60 s"
                                if !warmup_done && line.contains("Pipeline warm-up:") {
                                    warmup_done = true;
                                    let mut cur = detected_total;
                                    let mut tot = detected_total;
                                    if let Some(pos) = line.find("Pipeline warm-up:") {
                                        let sub = line[pos + 17..].trim();
                                        let parts: Vec<&str> = sub.split_whitespace().collect();
                                        if parts.len() >= 3 && parts[1] == "of" {
                                            if let (Ok(c), Ok(t)) = (parts[0].parse::<u32>(), parts[2].parse::<u32>()) {
                                                if t > 0 {
                                                    cur = c;
                                                    tot = t;
                                                }
                                            }
                                        }
                                    }
                                    let _ = progress_tx.send(ShaderProgressMsg {
                                        current: cur,
                                        total: tot,
                                        progress: 1.0,
                                        is_complete: true,
                                        game_window_ready: false,
                                    });
                                } else if !warmup_done
                                    && (line.contains("pipelines in")
                                        || line.contains("Preloaded")
                                        || line.contains("Pipeline binary cache:")
                                        || line.contains("Swapchain: VSync")
                                        || line.contains("Vulkan presenter ready")
                                        || line.contains("vblank")
                                        || line.contains("Overlay: menu ready"))
                                {
                                    warmup_done = true;
                                    let _ = progress_tx.send(ShaderProgressMsg {
                                        current: detected_total,
                                        total: detected_total,
                                        progress: 1.0,
                                        is_complete: true,
                                        game_window_ready: false,
                                    });
                                }

                                // 2. Game screen is actually visible on display: Sony opening credits, video playback or graphics stream
                                if warmup_done
                                    && (line.contains("OPENINGCREDITS")
                                        || line.contains("Starting video stream")
                                        || line.contains("allocate a frame buffer")
                                        || line.contains("my_decode_thread")
                                        || line.contains("subtitle =")
                                        || line.contains("WAD Shell")
                                        || line.contains("WAD R_UI")
                                        || line.contains("IT_SET_PREDICATION")
                                        || line.contains("ProcessGraphics"))
                                {
                                    let _ = progress_tx.send(ShaderProgressMsg {
                                        current: detected_total,
                                        total: detected_total,
                                        progress: 1.0,
                                        is_complete: true,
                                        game_window_ready: true,
                                    });
                                    return;
                                }
                            }
                            if let Ok(new_meta) = std::fs::metadata(&log_path) {
                                last_pos = new_meta.len();
                            }
                        }
                    }

                    // Also check user/log/shad_log.txt for opening credits / video decoder in case
                    if warmup_done {
                        if let Ok(mut uf) = std::fs::File::open(&user_log_path) {
                            use std::io::{BufRead, BufReader, Seek, SeekFrom};
                            if let Ok(umeta) = uf.metadata() {
                                if umeta.len() < user_last_pos {
                                    user_last_pos = 0;
                                }
                            }
                            if uf.seek(SeekFrom::Start(user_last_pos)).is_ok() {
                                let ureader = BufReader::new(uf);
                                for uline in ureader.lines().flatten() {
                                    if uline.contains("OPENINGCREDITS")
                                        || uline.contains("Starting video stream")
                                        || uline.contains("allocate a frame buffer")
                                        || uline.contains("my_decode_thread")
                                        || uline.contains("sceVideodecCreateDecoder")
                                    {
                                        let _ = progress_tx.send(ShaderProgressMsg {
                                            current: detected_total,
                                            total: detected_total,
                                            progress: 1.0,
                                            is_complete: true,
                                            game_window_ready: true,
                                        });
                                        return;
                                    }
                                }
                                if let Ok(new_umeta) = std::fs::metadata(&user_log_path) {
                                    user_last_pos = new_umeta.len();
                                }
                            }
                        }
                    }

                    thread::sleep(std::time::Duration::from_millis(30));
                }
            });
        }

        let base_dir_clone = self.base_dir.clone();
        let config_clone = self.config.clone();
        let use_gamemode = self.has_gamemode && self.config.use_gamemode;
        let pid_holder = self.running_pid.clone();

        thread::spawn(move || {
            match crate::runner::launch_gow3(&base_dir_clone, &config_clone, use_gamemode, Some(pid_holder)) {
                Ok(code) => {
                    let _ = tx.send(Ok(code));
                }
                Err(err) => {
                    crate::log_error!("Launch error: {}", err);
                    let log_path = base_dir_clone.join("gow3_log.txt");
                    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
                        use std::io::Write;
                        let _ = writeln!(f, "\n[LAUNCH ERROR] {}", err);
                    }
                    let _ = tx.send(Err(err));
                }
            }
        });
    }

    fn render_shader_loading_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (progress, current_shaders, total_shaders, phase) = if let Some(ref sl) = self.shader_loading {
            (sl.progress.clamp(0.0, 1.0), sl.current_shaders, sl.total_shaders, sl.phase)
        } else {
            return;
        };

        let screen_rect = ui.max_rect();
        let painter = ui.painter();
        let time = ctx.input(|i| i.time);

        // --- MODE B: PS4 SPLASH MODE (pic1.png animation until game window opens) ---
        if !self.config.show_shader_compile_rage_meter {
            painter.rect_filled(screen_rect, 0.0, egui::Color32::from_rgb(5, 5, 8));

            if let Some(ref bg_tex) = self.bg_texture {
                let elapsed = if let Some(ref sl) = self.shader_loading {
                    sl.start_time.elapsed().as_secs_f32()
                } else {
                    1.0
                };
                let alpha = (elapsed * 2.2).min(1.0);
                // Continuous, buttery-smooth cinematic push-in zoom towards camera
                let zoom_progress = (elapsed / 20.0).clamp(0.0, 1.0);
                let zoom_ease = zoom_progress * (2.0 - zoom_progress); // smooth quad ease-out
                let scale_factor = 1.0 + zoom_ease * 0.095;
                let center = screen_rect.center();
                let zoomed_size = screen_rect.size() * scale_factor;
                let draw_rect = egui::Rect::from_center_size(center, zoomed_size);

                painter.image(
                    bg_tex.id(),
                    draw_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::from_white_alpha((alpha * 255.0) as u8),
                );
            }

            // Subtle dark vignette at edges
            painter.rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 30),
            );

            // Force quit button at bottom in PS4 mode as well (unobtrusive)
            let scale = if screen_rect.height() > 800.0 {
                (screen_rect.height() / 720.0).clamp(1.0, 1.8)
            } else {
                1.0
            };
            let center = screen_rect.center();
            let quit_rect = egui::Rect::from_center_size(
                egui::pos2(center.x, screen_rect.max.y - 45.0 * scale.min(1.2)),
                egui::vec2(160.0 * scale.min(1.2), 36.0 * scale.min(1.2)),
            );
            let quit_response = ui.put(
                quit_rect,
                egui::Button::new(
                    egui::RichText::new(self.t("force_quit_btn"))
                        .size(13.0 * scale.min(1.1))
                        .color(egui::Color32::from_rgb(220, 195, 195)),
                )
                .fill(egui::Color32::from_rgba_unmultiplied(30, 16, 18, 200))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(160, 45, 45, 140)))
                .corner_radius(egui::CornerRadius::same(6)),
            );
            if quit_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if quit_response.clicked() {
                self.force_quit_game(Some(ctx));
            }
            return;
        }

        // --- MODE A: SPARTAN RAGE COMPILATION BAR ---

        // If compilation is complete and we are waiting for the game window:
        // Hold pure black screen (as requested: "derleme bittikten sonra siyah ekranda beklesin")!
        if phase == ShaderLoadingPhase::WaitingForGameWindow {
            painter.rect_filled(screen_rect, 0.0, egui::Color32::BLACK);
            return;
        }

        // 1. Cinematic dark background with subtle vignette
        painter.rect_filled(screen_rect, 0.0, egui::Color32::from_rgb(12, 10, 15));

        if let Some(ref bg_tex) = self.bg_texture {
            painter.image(
                bg_tex.id(),
                screen_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::from_black_alpha(215),
            );
        }

        // Responsive scaling factor for fullscreen / high-resolution displays
        let scale = if screen_rect.height() > 800.0 {
            (screen_rect.height() / 720.0).clamp(1.0, 1.8)
        } else {
            1.0
        };

        // Center position for rage bar
        let center = screen_rect.center();
        let glow_pulse = ((time * 2.8).sin() * 0.5 + 0.5) as f32; // 0.0 to 1.0

        // Ambient radial ember glow behind rage crest
        let glow_alpha = ((25.0 + 35.0 * progress + 20.0 * glow_pulse) as u8).min(110);
        painter.circle_filled(
            egui::pos2(center.x, center.y - 45.0 * scale),
            (160.0 + progress * 50.0) * scale,
            egui::Color32::from_rgba_unmultiplied(220, 60, 20, glow_alpha / 3),
        );
        painter.circle_filled(
            egui::pos2(center.x, center.y - 45.0 * scale),
            (90.0 + progress * 35.0) * scale,
            egui::Color32::from_rgba_unmultiplied(255, 170, 30, glow_alpha / 2),
        );

        // 2. God of War Spartan Rage Bar (Height: ~192, Width: ~135; aspect ratio 0.70)
        let bar_w = 135.0 * scale;
        let bar_h = 192.0 * scale;
        let bar_rect = egui::Rect::from_center_size(
            egui::pos2(center.x, center.y - 50.0 * scale),
            egui::vec2(bar_w, bar_h),
        );

        if let (Some(tex_empty), Some(tex_full)) = (&self.rage_empty_texture, &self.rage_full_texture) {
            // Background empty meter
            painter.image(
                tex_empty.id(),
                bar_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );

            // Fiery golden rage filling from bottom to top!
            if progress > 0.001 {
                let fill_height = bar_rect.height() * progress;
                let top_y = bar_rect.max.y - fill_height;
                let filled_rect = egui::Rect::from_min_max(
                    egui::pos2(bar_rect.min.x, top_y),
                    bar_rect.max,
                );
                let uv_rect = egui::Rect::from_min_max(
                    egui::pos2(0.0, 1.0 - progress),
                    egui::pos2(1.0, 1.0),
                );
                painter.image(tex_full.id(), filled_rect, uv_rect, egui::Color32::WHITE);
            }
        }

        // 3. Percentage in authentic God of War Font
        let pct_val = (progress * 100.0).round() as u32;
        let pct_str = format!("{}%", pct_val.min(100));

        let gow_font_pct = egui::FontId::new(44.0 * scale, egui::FontFamily::Name("GodOfWar".into()));
        let pct_pos = egui::pos2(center.x, center.y + 75.0 * scale);

        // Shadow / Glow behind percentage
        painter.text(
            egui::pos2(pct_pos.x + 2.0 * scale, pct_pos.y + 2.0 * scale),
            egui::Align2::CENTER_CENTER,
            &pct_str,
            gow_font_pct.clone(),
            egui::Color32::from_black_alpha(230),
        );
        painter.text(
            pct_pos,
            egui::Align2::CENTER_CENTER,
            &pct_str,
            gow_font_pct,
            egui::Color32::from_rgb(255, 218, 85),
        );

        // 4. "LOADING SHADERS..." in God of War Font
        let label_str = self.t("loading_shaders");
        let gow_font_label = egui::FontId::new(22.0 * scale, egui::FontFamily::Name("GodOfWar".into()));
        let label_pos = egui::pos2(center.x, center.y + 120.0 * scale);

        painter.text(
            egui::pos2(label_pos.x + 1.5 * scale, label_pos.y + 1.5 * scale),
            egui::Align2::CENTER_CENTER,
            label_str,
            gow_font_label.clone(),
            egui::Color32::from_black_alpha(220),
        );
        painter.text(
            label_pos,
            egui::Align2::CENTER_CENTER,
            label_str,
            gow_font_label,
            egui::Color32::from_rgb(235, 222, 190),
        );

        // 5. Subtle shader pipeline counter
        if total_shaders > 0 {
            let detail_str = format!("{} / {} Boru Hattı", current_shaders, total_shaders);
            painter.text(
                egui::pos2(center.x, center.y + 155.0 * scale),
                egui::Align2::CENTER_CENTER,
                detail_str,
                egui::FontId::proportional(13.0 * scale),
                egui::Color32::from_rgb(180, 170, 160),
            );
        } else {
            painter.text(
                egui::pos2(center.x, center.y + 155.0 * scale),
                egui::Align2::CENTER_CENTER,
                self.t("loading_shaders_sub"),
                egui::FontId::proportional(13.0 * scale),
                egui::Color32::from_rgb(160, 150, 140),
            );
        }

        // 6. Force Quit / Cancel button at bottom
        let quit_rect = egui::Rect::from_center_size(
            egui::pos2(center.x, screen_rect.max.y - 45.0 * scale.min(1.2)),
            egui::vec2(160.0 * scale.min(1.2), 36.0 * scale.min(1.2)),
        );
        let quit_response = ui.put(
            quit_rect,
            egui::Button::new(
                egui::RichText::new(self.t("force_quit_btn"))
                    .size(14.0 * scale.min(1.1))
                    .color(egui::Color32::from_rgb(240, 185, 185)),
            )
            .fill(egui::Color32::from_rgb(45, 18, 20))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(150, 45, 45)))
            .corner_radius(egui::CornerRadius::same(6)),
        );
        if quit_response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if quit_response.clicked() {
            self.force_quit_game(Some(ctx));
        }
    }

    pub fn render_console_main_menu(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let screen_rect = ui.max_rect();

        // 1. Fullscreen Wallpaper (pic1.png) - Dimmed slightly for comfort / less eye strain
        if let Some(ref bg) = self.bg_texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(bg.id(), screen_rect, uv, egui::Color32::from_rgb(180, 180, 192));
            ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_rgba_unmultiplied(6, 6, 10, 60));
        } else {
            ui.painter().rect_filled(screen_rect, 0.0, egui::Color32::from_rgb(15, 12, 18));
        }

        // 2. Cinematic Vignette (Top shadow + Bottom dark shadow for console menu readability)
        let top_vignette = egui::Rect::from_min_max(
            screen_rect.min,
            egui::pos2(screen_rect.max.x, screen_rect.min.y + 70.0),
        );
        ui.painter().rect_filled(top_vignette, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 130));

        let bottom_vignette_height = 145.0;
        let bottom_vignette = egui::Rect::from_min_max(
            egui::pos2(screen_rect.min.x, screen_rect.max.y - bottom_vignette_height),
            screen_rect.max,
        );
        ui.painter().rect_filled(bottom_vignette, 0.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 160));

        // 3. Top-Left: Dil Seçenekleri (Language switcher)
        let lang_top_left = screen_rect.min + egui::vec2(24.0, 18.0);
        let lang_rect = egui::Rect::from_min_size(lang_top_left, egui::vec2(320.0, 36.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(lang_rect), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                let langs = [
                    (Language::Turkish, "TR"),
                    (Language::English, "EN"),
                    (Language::German, "DE"),
                    (Language::Spanish, "ES"),
                ];
                for (lang, code_str) in &langs {
                    let is_selected = self.current_lang == *lang;
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(60.0, 28.0), egui::Sense::click());
                    let hov = resp.hovered();
                    let hov_anim = ctx.animate_bool(resp.id.with("hov"), hov);
                    let sel_anim = ctx.animate_bool(resp.id.with("sel"), is_selected);
                    if hov {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    let p = ui.painter();
                    let bg = if sel_anim > 0.01 {
                        egui::Color32::from_rgba_unmultiplied(
                            egui::lerp(24.0..=160.0, sel_anim) as u8,
                            egui::lerp(26.0..=35.0, sel_anim) as u8,
                            egui::lerp(35.0..=35.0, sel_anim) as u8,
                            egui::lerp(100.0..=220.0, sel_anim) as u8,
                        )
                    } else if hov_anim > 0.01 {
                        egui::Color32::from_rgba_unmultiplied(40, 42, 58, (180.0 * hov_anim) as u8)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(15, 17, 24, 150)
                    };

                    let stroke = if sel_anim > 0.01 {
                        egui::Stroke::new(1.3, egui::Color32::from_rgb(255, 215, 100))
                    } else if hov_anim > 0.01 {
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(239, 68, 68))
                    } else {
                        egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 35))
                    };

                    p.rect_filled(rect, 6.0, bg);
                    p.rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);

                    let text_color = if is_selected {
                        egui::Color32::WHITE
                    } else if hov {
                        egui::Color32::from_rgb(255, 225, 120)
                    } else {
                        egui::Color32::from_rgb(190, 195, 210)
                    };

                    let flag_size = egui::vec2(16.0, 11.0);
                    let flag_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.left() + 9.0, rect.center().y - flag_size.y / 2.0),
                        flag_size,
                    );
                    match lang {
                        Language::Turkish => paint_turkish_flag(&p, flag_rect),
                        Language::English => paint_uk_flag(&p, flag_rect),
                        Language::German => paint_german_flag(&p, flag_rect),
                        Language::Spanish => paint_spanish_flag(&p, flag_rect),
                    }
                    p.text(
                        egui::pos2(rect.left() + 32.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        *code_str,
                        egui::FontId::proportional(12.0),
                        text_color,
                    );

                    if resp.clicked() {
                        self.current_lang = *lang;
                        self.config.language = lang.code().to_string();
                        let _ = self.config.save(&self.base_dir);
                        self.discord_rpc.update_language(lang.code());
                        self.discord_rpc.set_in_menu(lang.code());
                    }
                }
            });
        });

        // 4. Top-Right: Launcher Sürümü & Güncelleme Butonu
        let top_right_rect = egui::Rect::from_min_max(
            egui::pos2(screen_rect.max.x - 380.0, screen_rect.min.y + 16.0),
            egui::pos2(screen_rect.max.x - 24.0, screen_rect.min.y + 48.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(top_right_rect), |ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Version display
                let ver_text = format!("v{}", self.launcher_version.trim_start_matches('v'));
                let ver_font = egui::FontId::proportional(15.0);
                ui.label(
                    egui::RichText::new(ver_text)
                        .font(ver_font)
                        .color(egui::Color32::from_rgb(180, 185, 200))
                        .strong(),
                );

                ui.add_space(10.0);

                // Update Button
                if self.is_checking_update {
                    let time = ctx.input(|i| i.time);
                    let pulse = ((time * 4.0).sin() * 0.5 + 0.5) as f32;
                    ctx.request_repaint();

                    let btn = egui::Button::new(
                        egui::RichText::new(format!("🔄 {}", self.t("checking_updates")))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(200, 210, 230)),
                    )
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 35, 50, (150.0 + 40.0 * pulse) as u8))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 150, 255, 120)))
                    .corner_radius(egui::CornerRadius::same(6));
                    ui.add_enabled(false, btn);
                } else if self.has_new_update {
                    let time = ctx.input(|i| i.time);
                    let pulse = ((time * 3.5).sin() * 0.5 + 0.5) as f32;
                    ctx.request_repaint();

                    let btn = egui::Button::new(
                        egui::RichText::new(self.t("update_available_badge"))
                            .size(12.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgba_unmultiplied(
                        (180.0 + 40.0 * pulse) as u8,
                        (40.0 + 20.0 * pulse) as u8,
                        25,
                        230,
                    ))
                    .stroke(egui::Stroke::new(
                        1.5 + pulse * 0.5,
                        egui::Color32::from_rgb(255, (200.0 + 55.0 * pulse) as u8, 80),
                    ))
                    .corner_radius(egui::CornerRadius::same(6));

                    let resp = ui.add(btn);
                    if resp.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        if let Some(ref info) = self.cached_update_info {
                            self.update_modal = Some(info.clone());
                        } else {
                            self.start_update_check();
                        }
                    }
                } else {
                    let btn = egui::Button::new(
                        egui::RichText::new(format!("🔄 {}", self.t("btn_update")))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(220, 225, 235)),
                    )
                    .fill(egui::Color32::from_rgba_unmultiplied(22, 24, 34, 180))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 45)))
                    .corner_radius(egui::CornerRadius::same(6));

                    let resp = ui.add(btn);
                    if resp.hovered() {
                        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        self.start_update_check();
                    }
                }
            });
        });

        // 5. Bottom-Right: God of War III Remastered Logo (gow3-remastered.png)
        if let Some(ref logo) = self.logo_texture {
            let logo_w = (screen_rect.width() * 0.28).clamp(240.0, 320.0);
            let logo_h = logo_w / 2.79;
            let logo_rect = egui::Rect::from_min_size(
                egui::pos2(screen_rect.max.x - logo_w - 24.0, screen_rect.max.y - logo_h - 20.0),
                egui::vec2(logo_w, logo_h),
            );
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(logo.id(), logo_rect, uv, egui::Color32::WHITE);
        }

        // 6. Bottom-Center: Play Button (▶ OYNA)
        let play_y = screen_rect.max.y - 72.0;
        let play_center_x = screen_rect.center().x;

        if self.is_game_running {
            let time = ctx.input(|i| i.time);
            let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
            ctx.request_repaint();

            let row_w = 400.0;
            let row_rect = egui::Rect::from_min_size(
                egui::pos2(play_center_x - row_w / 2.0, play_y),
                egui::vec2(row_w, 48.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(row_rect), |ui| {
                ui.horizontal(|ui| {
                    let emerald_r = (25.0 + 15.0 * pulse) as u8;
                    let emerald_g = (65.0 + 30.0 * pulse) as u8;
                    let emerald_b = (40.0 + 15.0 * pulse) as u8;

                    let running_btn = egui::Button::new(
                        egui::RichText::new(self.t("play_running"))
                            .size(15.5)
                            .color(egui::Color32::from_rgb(140, 255, 155))
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(emerald_r, emerald_g, emerald_b))
                    .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(100, 255, 120, (120.0 + 100.0 * pulse) as u8)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(200.0, 48.0));
                    ui.add_enabled(false, running_btn);

                    let force_quit_btn = egui::Button::new(
                        egui::RichText::new(self.t("force_quit_btn"))
                            .size(15.5)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(220, 42, 42))
                    .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 120, 120, 180)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(170.0, 48.0));

                    let res = ui.add(force_quit_btn).on_hover_text(self.t("force_quit_tooltip"));
                    if res.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if res.clicked() {
                        self.force_quit_game(Some(ui.ctx()));
                    }
                });
            });
        } else {
            let time = ctx.input(|i| i.time);
            let pulse = ((time * 2.5).sin() * 0.5 + 0.5) as f32;
            ctx.request_repaint();

            let play_label = format!("▶  {}", self.t("menu_play"));
            let play_font = egui::FontId::proportional(22.0);
            let play_galley = ui.painter().layout_no_wrap(play_label.clone(), play_font.clone(), egui::Color32::WHITE);
            let btn_w = (play_galley.rect.width() + 48.0).max(210.0);
            let btn_h = 50.0;
            let play_rect = egui::Rect::from_center_size(
                egui::pos2(play_center_x, play_y + btn_h / 2.0),
                egui::vec2(btn_w, btn_h),
            );

            let resp = ui.allocate_rect(play_rect, egui::Sense::click());
            let is_hov = resp.hovered();
            let hov_anim = ctx.animate_bool(resp.id.with("play_hov"), is_hov);
            if is_hov {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            let is_pressed = resp.is_pointer_button_down_on();
            let scale = if is_pressed { -1.5 } else { hov_anim * 2.0 };
            let draw_rect = play_rect.expand(scale);

            let bg_r = (215.0 + 35.0 * pulse) as u8;
            let bg_g = (35.0 + 25.0 * pulse) as u8;
            let bg_b = 35u8;
            let btn_color = if hov_anim > 0.01 {
                egui::Color32::from_rgb(
                    egui::lerp(bg_r as f32..=245.0, hov_anim) as u8,
                    egui::lerp(bg_g as f32..=50.0, hov_anim) as u8,
                    egui::lerp(bg_b as f32..=40.0, hov_anim) as u8,
                )
            } else {
                egui::Color32::from_rgb(bg_r, bg_g, bg_b)
            };

            let border_color = if hov_anim > 0.01 {
                egui::Color32::from_rgb(255, 225, 120)
            } else {
                egui::Color32::from_rgba_unmultiplied(
                    255,
                    (110.0 + 90.0 * pulse) as u8,
                    (70.0 + 50.0 * pulse) as u8,
                    (160.0 + 95.0 * pulse) as u8,
                )
            };

            ui.painter().rect_filled(draw_rect, 10.0, btn_color);
            ui.painter().rect_stroke(draw_rect, 10.0, egui::Stroke::new(1.8 + pulse * 1.0 + hov_anim * 0.8, border_color), egui::StrokeKind::Inside);

            let text_color = if is_hov {
                egui::Color32::WHITE
            } else {
                egui::Color32::from_rgb(255, 245, 230)
            };
            ui.painter().text(
                draw_rect.center(),
                egui::Align2::CENTER_CENTER,
                play_label,
                play_font,
                text_color,
            );

            if resp.clicked() {
                self.launch_game(ctx);
            }
        }

        // 7. Bottom-Left: Console Action Menu Items (Row 1: Latest News, Row 2: Mods, Row 3: Options)
        let bl_x = screen_rect.min.x + 28.0;
        let bl_y = screen_rect.max.y - 124.0;
        let bl_rect = egui::Rect::from_min_size(
            egui::pos2(bl_x, bl_y),
            egui::vec2(280.0, 114.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(bl_rect), |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 6.0);

                // Row 1: 📜 GÜNCELLEME NOTLARI / LATEST NEWS
                if render_console_action_button(
                    ui,
                    "btn_latest_news",
                    "📜",
                    self.t("menu_latest_news"),
                    14.5,
                    true,
                ) {
                    self.start_fetch_release_notes();
                    self.show_release_notes_modal = true;
                }

                // Row 2: 📦 MODLAR / MODS
                if render_console_action_button(
                    ui,
                    "btn_mods",
                    "📦",
                    self.t("nav_mods"),
                    14.5,
                    false,
                ) {
                    self.settings_tab = SettingsTab::Mods;
                    self.view_mode = ViewMode::Settings;
                    self.view_transition_start = std::time::Instant::now();
                }

                // Row 3: ⚙️ AYARLAR / OPTIONS
                if render_console_action_button(
                    ui,
                    "btn_options",
                    "⚙️",
                    self.t("menu_options"),
                    14.5,
                    false,
                ) {
                    self.settings_tab = SettingsTab::Graphics;
                    self.view_mode = ViewMode::Settings;
                    self.view_transition_start = std::time::Instant::now();
                }
            });
        });

        // Status banner overlay if status message is active
        if let Some((ref msg, is_err)) = self.status_message {
            let (bg, border, text_col) = if is_err {
                (
                    egui::Color32::from_rgba_unmultiplied(65, 20, 20, 220),
                    egui::Color32::from_rgb(239, 68, 68),
                    egui::Color32::from_rgb(255, 140, 140),
                )
            } else {
                (
                    egui::Color32::from_rgba_unmultiplied(20, 50, 28, 220),
                    egui::Color32::from_rgb(34, 197, 94),
                    egui::Color32::from_rgb(134, 239, 172),
                )
            };

            let banner_rect = egui::Rect::from_center_size(
                egui::pos2(screen_rect.center().x, screen_rect.min.y + 60.0),
                egui::vec2(480.0, 32.0),
            );
            ui.painter().rect_filled(banner_rect, 6.0, bg);
            ui.painter().rect_stroke(banner_rect, 6.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);
            let icon = if is_err { "⚠️" } else { "✅" };
            ui.painter().text(
                banner_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} {}", icon, msg),
                egui::FontId::proportional(12.5),
                text_col,
            );
        }
    }

    pub fn render_settings_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let screen_rect = ui.max_rect();

        // 1. Fullscreen Wallpaper (pic1.png)
        if let Some(ref bg) = self.bg_texture {
            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            ui.painter().image(bg.id(), screen_rect, uv, egui::Color32::from_white_alpha(90));
        }

        // 2. Dark Frosted Backdrop
        ui.painter().rect_filled(
            screen_rect,
            0.0,
            egui::Color32::from_rgba_unmultiplied(10, 11, 16, 238),
        );

        let total_avail = ui.available_size();
        let header_h = 52.0;
        let footer_h = 48.0;
        let content_h = (total_avail.y - header_h - footer_h - 16.0).max(200.0);

        ui.vertical(|ui| {
            // ==========================================
            // HEADER BAR: TITLE + 4 TABS
            // ==========================================
            ui.allocate_ui_with_layout(
                egui::vec2(total_avail.x, header_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(format!("⚙️ {}", self.t("menu_options")))
                            .size(19.0)
                            .color(egui::Color32::from_rgb(255, 215, 100))
                            .strong(),
                    );
                    ui.add_space(24.0);

                    // 1. 🎨 GRAFİKLER
                    if render_settings_tab_btn(
                        ui,
                        "tab_gfx",
                        "🎨",
                        self.t("nav_graphics"),
                        self.settings_tab == SettingsTab::Graphics,
                    ) {
                        self.settings_tab = SettingsTab::Graphics;
                        self.view_transition_start = std::time::Instant::now();
                    }
                    ui.add_space(8.0);

                    // 2. 📦 MODLAR
                    if render_settings_tab_btn(
                        ui,
                        "tab_mods",
                        "📦",
                        self.t("nav_mods"),
                        self.settings_tab == SettingsTab::Mods,
                    ) {
                        self.settings_tab = SettingsTab::Mods;
                        self.view_transition_start = std::time::Instant::now();
                    }
                    ui.add_space(8.0);

                    // 3. ⚡ SİSTEM ÖZELLİKLERİ
                    if render_settings_tab_btn(
                        ui,
                        "tab_sys",
                        "⚡",
                        self.t("system_specs_title"),
                        self.settings_tab == SettingsTab::SystemSpecs,
                    ) {
                        self.settings_tab = SettingsTab::SystemSpecs;
                        self.view_transition_start = std::time::Instant::now();
                    }
                    ui.add_space(8.0);

                    // 4. 📁 OYUN KONUMU
                    if render_settings_tab_btn(
                        ui,
                        "tab_loc",
                        "📁",
                        self.t("nav_game_location"),
                        self.settings_tab == SettingsTab::GameLocation,
                    ) {
                        self.settings_tab = SettingsTab::GameLocation;
                        self.view_transition_start = std::time::Instant::now();
                    }
                    ui.add_space(8.0);

                    // 5. 🎮 KONTROLLER
                    if render_settings_tab_btn(
                        ui,
                        "tab_ctrl",
                        "🎮",
                        self.t("nav_controls"),
                        self.settings_tab == SettingsTab::Controls,
                    ) {
                        self.settings_tab = SettingsTab::Controls;
                        self.view_transition_start = std::time::Instant::now();
                    }
                    ui.add_space(8.0);

                    // 6. ⚡ SHADER AYARI
                    if render_settings_tab_btn(
                        ui,
                        "tab_shd",
                        "⚡",
                        self.t("nav_shaders"),
                        self.settings_tab == SettingsTab::Shaders,
                    ) {
                        self.settings_tab = SettingsTab::Shaders;
                        self.view_transition_start = std::time::Instant::now();
                    }
                },
            );

            ui.separator();

            // ==========================================
            // MAIN CONTENT SCROLL AREA
            // ==========================================
            ui.allocate_ui_with_layout(
                egui::vec2(total_avail.x, content_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            // Ayarlar ekranında göz yormayan, okunaklı net metin boyutları
                            ui.style_mut().text_styles.insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
                            ui.style_mut().text_styles.insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));

                            // Fade / smooth slide transition
                            let elapsed = self.view_transition_start.elapsed().as_secs_f32();
                            let t = (elapsed / 0.18).min(1.0);
                            if t < 1.0 {
                                ui.ctx().request_repaint();
                            }
                            let slide = (1.0 - t * (2.0 - t)) * 6.0;
                            ui.add_space(slide);

                            match self.settings_tab {
                                SettingsTab::Graphics => {
                                    self.render_graphics_screen(ui, ctx);
                                    ui.add_space(14.0);
                                    self.render_mods_screen(ui, ctx);
                                }
                                SettingsTab::Mods => {
                                    self.render_custom_mods_screen(ui, ctx);
                                }
                                SettingsTab::SystemSpecs => {
                                    self.render_system_specs_screen(ui, ctx);
                                }
                                SettingsTab::GameLocation => {
                                    self.render_game_location_screen(ui, ctx);
                                }
                                SettingsTab::Controls => {
                                    self.render_controls_screen(ui, ctx);
                                }
                                SettingsTab::Shaders => {
                                    self.render_shaders_screen(ui, ctx);
                                }
                            }
                        });
                },
            );

            ui.separator();

            // ==========================================
            // FOOTER BAR: GERİ DÖN + KAYDET + LOG BUTTONS
            // ==========================================
            ui.allocate_ui_with_layout(
                egui::vec2(total_avail.x, footer_h),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.add_space(16.0);

                    // ◀ GERİ DÖN
                    let back_text = format!("◀  {}", self.t("menu_back"));
                    if render_button_feedback(ui, "btn_settings_back", &back_text, egui::vec2(130.0, 34.0), false).clicked() {
                        self.view_mode = ViewMode::MainMenu;
                        self.view_transition_start = std::time::Instant::now();
                    }

                    ui.add_space(16.0);

                    // 💾 KAYDET VE UYGULA
                    if render_button_feedback(ui, "btn_settings_save", &format!("💾 {}", self.t("save_apply")), egui::vec2(220.0, 34.0), false).clicked() {
                        self.apply_all_optimizations();
                    }

                    // Status banner if present
                    if let Some((ref msg, is_err)) = self.status_message {
                        ui.add_space(16.0);
                        let icon = if is_err { "⚠️" } else { "✅" };
                        let col = if is_err { egui::Color32::from_rgb(255, 140, 140) } else { egui::Color32::from_rgb(134, 239, 172) };
                        ui.label(egui::RichText::new(format!("{} {}", icon, msg)).color(col).strong().size(12.0));
                    }

                    // Right-aligned log and update buttons
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        if ui.button("📋 Launcher Logu").clicked() {
                            self.launcher_log_content = crate::logger::read_launcher_log(&self.base_dir);
                            self.show_launcher_log_modal = true;
                        }
                        ui.add_space(8.0);
                        if ui.button("📝 Oyun Logu").clicked() {
                            let log_path = self.base_dir.join("gow3.log");
                            self.log_content = std::fs::read_to_string(&log_path).unwrap_or_else(|_| "Log dosyası bulunamadı.".to_string());
                            self.show_log_modal = true;
                        }
                        ui.add_space(8.0);
                        if ui.button(format!("🔄 {}", self.t("check_updates"))).clicked() {
                            self.start_update_check();
                        }
                    });
                },
            );
        });
    }

    // ==========================================
    // SCREEN: SİSTEM ÖZELLİKLERİ (SYSTEM SPECS)
    // ==========================================
    pub fn render_system_specs_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        render_section_tag(ui, &format!("{} (CPU, GPU, VRAM, RAM, EKRAN HZ)", self.t("system_specs_title")));
        card_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new(format!("⚡ {}", self.t("system_specs_title")))
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 205, 255))
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🔄 Donanımı Yeniden Algıla").clicked() {
                        self.config.cached_hardware = crate::system::detect_hardware();
                        let _ = self.config.save(&self.base_dir);
                        self.status_message = Some(("Donanım özellikleri başarıyla güncellendi.".to_string(), false));
                    }
                    ui.add_space(6.0);
                    if ui.button(format!("🔄 {}", self.t("check_updates"))).clicked() {
                        self.start_update_check();
                    }
                    ui.add_space(6.0);
                    if ui.button("🛑 Kalan Süreçleri Kapat").clicked() {
                        self.kill_leftover_processes(Some(ui.ctx()));
                    }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(self.t("locked_auto")).color(egui::Color32::GRAY).size(11.0));
                });
            });
            ui.separator();

            let hw = &self.config.cached_hardware;

            let cpu_grade = crate::system::evaluate_cpu(&hw.cpu_name, hw.cpu_cores, hw.cpu_threads);
            let cpu_badge = cpu_grade.badge_text(&self.config.language);
            let (cpu_r, cpu_g, cpu_b) = cpu_grade.rgb();
            let cpu_color = egui::Color32::from_rgb(cpu_r, cpu_g, cpu_b);

            let (gpu_color, gpu_badge) = if hw.vram_mb >= 8192 {
                (egui::Color32::from_rgb(0, 230, 118), "• Üst Seviye (Yüksek FPS)".to_string())
            } else if hw.vram_mb >= 6144 {
                (egui::Color32::from_rgb(102, 225, 106), "• Güçlü (Akıcı Deneyim)".to_string())
            } else if hw.vram_mb >= 4096 {
                (egui::Color32::from_rgb(255, 213, 79), "• Orta Seviye (Standart)".to_string())
            } else {
                (egui::Color32::from_rgb(239, 83, 80), "• Giriş Seviye / Düşük VRAM".to_string())
            };

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

            let (hz_color, hz_badge) = if hw.refresh_rate_hz >= 120 {
                (egui::Color32::from_rgb(102, 225, 106), "• Yüksek Tazeleme (Ultra Akıcı)")
            } else {
                (egui::Color32::from_rgb(130, 215, 140), "• Akıcı Ekran (Standart 60Hz)")
            };

            // Row 1: CPU & GPU
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    let clean_cpu = crate::system::clean_cpu_name(&hw.cpu_name);
                    let cpu_val = if hw.cpu_threads > 0 && hw.cpu_cores > 0 {
                        format!("{} ({}C / {}T)", clean_cpu, hw.cpu_cores, hw.cpu_threads)
                    } else {
                        clean_cpu
                    };
                    render_stat_tile(
                        ui,
                        "CPU",
                        egui::Color32::from_rgb(70, 175, 255),
                        self.t("cpu"),
                        &cpu_val,
                        Some((cpu_badge, cpu_color)),
                    );
                });
                cols[1].vertical(|ui| {
                    render_stat_tile(
                        ui,
                        "GPU",
                        egui::Color32::from_rgb(0, 230, 118),
                        self.t("gpu"),
                        &hw.gpu_name,
                        Some((&gpu_badge, gpu_color)),
                    );
                });
            });

            ui.add_space(8.0);

            // Row 2: VRAM, RAM & EKRAN HZ
            ui.columns(3, |cols| {
                cols[0].vertical(|ui| {
                    let vram_val = format!("{} MB (~{} GB)", hw.vram_mb, hw.vram_mb / 1024);
                    render_stat_tile(
                        ui,
                        "VRAM",
                        egui::Color32::from_rgb(255, 215, 0),
                        self.t("vram"),
                        &vram_val,
                        Some((&vram_badge, vram_color)),
                    );
                });
                cols[1].vertical(|ui| {
                    let ram_val = format!("{} MB (~{} GB)", hw.ram_mb, hw.ram_mb / 1024);
                    render_stat_tile(
                        ui,
                        "RAM",
                        egui::Color32::from_rgb(180, 100, 255),
                        self.t("ram"),
                        &ram_val,
                        Some((&ram_badge, ram_color)),
                    );
                });
                cols[2].vertical(|ui| {
                    let hz_val = format!("{} Hz", hw.refresh_rate_hz);
                    render_stat_tile(
                        ui,
                        "HZ",
                        egui::Color32::from_rgb(255, 120, 60),
                        self.t("refresh_rate"),
                        &hz_val,
                        Some((hz_badge, hz_color)),
                    );
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);

            // Dynamic Hardware Analysis & Optimization Recommendation
            let effective_vram = self.config.get_effective_vram_budget();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("💡 Donanım Analizi & Öneri:").strong().color(egui::Color32::from_rgb(255, 215, 100)));
                    let rec_text = match cpu_grade {
                        crate::system::CpuGrade::Ultra | crate::system::CpuGrade::Good => {
                            "İşlemciniz güçlü. 144 FPS veya 240 FPS yamasıyla birlikte yüksek çözünürlük ölçekleyiciyi rahatça kullanabilirsiniz."
                        }
                        crate::system::CpuGrade::Medium => {
                            "İşlemciniz orta seviyede. 60 FPS veya 120 FPS modu ve FSR dengeli ölçekleme önerilir."
                        }
                        crate::system::CpuGrade::Weak => {
                            "İşlemciniz giriş seviyesinde. 60 FPS kilidi, FSR Performans modu ve shader arka plan derleme önerilir."
                        }
                    };
                    ui.label(egui::RichText::new(rec_text).size(11.5).color(egui::Color32::from_rgb(220, 225, 235)));
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!(
                        "Aktif Güvenli VRAM Bütçesi: {} MB (Donanım Limiti: {} MB)",
                        effective_vram, hw.vram_mb
                    )).size(11.0).color(egui::Color32::from_rgb(140, 220, 150)));
                });
            });
        });
    }

    // ==========================================
    // SCREEN 2: MODLAR (MODS & PATCHES)
    // ==========================================
    pub fn render_mods_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        render_section_tag(ui, self.t("tag_patches"));
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new(format!("📥 {}", self.t("nav_mods")))
                    .size(16.0)
                    .color(egui::Color32::from_rgb(150, 205, 255)),
            );
            ui.label(
                egui::RichText::new("God of War III performansı, akıcılığı ve sinematikleri için oyun yamaları")
                    .size(11.5)
                    .color(egui::Color32::from_rgb(175, 185, 205)),
            );
            ui.separator();

            // 1. Target FPS
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.t("target_fps")).strong().color(egui::Color32::WHITE).size(14.5));
                let fps_str = match self.config.target_fps {
                    30 => "30 FPS (Konsol Kilidi & Düşük Sistem - Maksimum Kararlılık)".to_string(),
                    40 => "40 FPS (Steam Deck 40Hz)".to_string(),
                    60 => "60 FPS (Varsayılan - Akıcı & Kararlı)".to_string(),
                    90 => "90 FPS (Steam Deck OLED 90Hz)".to_string(),
                    120 => "120 FPS (120Hz Ultra Akıcı - 60Hz Fizik Korumalı)".to_string(),
                    144 => "144 FPS (144Hz Ultra Akıcı - 60Hz Fizik Korumalı)".to_string(),
                    165 => "165 FPS (165Hz Ultra Akıcı - 60Hz Fizik Korumalı)".to_string(),
                    240 => "240 FPS (240Hz Maksimum - 60Hz Fizik Korumalı)".to_string(),
                    0 => "Sınırsız (Uncapped)".to_string(),
                    other => format!("{} FPS", other),
                };
                egui::ComboBox::from_id_salt("target_fps_combo")
                    .selected_text(fps_str)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config.target_fps, 30, "30 FPS (Konsol Kilidi & Düşük Sistem)");
                        ui.selectable_value(&mut self.config.target_fps, 40, "40 FPS (Steam Deck 40Hz)");
                        ui.selectable_value(&mut self.config.target_fps, 60, "60 FPS (Varsayılan - Akıcı & Kararlı)");
                        ui.selectable_value(&mut self.config.target_fps, 90, "90 FPS (Steam Deck OLED 90Hz)");
                        ui.selectable_value(&mut self.config.target_fps, 120, "120 FPS (120Hz Ultra Akıcı)");
                        ui.selectable_value(&mut self.config.target_fps, 144, "144 FPS (144Hz Ultra Akıcı)");
                        ui.selectable_value(&mut self.config.target_fps, 165, "165 FPS (165Hz Ultra Akıcı)");
                        ui.selectable_value(&mut self.config.target_fps, 240, "240 FPS (240Hz Maksimum - Ultra Hızlı)");
                        ui.selectable_value(&mut self.config.target_fps, 0, "Sınırsız (Uncapped)");
                    });
            });
            ui.label(egui::RichText::new("Oyunun hedef kare hızı sınırı. Delta-Time ve 60Hz fizik motor koruması sayesinde 240 FPS'e kadar oyun hızlanmadan ve sapıtmadan ultra akıcı çalışır.").color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 2. Skip Cinematics with X
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.skip_cinematics);
                let lbl = self.t("skip_cinematics");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.skip_cinematics = !self.config.skip_cinematics;
                }
            });
            ui.label(egui::RichText::new(self.t("skip_cinematics_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 3. Feral GameMode
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.use_gamemode);
                let lbl = self.t("gamemode_toggle");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.use_gamemode = !self.config.use_gamemode;
                }
                if !self.has_gamemode {
                    ui.label(egui::RichText::new("(Sistemde Gamemode Bulunamadı)").color(egui::Color32::YELLOW).size(11.0));
                }
            });
            ui.label(egui::RichText::new(self.t("gamemode_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 4. Delta Time Speed Patch
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.patch_delta_time);
                let lbl = "Delta Time Senkronizasyon Yaması (60-144 FPS)";
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.patch_delta_time = !self.config.patch_delta_time;
                }
            });
            ui.label(egui::RichText::new("Yüksek kare hızlarında (60-144 FPS) oyun hızının iki katına çıkmasını engelleyerek normal hızda çalışmasını sağlar.").color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 5. Disable Motion Blur
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.disable_motion_blur);
                let lbl = self.t("disable_motion_blur");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.disable_motion_blur = !self.config.disable_motion_blur;
                }
            });
            ui.label(egui::RichText::new(self.t("disable_motion_blur_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 6. Disable Depth of Field
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.disable_dof);
                let lbl = self.t("disable_dof");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.disable_dof = !self.config.disable_dof;
                }
            });
            ui.label(egui::RichText::new(self.t("disable_dof_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 7. Background Controller Input
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.background_controller_input);
                let lbl = self.t("bg_input");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.background_controller_input = !self.config.background_controller_input;
                }
            });
            ui.label(egui::RichText::new(self.t("bg_input_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 8. Hide Mouse Cursor
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.hide_mouse_cursor);
                let lbl = self.t("hide_mouse_cursor");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.hide_mouse_cursor = !self.config.hide_mouse_cursor;
                }
            });
            ui.label(egui::RichText::new(self.t("hide_mouse_cursor_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 9. Discord Rich Presence
            ui.horizontal(|ui| {
                if toggle_switch(ui, &mut self.config.discord_rpc_enabled).changed() {
                    self.discord_rpc.update_config(self.config.discord_rpc_enabled);
                }
                let lbl = self.t("discord_rpc_toggle");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.discord_rpc_enabled = !self.config.discord_rpc_enabled;
                    self.discord_rpc.update_config(self.config.discord_rpc_enabled);
                }
            });
            ui.label(egui::RichText::new(self.t("discord_rpc_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            if self.config.discord_rpc_enabled {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("🔒 Discord App: God of War III: Remastered (1440140183675867156)")
                            .size(12.5)
                            .strong()
                            .color(egui::Color32::from_rgb(160, 220, 180)),
                    );
                });
                ui.label(
                    egui::RichText::new("Bölümler, Poseidon/Hades/Zeus boss savaşları, mücadeleler ve arena durumu otomatik paylaşılır.")
                        .size(11.5)
                        .color(egui::Color32::from_rgb(170, 175, 195)),
                );
            }

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if render_button_feedback(ui, "btn_save_mods", &format!("💾 {}", self.t("save_apply")), egui::vec2(200.0, 34.0), false).clicked() {
                    self.apply_all_optimizations();
                }
            });
        });
    }

    // ==========================================
    // SCREEN 2.5: AYARLAR -> OYUN MODLARI (MOD MANAGER)
    // ==========================================
    pub fn render_custom_mods_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        render_section_tag(ui, self.t("tag_mods"));
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new(format!("📦 {}", self.t("mods_title")))
                    .size(16.0)
                    .color(egui::Color32::from_rgb(255, 215, 120)),
            );
            ui.label(
                egui::RichText::new(self.t("mods_desc"))
                    .size(12.5)
                    .color(egui::Color32::from_rgb(190, 195, 210)),
            );
            ui.separator();

            // Actions row: Open Folder & Refresh
            ui.horizontal(|ui| {
                if render_button_feedback(
                    ui,
                    "btn_open_mods_folder",
                    &format!("📂 {}", self.t("mods_open_folder")),
                    egui::vec2(210.0, 34.0),
                    false,
                )
                .clicked()
                {
                    crate::mods_manager::open_mods_folder(&self.base_dir);
                }

                if render_button_feedback(
                    ui,
                    "btn_refresh_mods_list",
                    &format!("🔄 {}", self.t("mods_refresh")),
                    egui::vec2(180.0, 34.0),
                    false,
                )
                .clicked()
                {
                    self.refresh_installed_mods();
                    self.status_message = Some((
                        format!("{} ({} mod)", self.t("mods_refresh"), self.installed_mods.len()),
                        false,
                    ));
                }
            });

            ui.add_space(8.0);

            // Summary badge bar
            let total_count = self.installed_mods.len();
            let active_count = self.installed_mods.iter().filter(|m| m.is_enabled).count();
            let disabled_count = total_count.saturating_sub(active_count);

            let summary_frame = egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(18, 22, 34, 220))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(100, 180, 255, 60)))
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::symmetric(10, 6));

            summary_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(
                            self.t("mods_count_summary")
                                .replacen("{}", &total_count.to_string(), 1)
                                .replacen("{}", &active_count.to_string(), 1)
                                .replacen("{}", &disabled_count.to_string(), 1),
                        )
                        .color(egui::Color32::from_rgb(160, 215, 255))
                        .size(13.0)
                        .strong(),
                    );
                });
            });

            ui.add_space(10.0);

            // Mods list or empty state
            if self.installed_mods.is_empty() {
                let empty_frame = egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(22, 24, 32, 200))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(16, 16));

                empty_frame.show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new(format!("📦 {}", self.t("mods_empty_title")))
                                .color(egui::Color32::from_rgb(255, 215, 120))
                                .size(14.5)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(self.t("mods_empty_desc"))
                                .color(egui::Color32::from_rgb(190, 195, 210))
                                .size(12.5),
                        );
                    });
                });
            } else {
                let mut toggled_mod: Option<(String, bool)> = None;

                for m in &self.installed_mods {
                    let is_active = m.is_enabled;
                    let mod_frame = egui::Frame::new()
                        .fill(if is_active {
                            egui::Color32::from_rgba_unmultiplied(18, 25, 38, 230)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(18, 19, 24, 180)
                        })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if is_active {
                                egui::Color32::from_rgba_unmultiplied(100, 180, 255, 90)
                            } else {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20)
                            },
                        ))
                        .corner_radius(egui::CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(12, 10));

                    mod_frame.show(ui, |ui| {
                        // Line 1: Switch, Icon, Title, Version, Status Badge
                        ui.horizontal(|ui| {
                            let mut checked = is_active;
                            if toggle_switch(ui, &mut checked).changed() {
                                toggled_mod = Some((m.folder_name.clone(), checked));
                            }

                            let icon = if m.folder_name.to_lowercase().contains("cj") {
                                "🦸‍♂️"
                            } else if m.folder_name.to_lowercase().contains("blade") {
                                "⚔️"
                            } else if m.folder_name.to_lowercase().contains("costume") || m.folder_name.to_lowercase().contains("skin") {
                                "🎭"
                            } else {
                                "📦"
                            };

                            let title_resp = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{} {}", icon, m.title))
                                        .size(15.0)
                                        .strong()
                                        .color(if is_active {
                                            egui::Color32::WHITE
                                        } else {
                                            egui::Color32::from_rgb(160, 165, 175)
                                        }),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if title_resp.clicked() {
                                toggled_mod = Some((m.folder_name.clone(), !is_active));
                            }
                            if title_resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            ui.label(
                                egui::RichText::new(format!("[{}]", m.version))
                                    .size(11.5)
                                    .color(egui::Color32::from_rgb(150, 175, 210)),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if is_active {
                                    ui.label(
                                        egui::RichText::new(self.t("mods_status_active"))
                                            .size(12.5)
                                            .color(egui::Color32::from_rgb(134, 239, 172))
                                            .strong(),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new(self.t("mods_status_disabled"))
                                            .size(12.5)
                                            .color(egui::Color32::from_rgb(200, 110, 110)),
                                    );
                                }
                            });
                        });

                        // Line 2: Description (if any)
                        if !m.description.is_empty() {
                            ui.add_space(3.0);
                            ui.label(
                                egui::RichText::new(&m.description)
                                    .size(12.5)
                                    .color(egui::Color32::from_rgb(190, 195, 210)),
                            );
                        }

                        // Line 3: Author and Replaced files info
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("👤 {}: {}", self.t("mods_author"), m.author))
                                    .size(11.5)
                                    .color(egui::Color32::from_rgb(255, 215, 120)),
                            );

                            ui.label(egui::RichText::new("•").color(egui::Color32::DARK_GRAY));

                            let samples_str = if !m.sample_files.is_empty() {
                                format!(" (örn. {})", m.sample_files.join(", "))
                            } else {
                                String::new()
                            };

                            ui.label(
                                egui::RichText::new(format!(
                                    "🎯 {}: {}{}",
                                    self.t("mods_files_replaced"),
                                    m.file_count,
                                    samples_str
                                ))
                                .size(11.5)
                                .color(egui::Color32::from_rgb(160, 210, 255)),
                            );
                        });
                    });

                    ui.add_space(6.0);
                }

                // If user toggled a mod, update config and state
                if let Some((folder, enabled)) = toggled_mod {
                    if let Some(item) = self.installed_mods.iter_mut().find(|x| x.folder_name == folder) {
                        item.is_enabled = enabled;
                    }

                    if enabled {
                        self.config.disabled_mods.retain(|x| x != &folder);
                    } else if !self.config.disabled_mods.contains(&folder) {
                        self.config.disabled_mods.push(folder.clone());
                    }

                    let _ = self.config.save(&self.base_dir);
                    self.status_message = Some((
                        format!("'{}' modu {}", folder, if enabled { "aktif edildi" } else { "devre dışı bırakıldı" }),
                        false,
                    ));
                }
            }

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("💡 Bilgi: Oyun modları sanal katman (Overlay FS) üzerinden dinamik aktarılır. Orijinal oyun dosyalarınız asla bozulmaz veya değişmez.")
                        .size(10.5)
                        .color(egui::Color32::from_rgb(140, 150, 170)),
                );
            });
        });
    }

    // ==========================================
    // SCREEN 3: AYARLAR -> GRAFİKLER
    // ==========================================
    pub fn render_graphics_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        render_section_tag(ui, self.t("graphics_header"));
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new(format!("⚙️ {}", self.t("nav_graphics")))
                    .size(17.5)
                    .color(egui::Color32::from_rgb(150, 205, 255))
                    .strong(),
            );
            ui.label(
                egui::RichText::new(self.t("graphics_subtitle"))
                    .size(13.0)
                    .color(egui::Color32::from_rgb(185, 195, 215)),
            );
            ui.separator();

            // 1. Hazır Grafik Profili
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.t("graphic_preset")).strong().color(egui::Color32::WHITE).size(14.5));
                let current_preset_text = match self.config.graphic_preset {
                    crate::config::GraphicPreset::UltraLow => self.t("preset_ultra_low"),
                    crate::config::GraphicPreset::Low => self.t("preset_low"),
                    crate::config::GraphicPreset::SteamDeck => self.t("preset_steam_deck"),
                    crate::config::GraphicPreset::Balanced => self.t("preset_balanced"),
                    crate::config::GraphicPreset::Ultra => self.t("preset_ultra"),
                    crate::config::GraphicPreset::Custom => self.t("preset_custom"),
                };

                egui::ComboBox::from_id_salt("graphic_preset_combo")
                    .selected_text(current_preset_text)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.config.graphic_preset == crate::config::GraphicPreset::UltraLow, self.t("preset_ultra_low")).clicked() {
                            self.config.apply_preset(crate::config::GraphicPreset::UltraLow);
                        }
                        if ui.selectable_label(self.config.graphic_preset == crate::config::GraphicPreset::Low, self.t("preset_low")).clicked() {
                            self.config.apply_preset(crate::config::GraphicPreset::Low);
                        }
                        if ui.selectable_label(self.config.graphic_preset == crate::config::GraphicPreset::SteamDeck, self.t("preset_steam_deck")).clicked() {
                            self.config.apply_preset(crate::config::GraphicPreset::SteamDeck);
                        }
                        if ui.selectable_label(self.config.graphic_preset == crate::config::GraphicPreset::Balanced, self.t("preset_balanced")).clicked() {
                            self.config.apply_preset(crate::config::GraphicPreset::Balanced);
                        }
                        if ui.selectable_label(self.config.graphic_preset == crate::config::GraphicPreset::Ultra, self.t("preset_ultra")).clicked() {
                            self.config.apply_preset(crate::config::GraphicPreset::Ultra);
                        }
                        if ui.selectable_label(self.config.graphic_preset == crate::config::GraphicPreset::Custom, self.t("preset_custom")).clicked() {
                            self.config.graphic_preset = crate::config::GraphicPreset::Custom;
                        }
                    });
            });
            ui.label(egui::RichText::new(self.t("preset_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 2. Render Resolution
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.t("render_resolution")).strong().color(egui::Color32::WHITE).size(14.5));
                let res_text = match self.config.render_resolution.as_str() {
                    "480p" => "480p (854x480 - Ultra Düşük Donanım / Yüksek FPS)",
                    "720p" => "720p (1280x720 - Düşük Sistem & Steam Deck)",
                    "800p" => "800p (1280x800 - Steam Deck 16:10)",
                    "native" => "Native 1080p (1920x1080 - Orijinal Kalite)",
                    "1440p" => "1440p (2560x1440 - 2K QHD)",
                    "4K" => "4K (3840x2160 - Ultra Yüksek Çözünürlük)",
                    _ => "Native 1080p (1920x1080)",
                };

                egui::ComboBox::from_id_salt("render_resolution_combo")
                    .selected_text(res_text)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.config.render_resolution == "480p", "480p (854x480)").clicked() {
                            self.config.render_resolution = "480p".to_string();
                            self.config.check_preset_match();
                        }
                        if ui.selectable_label(self.config.render_resolution == "720p", "720p (1280x720)").clicked() {
                            self.config.render_resolution = "720p".to_string();
                            self.config.check_preset_match();
                        }
                        if ui.selectable_label(self.config.render_resolution == "800p", "800p (1280x800)").clicked() {
                            self.config.render_resolution = "800p".to_string();
                            self.config.check_preset_match();
                        }
                        if ui.selectable_label(self.config.render_resolution == "native", "Native 1080p (1920x1080)").clicked() {
                            self.config.render_resolution = "native".to_string();
                            self.config.check_preset_match();
                        }
                        if ui.selectable_label(self.config.render_resolution == "1440p", "1440p (2560x1440)").clicked() {
                            self.config.render_resolution = "1440p".to_string();
                            self.config.check_preset_match();
                        }
                        if ui.selectable_label(self.config.render_resolution == "4K", "4K (3840x2160)").clicked() {
                            self.config.render_resolution = "4K".to_string();
                            self.config.check_preset_match();
                        }
                    });
            });
            ui.label(egui::RichText::new(self.t("res_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 3. AMD FSR
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.fsr_enabled);
                let lbl = self.t("fsr_toggle");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.fsr_enabled = !self.config.fsr_enabled;
                    self.config.check_preset_match();
                }
            });
            ui.label(egui::RichText::new(self.t("fsr_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 4. Texture Corruption & Yellow Tint Fix (patch_readbacks)
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.patch_readbacks);
                let lbl = self.t("readback_patch");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.patch_readbacks = !self.config.patch_readbacks;
                }
            });
            ui.label(egui::RichText::new(self.t("readback_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 5. Asynchronous Readbacks (async_readbacks)
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.async_readbacks);
                let lbl = self.t("async_readbacks");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.async_readbacks = !self.config.async_readbacks;
                }
            });
            ui.label(egui::RichText::new(self.t("async_readbacks_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 6. VRAM GC Budget
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.t("vram_budget")).strong().color(egui::Color32::WHITE).size(14.5));
                let vram_text = match self.config.vram_gc_budget_mb {
                    1280 => "1280 MB (2 GB GPU / Düşük Sistem - PCIe Taşma Korumalı)".to_string(),
                    2048 => "2048 MB (3 GB GPU / Steam Deck)".to_string(),
                    3072 => "3072 MB (4 GB GPU)".to_string(),
                    4096 => "4096 MB (6 GB GPU)".to_string(),
                    5120 => "5120 MB (8 GB GPU - Önerilen / Kararlı)".to_string(),
                    6144 => "6144 MB (10-12 GB GPU)".to_string(),
                    8192 => "8192 MB (12-16 GB+ GPU / Yüksek)".to_string(),
                    other => format!("{} MB", other),
                };
                egui::ComboBox::from_id_salt("vram_gc_combo")
                    .selected_text(vram_text)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 1280, "1280 MB (2 GB GPU / Düşük Sistem - PCIe Taşma Korumalı)");
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 2048, "2048 MB (3 GB GPU / Steam Deck)");
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 3072, "3072 MB (4 GB GPU)");
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 4096, "4096 MB (6 GB GPU)");
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 5120, "5120 MB (8 GB GPU - Önerilen / Kararlı)");
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 6144, "6144 MB (10-12 GB GPU)");
                        ui.selectable_value(&mut self.config.vram_gc_budget_mb, 8192, "8192 MB (12-16 GB+ GPU / Yüksek)");
                    });
            });
            ui.label(egui::RichText::new(self.t("vram_budget_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            let effective = self.config.get_effective_vram_budget();
            if effective != self.config.vram_gc_budget_mb && self.config.cached_hardware.vram_mb > 0 {
                ui.label(egui::RichText::new(format!(
                    "🛡️ Donanım Koruması Devrede: Ekran kartınızın kapasitesine ({} MB) göre aktif VRAM bütçesi {} MB olarak güvenli şekilde sınırlandırıldı.",
                    self.config.cached_hardware.vram_mb, effective
                )).color(egui::Color32::from_rgb(102, 225, 106)).size(12.0));
            }

            ui.add_space(8.0);
            // 7. Display Mode & VSync
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(self.t("display_mode")).strong().color(egui::Color32::WHITE).size(14.5));
                let mode_borderless = self.t("display_borderless");
                let mode_fullscreen = self.t("display_fullscreen");
                let mode_windowed = self.t("display_windowed");
                let current_display_text = match self.config.display_mode {
                    DisplayMode::Borderless => mode_borderless,
                    DisplayMode::Fullscreen => mode_fullscreen,
                    DisplayMode::Windowed => mode_windowed,
                };
                egui::ComboBox::from_id_salt("disp_mode_cb")
                    .selected_text(current_display_text)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config.display_mode, DisplayMode::Borderless, mode_borderless);
                        ui.selectable_value(&mut self.config.display_mode, DisplayMode::Fullscreen, mode_fullscreen);
                        ui.selectable_value(&mut self.config.display_mode, DisplayMode::Windowed, mode_windowed);
                    });

                ui.add_space(16.0);
                toggle_switch(ui, &mut self.config.vsync);
                let vsync_lbl = self.t("vsync");
                if ui.add(egui::Label::new(egui::RichText::new(vsync_lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.vsync = !self.config.vsync;
                }
            });
            ui.label(egui::RichText::new(self.t("display_mode_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // 8. Hide Mouse Cursor
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.hide_mouse_cursor);
                let lbl = self.t("hide_mouse_cursor");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.hide_mouse_cursor = !self.config.hide_mouse_cursor;
                }
            });
            ui.label(egui::RichText::new(self.t("hide_mouse_cursor_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if render_button_feedback(ui, "btn_save_gfx", &format!("💾 {}", self.t("save_apply")), egui::vec2(200.0, 34.0), false).clicked() {
                    self.apply_all_optimizations();
                }
            });
        });
    }

    // ==========================================
    // SCREEN 4: AYARLAR -> OYUN KONUMU
    // ==========================================
    pub fn render_game_location_screen(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        render_section_tag(ui, self.t("tag_game_location"));
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new(format!("📁 {}", self.t("game_location")))
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
                ui.label(egui::RichText::new(dir_display).monospace().size(13.5).color(egui::Color32::WHITE));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if render_button_feedback(ui, "btn_browse", self.t("manual_select"), egui::vec2(120.0, 30.0), false).clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.set_game_directory(&folder, ctx);
                        }
                    }

                    if self.is_scanning {
                        ui.spinner();
                        ui.label(self.t("scanning"));
                    } else if render_button_feedback(ui, "btn_scan", self.t("auto_scan"), egui::vec2(120.0, 30.0), false).clicked() {
                        self.start_game_scan();
                    }
                });
            });

            ui.add_space(4.0);
            if self.is_game_valid {
                ui.label(
                    egui::RichText::new(self.t("game_valid"))
                        .color(egui::Color32::from_rgb(105, 225, 110))
                        .size(13.0)
                        .strong(),
                );
            } else {
                ui.label(
                    egui::RichText::new(self.t("game_invalid"))
                        .color(egui::Color32::from_rgb(245, 90, 85))
                        .size(13.0)
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
                            ui.label(egui::RichText::new("[ ! ]").size(18.0));
                            ui.label(
                                egui::RichText::new(self.t("scan_not_found_inline"))
                                .color(egui::Color32::from_rgb(255, 140, 140))
                                .size(13.0)
                                .strong(),
                            );
                        });
                    });
            }
        });

        ui.add_space(10.0);
        // Engine & Python components
        render_section_tag(ui, "MOTOR VE ÇALIŞMA ORTAMI BİLEŞENLERİ");
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new("⚡ Sistem ve Çalışma Ortamı Bileşenleri")
                    .size(15.0)
                    .color(egui::Color32::from_rgb(150, 205, 255)),
            );
            ui.separator();

            // 1. Python
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🐍 Python 3 Ortamı:").strong().color(egui::Color32::WHITE).size(14.5));
                if self.has_python {
                    ui.label(egui::RichText::new("✅ Sistemde Hazır").color(egui::Color32::from_rgb(105, 225, 110)).size(13.5).strong());
                } else if self.is_downloading_python {
                    ui.spinner();
                    ui.label(egui::RichText::new("İndiriliyor...").color(egui::Color32::YELLOW).size(13.0));
                } else {
                    ui.label(egui::RichText::new("⚠️ Eksik").color(egui::Color32::from_rgb(245, 90, 85)).size(13.5).strong());
                    if render_button_feedback(ui, "btn_py_dl", "Python Ortamını İndir", egui::vec2(160.0, 26.0), false).clicked() {
                        self.start_python_download();
                    }
                }
            });

            ui.add_space(6.0);
            // 2. Engine
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚙️ GOW3 Port Motoru:").strong().color(egui::Color32::WHITE).size(14.5));
                if self.has_engine {
                    ui.label(egui::RichText::new("✅ Motor Hazır").color(egui::Color32::from_rgb(105, 225, 110)).size(13.5).strong());
                } else if self.is_downloading_engine {
                    ui.spinner();
                    ui.label(egui::RichText::new("İndiriliyor...").color(egui::Color32::YELLOW).size(13.0));
                } else {
                    ui.label(egui::RichText::new("⚠️ Eksik").color(egui::Color32::from_rgb(245, 90, 85)).size(13.5).strong());
                    if render_button_feedback(ui, "btn_eng_dl", "Motoru İndir", egui::vec2(140.0, 26.0), false).clicked() {
                        self.start_engine_download(false);
                    }
                    ui.add_space(4.0);
                    if render_button_feedback(ui, "btn_eng_alt", "Alternatif İndir", egui::vec2(140.0, 26.0), false).clicked() {
                        self.start_engine_download(true);
                    }
                }
            });
        });
    }

    // ==========================================
    // SCREEN 5: AYARLAR -> KONTROLLER
    // ==========================================
    pub fn render_controls_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        // Gamepad settings
        render_section_tag(ui, self.t("tag_controller"));
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new(self.t("controller_header"))
                    .size(16.0)
                    .color(egui::Color32::from_rgb(150, 205, 255)),
            );
            ui.separator();

            // Motion controls toggle
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.is_motion_controls_enabled);
                let motion_lbl = self.t("motion_controls");
                if ui.add(egui::Label::new(egui::RichText::new(motion_lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.is_motion_controls_enabled = !self.config.is_motion_controls_enabled;
                }
            });
            ui.label(egui::RichText::new(self.t("motion_controls_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(8.0);
            // Analog stick deadzones
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(egui::RichText::new(self.t("deadzone_left")).strong().color(egui::Color32::WHITE).size(14.0));
                    ui.add(egui::Slider::new(&mut self.config.deadzone_left, 1..=40).text(""));
                });
                cols[1].vertical(|ui| {
                    ui.label(egui::RichText::new(self.t("deadzone_right")).strong().color(egui::Color32::WHITE).size(14.0));
                    ui.add(egui::Slider::new(&mut self.config.deadzone_right, 1..=40).text(""));
                });
            });
            ui.label(egui::RichText::new(self.t("deadzone_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));
        });

        ui.add_space(10.0);
        // Keybindings matrix
        render_section_tag(ui, self.t("tag_keyboard"));
        card_frame.show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new(self.t("keyboard_header"))
                        .size(16.0)
                        .color(egui::Color32::from_rgb(150, 205, 255)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if render_button_feedback(ui, "btn_kb_reset", self.t("reset_defaults"), egui::vec2(130.0, 26.0), false).clicked() {
                        self.config.keyboard_bindings = KeyboardBindings::recommended();
                    }

                    if render_button_feedback(ui, "btn_kb_mouse", self.t("preset_mouse_kb"), egui::vec2(150.0, 26.0), false).clicked() {
                        self.config.keyboard_bindings = KeyboardBindings::mouse_keyboard();
                    }

                    if render_button_feedback(ui, "btn_kb_rec", self.t("preset_recommended"), egui::vec2(150.0, 26.0), false).clicked() {
                        self.config.keyboard_bindings = KeyboardBindings::recommended();
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

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if render_button_feedback(ui, "btn_save_kb", &format!("💾 {}", self.t("save_apply")), egui::vec2(200.0, 34.0), false).clicked() {
                    self.apply_all_optimizations();
                }
            });
        });
    }

    // ==========================================
    // SCREEN 6: AYARLAR -> SHADER AYARI
    // ==========================================
    pub fn render_shaders_screen(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        let card_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgba_unmultiplied(14, 15, 23, 235))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
            .inner_margin(egui::Margin::symmetric(14, 12))
            .corner_radius(egui::CornerRadius::same(10));

        render_section_tag(ui, self.t("shader_cache_title"));
        card_frame.show(ui, |ui| {
            ui.heading(
                egui::RichText::new(self.t("shader_cache_title"))
                    .size(16.0)
                    .color(egui::Color32::from_rgb(255, 215, 120)),
            );
            ui.label(
                egui::RichText::new(self.t("shader_cache_desc"))
                    .size(12.5)
                    .color(egui::Color32::from_rgb(190, 195, 210)),
            );
            ui.separator();

            // 1. SHADER CACHE PRE-CACHING STATUS BADGE
            let has_full_cache = self.cached_shader_count >= 20000;
            let badge_text = if has_full_cache {
                self.t("shader_installed_badge").replace("{}", &self.cached_shader_count.to_string())
            } else {
                self.t("shader_missing_badge").replace("{}", &self.cached_shader_count.to_string())
            };

            let badge_frame = egui::Frame::new()
                .fill(if has_full_cache {
                    egui::Color32::from_rgba_unmultiplied(20, 50, 30, 220)
                } else {
                    egui::Color32::from_rgba_unmultiplied(65, 45, 20, 220)
                })
                .stroke(egui::Stroke::new(
                    1.0,
                    if has_full_cache {
                        egui::Color32::from_rgb(34, 197, 94)
                    } else {
                        egui::Color32::from_rgb(234, 179, 8)
                    },
                ))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 8));

            badge_frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(&badge_text)
                            .color(if has_full_cache {
                                egui::Color32::from_rgb(134, 239, 172)
                            } else {
                                egui::Color32::from_rgb(254, 240, 138)
                            })
                            .strong()
                            .size(13.5),
                    );
                });
            });

            ui.add_space(8.0);

            // 2. DOWNLOAD & INSTALL BUTTON
            if self.is_installing_shaders {
                ui.horizontal(|ui| {
                    ui.spinner();
                    let status = self.shader_install_status.as_deref().unwrap_or(self.t("shader_downloading"));
                    ui.label(egui::RichText::new(status).color(egui::Color32::from_rgb(255, 225, 120)).size(13.0).strong());
                });
            } else {
                let dl_btn_text = if has_full_cache {
                    self.t("shader_download_btn_update")
                } else {
                    self.t("shader_download_btn")
                };

                let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width().min(420.0), 38.0), egui::Sense::click());
                let hov = resp.hovered();
                let hov_anim = ui.ctx().animate_bool(resp.id.with("hov"), hov);
                if hov {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                let is_pressed = resp.is_pointer_button_down_on();
                let scale = if is_pressed { -1.0 } else { hov_anim * 1.5 };
                let draw_rect = rect.expand(scale);

                let bg = if hov_anim > 0.01 {
                    egui::Color32::from_rgb(
                        egui::lerp(35.0..=55.0, hov_anim) as u8,
                        egui::lerp(55.0..=75.0, hov_anim) as u8,
                        egui::lerp(85.0..=120.0, hov_anim) as u8,
                    )
                } else {
                    egui::Color32::from_rgb(30, 48, 72)
                };
                let border = if hov_anim > 0.01 {
                    egui::Color32::from_rgb(100, 180, 255)
                } else {
                    egui::Color32::from_rgba_unmultiplied(70, 130, 200, 140)
                };

                let painter = ui.painter();
                painter.rect_filled(draw_rect, 8.0, bg);
                painter.rect_stroke(draw_rect, 8.0, egui::Stroke::new(1.2, border), egui::StrokeKind::Inside);
                painter.text(
                    draw_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    dl_btn_text,
                    egui::FontId::proportional(13.5),
                    egui::Color32::WHITE,
                );

                if resp.clicked() {
                    self.start_shader_install();
                }
            }

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(6.0);

            // 3. LAUNCH WHILE COMPILING SHADERS TOGGLE
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.launch_while_compiling_shaders);
                let lbl = self.t("launch_while_compiling_shaders");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.launch_while_compiling_shaders = !self.config.launch_while_compiling_shaders;
                }
            });
            ui.label(egui::RichText::new(self.t("launch_while_compiling_shaders_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(10.0);

            // 4. SHOW SHADER COMPILE RAGE METER TOGGLE
            ui.horizontal(|ui| {
                toggle_switch(ui, &mut self.config.show_shader_compile_rage_meter);
                let lbl = self.t("show_shader_rage_meter");
                if ui.add(egui::Label::new(egui::RichText::new(lbl).strong().color(egui::Color32::WHITE).size(14.5)).sense(egui::Sense::click())).clicked() {
                    self.config.show_shader_compile_rage_meter = !self.config.show_shader_compile_rage_meter;
                }
            });
            ui.label(egui::RichText::new(self.t("show_shader_rage_meter_desc")).color(egui::Color32::from_rgb(190, 195, 210)).size(12.5));

            ui.add_space(14.0);
            // SHADER SCREEN PREVIEW INFO
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 22, 32, 200))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(235, 175, 75, 80)))
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if self.config.show_shader_compile_rage_meter {
                            if let Some(ref rage_icon) = self.rage_full_texture {
                                ui.add(egui::Image::new(rage_icon).max_height(48.0));
                                ui.add_space(10.0);
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("⚔️ Spartan Rage Shader Yükleme Ekranı (Aktif)")
                                        .size(13.5)
                                        .color(egui::Color32::from_rgb(255, 215, 120))
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new("Oyun başlatılırken shader'lar önceden derlenir ve God of War III Spartan Rage barı yukarı doğru dolar. Derleme %100 bittiğinde oyun penceresi açılana kadar siyah ekranda bekler.")
                                        .size(12.5)
                                        .color(egui::Color32::from_rgb(205, 210, 225)),
                                );
                            });
                        } else {
                            if let Some(ref bg_icon) = self.bg_texture {
                                ui.add(egui::Image::new(bg_icon).max_height(48.0));
                                ui.add_space(10.0);
                            }
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("🎮 PS4 Konsol Tarzı Açılış Ekranı (pic1.png)")
                                        .size(13.5)
                                        .color(egui::Color32::from_rgb(130, 205, 255))
                                        .strong(),
                                );
                                ui.label(
                                    egui::RichText::new("Shader derleme barları ve sayaçlar gizlenir. Oyun penceresi açılana kadar ekranda tam ekran pürüzsüz yaklaşan animasyonlu PS4 resmi (pic1.png) görüntülenir.")
                                        .size(12.5)
                                        .color(egui::Color32::from_rgb(205, 210, 225)),
                                );
                            });
                        }
                    });
                });

            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if render_button_feedback(ui, "btn_save_shaders", &format!("💾 {}", self.t("save_apply")), egui::vec2(200.0, 34.0), false).clicked() {
                    self.apply_all_optimizations();
                }
            });
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
                    match event {
                        egui::Event::Key { key, pressed: true, .. } => {
                            if *key == egui::Key::Escape {
                                cancel = true;
                            } else if let Some(numpad_key) = check_physical_numpad() {
                                captured_key = Some(numpad_key.to_string());
                            } else {
                                // Direct detection: map number and keypad symbols to Numpad bindings
                                let mapped = match *key {
                                    egui::Key::Num0 => Some("kp0"),
                                    egui::Key::Num1 => Some("kp1"),
                                    egui::Key::Num2 => Some("kp2"),
                                    egui::Key::Num3 => Some("kp3"),
                                    egui::Key::Num4 => Some("kp4"),
                                    egui::Key::Num5 => Some("kp5"),
                                    egui::Key::Num6 => Some("kp6"),
                                    egui::Key::Num7 => Some("kp7"),
                                    egui::Key::Num8 => Some("kp8"),
                                    egui::Key::Num9 => Some("kp9"),
                                    egui::Key::Plus => Some("kpplus"),
                                    egui::Key::Minus => Some("kpminus"),
                                    egui::Key::Slash => Some("kpslash"),
                                    egui::Key::Period => Some("kpperiod"),
                                    other => egui_key_to_shadps4(other),
                                };
                                if let Some(k_str) = mapped {
                                    captured_key = Some(k_str.to_string());
                                }
                            }
                        }
                        egui::Event::PointerButton { button, pressed: true, .. } => {
                            match button {
                                egui::PointerButton::Secondary => captured_key = Some("rightbutton".to_string()),
                                egui::PointerButton::Middle => captured_key = Some("middlebutton".to_string()),
                                egui::PointerButton::Extra1 => captured_key = Some("sidebuttonback".to_string()),
                                egui::PointerButton::Extra2 => captured_key = Some("sidebuttonforward".to_string()),
                                _ => {}
                            }
                        }
                        egui::Event::MouseWheel { delta, .. } => {
                            if delta.y > 6.0 {
                                captured_key = Some("mousewheelup".to_string());
                            } else if delta.y < -6.0 {
                                captured_key = Some("mousewheeldown".to_string());
                            }
                        }
                        _ => {}
                    }
                }
            });

            if cancel {
                self.listening_key_for = None;
            } else if let Some(new_key) = captured_key {
                apply_binding_to_config(&mut self.config, target, new_key);
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
            if let Ok(res) = rx.try_recv() {
                self.is_game_running = false;
                self.game_exit_rx = None;
                self.discord_rpc.set_in_menu(&self.config.language);
                if self.shader_loading.is_some() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                }
                self.shader_loading = None;

                match res {
                    Ok(exit_code) => {
                        crate::log_info!("Game process ended with exit code: {}", exit_code);
                        if let Some(diagnosis) = analyze_post_game(&self.base_dir, Some(exit_code)) {
                            self.active_crash_diagnosis = Some(diagnosis);
                        }
                    }
                    Err(err) => {
                        crate::log_error!("Game failed to run: {}", err);
                        self.status_message = Some((format!("Başlatma hatası: {}", err), true));
                        self.error_modal = Some(ErrorModalData {
                            title: self.t("err_game_launch_title").to_string(),
                            summary: self.t("err_game_launch_summary").to_string(),
                            details: err,
                            log_hint: Some(self.t("err_game_launch_hint").to_string()),
                        });
                    }
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
                        self.cached_update_info = Some(info.clone());
                        if info.is_new {
                            self.has_new_update = true;
                            if !self.update_dismissed_this_session {
                                self.update_modal = Some(info);
                            }
                        } else if !self.silent_update_check {
                            self.update_modal = Some(info);
                        }
                    }
                    Err(e) => {
                        if !self.silent_update_check {
                            self.status_message = Some((format!("Güncelleme hatası: {}", e), true));
                        }
                    }
                }
            }
        }

        // 4b. Check background update applying thread
        if let Some(ref rx) = self.update_apply_rx {
            if let Ok((res, new_config)) = rx.try_recv() {
                self.is_applying_update = false;
                self.update_apply_rx = None;
                match res {
                    Ok(success_msg) => {
                        self.config = new_config;
                        let _ = self.config.save(&self.base_dir);
                        self.has_new_update = false;
                        if let Some(ref info) = self.cached_update_info {
                            if let Some(ref tag) = info.version_tag {
                                self.launcher_version = tag.clone();
                            }
                        }
                        self.update_modal = None;
                        self.status_message = Some((success_msg, false));
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Güncelleme hatası: {}", e), true));
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
                        crate::log_info!("Python 3 environment successfully configured.");
                    }
                    Err(e) => {
                        self.status_message = Some((format!("{}{}", self.t("python_download_err"), e), true));
                        crate::log_error!("Python 3 download failed: {}", e);
                        self.error_modal = Some(ErrorModalData {
                            title: self.t("err_python_dl_title").to_string(),
                            summary: self.t("err_python_dl_summary").to_string(),
                            details: e,
                            log_hint: Some(self.t("err_python_dl_hint").to_string()),
                        });
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
                        crate::log_info!("Game engine installed successfully.");
                    }
                    Err(e) => {
                        self.status_message = Some((format!("Hata: {}", e), true));
                        crate::log_error!("Game engine download failed: {}", e);
                        self.error_modal = Some(ErrorModalData {
                            title: self.t("err_engine_dl_title").to_string(),
                            summary: self.t("err_engine_dl_summary").to_string(),
                            details: e,
                            log_hint: Some(self.t("err_engine_dl_hint").to_string()),
                        });
                    }
                }
            }
        }

        // 6b. Check Shader cache installer thread
        if let Some(ref rx) = self.shader_install_rx {
            if let Ok(res) = rx.try_recv() {
                self.is_installing_shaders = false;
                self.shader_install_rx = None;
                match res {
                    Ok(count) => {
                        self.cached_shader_count = count;
                        let msg = self.t("shader_success").replace("{}", &count.to_string());
                        self.shader_install_status = Some(msg.clone());
                        self.status_message = Some((msg, false));
                        crate::log_info!("Shader cache pre-caching successful. Total shaders: {}", count);
                    }
                    Err(e) => {
                        let err_msg = format!("Hata: {}", e);
                        self.shader_install_status = Some(err_msg.clone());
                        self.status_message = Some((err_msg, true));
                        crate::log_error!("Shader cache pre-caching failed: {}", e);
                    }
                }
            }
        }

        // 6c. Check GitHub release notes fetch thread
        if let Some(ref rx) = self.release_notes_rx {
            if let Ok(res) = rx.try_recv() {
                self.is_fetching_release_notes = false;
                self.release_notes_rx = None;
                match res {
                    Ok(notes) => {
                        self.github_release_notes = Some(notes);
                    }
                    Err(e) => {
                        eprintln!("[Launcher] GitHub release notes fetch failed: {}", e);
                    }
                }
            }
        }

        // --- 7. SHADER LOADING SCREEN PROGRESS & ANIMATION ---
        if let Some(ref mut sl) = self.shader_loading {
            // Kesintisiz yüksek kare hızında (60-144+ FPS) pürüzsüz sinematik animasyon
            ctx.request_repaint();

            // Poll progress updates from monitor thread
            let mut game_ready_now = false;
            while let Ok(msg) = sl.rx.try_recv() {
                if msg.total > 0 {
                    sl.total_shaders = msg.total;
                }
                if msg.current > 0 {
                    sl.current_shaders = msg.current;
                }
                if msg.progress > sl.target_progress {
                    sl.target_progress = msg.progress;
                }
                if msg.is_complete {
                    sl.is_complete = true;
                    sl.target_progress = 1.0;
                    if sl.completed_at.is_none() {
                        sl.completed_at = Some(std::time::Instant::now());
                    }
                }
                if msg.game_window_ready {
                    game_ready_now = true;
                }
            }

            let elapsed = sl.start_time.elapsed().as_secs_f32();
            let pid = self.running_pid.load(Ordering::SeqCst);

            // Auto-detect game window on Windows / Linux if rendered
            if !game_ready_now && pid > 0 && elapsed > 2.0 {
                if crate::win_input::find_game_hwnd(pid).is_some() {
                    game_ready_now = true;
                }
            }

            // Auto-complete triggers if game engine is up and running or after warmup phase
            if (pid > 0 && elapsed > 5.5) || elapsed > 8.5 {
                sl.is_complete = true;
                sl.target_progress = 1.0;
            }

            // Continuous, smooth synchronous baseline progress curve (no artificial 35% cap)
            let baseline = if sl.is_complete {
                1.0
            } else {
                let curve = (1.0 - (-elapsed / 3.0).exp()) * 0.96;
                curve.clamp(0.06, 0.96)
            };

            if sl.target_progress < baseline {
                sl.target_progress = baseline;
            }

            let dt = ctx.input(|i| i.stable_dt).min(0.1);
            let speed = if sl.target_progress >= 1.0 { 9.0 } else { 5.5 };
            sl.progress += (sl.target_progress - sl.progress) * (dt * speed).min(1.0);

            // Synchronize current shaders counter concurrently with progress
            if sl.total_shaders > 0 {
                let cur_count = (sl.total_shaders as f32 * sl.progress).round() as u32;
                sl.current_shaders = sl.current_shaders.max(cur_count).min(sl.total_shaders);
            }

            // Transition from Compiling to WaitingForGameWindow
            if sl.phase == ShaderLoadingPhase::Compiling && (sl.progress >= 0.995 || sl.is_complete) {
                sl.progress = 1.0;
                sl.current_shaders = sl.total_shaders;
                sl.phase = ShaderLoadingPhase::WaitingForGameWindow;
                sl.phase_changed_at = Some(std::time::Instant::now());
                if sl.completed_at.is_none() {
                    sl.completed_at = Some(std::time::Instant::now());
                }
                // Drop AlwaysOnTop to Normal so the SDL2 game window can focus in front
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));

                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
                let is_modded = self.installed_mods.iter().any(|m| m.is_enabled);
                self.discord_rpc.set_in_game(self.base_dir.clone(), now, self.config.target_fps, is_modded, &self.config.language);
            }

            // Oyun penceresi açıldığında oyun ekranını en öne getir ve launcher'ı Normal seviyeye çek
            // Shader ekranı hemen kapanmaz veya görev çubuğuna küçülmez! Oyunun arkasında 10 saniye açık kalır.
            if game_ready_now && sl.game_ready_at.is_none() {
                sl.game_ready_at = Some(std::time::Instant::now());
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                crate::win_input::bring_game_window_to_front(pid);
            }

            // Oyun açıldıktan 10 saniye sonra (veya algılanamazsa güvenlik zaman aşımı 25 saniye sonra)
            // shader ekranı arka planda kapanıp standart launcher arayüzüne döner (görev çubuğuna küçülmez!)
            let should_close = if let Some(ready_at) = sl.game_ready_at {
                ready_at.elapsed().as_secs_f32() >= 10.0
            } else {
                elapsed >= 25.0
            };

            if should_close {
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                #[cfg(target_os = "windows")]
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1024.0, 576.0)));
                }
                self.shader_loading = None;
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                // Güvenlik: Kullanıcı ESC tuşuna basarak ekranı erken kapatabilir
                ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                #[cfg(target_os = "windows")]
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1024.0, 576.0)));
                }
                self.shader_loading = None;
            }
        }

        // If shader loading screen is active, render it full screen and return!
        if self.shader_loading.is_some() {
            #[cfg(target_os = "windows")]
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            self.render_shader_loading_screen(ui, &ctx);
            return;
        }



        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(7, 8, 12);
        visuals.window_fill = egui::Color32::from_rgb(11, 12, 17);
        visuals.window_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 80));
        visuals.window_corner_radius = egui::CornerRadius::same(12);
        visuals.override_text_color = Some(egui::Color32::from_rgb(240, 240, 245));
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(13, 14, 21);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(22, 24, 34);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(45, 48, 66);
        visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgba_unmultiplied(220, 38, 38, 50);
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(239, 68, 68));
        visuals.widgets.hovered.expansion = 1.5;
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(220, 38, 38);
        visuals.widgets.active.expansion = 0.5;
        visuals.selection.bg_fill = egui::Color32::from_rgb(220, 38, 38);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(239, 68, 68));
        ctx.set_visuals(visuals);

        // Slide bar (scrollbar) kalınlaştırma ve görünürlük optimizasyonu
        ctx.style_mut_of(egui::Theme::Dark, |s| {
            s.spacing.scroll.bar_width = 12.0;
            s.spacing.scroll.floating_width = 8.0;
            s.spacing.scroll.handle_min_length = 32.0;
            s.spacing.scroll.dormant_handle_opacity = 0.60;
            s.spacing.scroll.active_handle_opacity = 0.85;
            s.spacing.scroll.interact_handle_opacity = 1.0;
            s.spacing.scroll.dormant_background_opacity = 0.25;
            s.spacing.scroll.active_background_opacity = 0.45;
            s.spacing.scroll.interact_background_opacity = 0.65;
            s.spacing.scroll.bar_inner_margin = 2.0;
            s.spacing.scroll.bar_outer_margin = 2.0;
            s.spacing.scroll.foreground_color = true;
        });

        // --- MAIN VIEW ROUTING: CONSOLE MAIN MENU OR SETTINGS SCREEN ---
        match self.view_mode {
            ViewMode::MainMenu => self.render_console_main_menu(ui, &ctx),
            ViewMode::Settings => self.render_settings_screen(ui, &ctx),
        }

        // Dim background if any modal dialog is currently active
        let modal_active = self.found_candidates_modal.is_some()
            || self.show_game_not_found_modal
            || self.show_first_launch_modal
            || self.active_crash_diagnosis.is_some()
            || self.show_log_modal
            || self.show_launcher_log_modal
            || self.error_modal.is_some()
            || self.update_modal.is_some()
            || self.listening_key_for.is_some()
            || self.show_release_notes_modal;

        if modal_active {
            ui.painter().rect_filled(
                ui.max_rect(),
                0.0,
                egui::Color32::from_rgba_unmultiplied(6, 7, 10, 195),
            );
        }

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
                                                self.status_message = Some((format!("{} seçildi ve kaydedildi!", selected_name), false));
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
                            egui::RichText::new("[ ! ]")
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

        // --- MODAL 4: GAME LOG MODAL ---
        if self.show_log_modal {
            egui::Window::new("gow3_log.txt (God of War III Engine Log)")
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(680.0, 440.0))
                .min_width(450.0)
                .min_height(250.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 44.0)
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(&self.log_content).monospace().size(11.0));
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(self.t("refresh_log")).clicked() {
                            let log_path = self.base_dir.join("gow3_log.txt");
                            self.log_content = std::fs::read_to_string(&log_path)
                                .unwrap_or_else(|_| "Log file is empty or not created yet.".to_string());
                        }
                        if ui.button(self.t("open_in_editor")).clicked() {
                            let log_path = self.base_dir.join("gow3_log.txt");
                            crate::logger::open_file_path(&log_path);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(self.t("close")).clicked() {
                                self.show_log_modal = false;
                            }
                        });
                    });
                });
        }

        // --- LAUNCHER LOG MODAL ---
        if self.show_launcher_log_modal {
            egui::Window::new("launcher_log.txt (God of War III Launcher Protokolü)")
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(720.0, 480.0))
                .min_width(480.0)
                .min_height(280.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(ui.available_height() - 44.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&self.launcher_log_content)
                                    .monospace()
                                    .size(11.5)
                                    .color(egui::Color32::from_rgb(220, 225, 235)),
                            );
                        });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(self.t("refresh_log")).clicked() {
                            self.launcher_log_content = crate::logger::read_launcher_log(&self.base_dir);
                        }
                        if ui.button(self.t("open_in_editor")).clicked() {
                            crate::logger::open_log_in_editor(&self.base_dir);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(self.t("close")).clicked() {
                                self.show_launcher_log_modal = false;
                            }
                        });
                    });
                });
        }

        // --- ERROR POP-UP MODAL ---
        if let Some(ref err_data) = self.error_modal.clone() {
            let mut close_modal = false;
            let mut open_log = false;
            let mut copy_error = false;
            let mut retry_alt_engine = false;

            egui::Window::new(self.t("err_modal_title"))
                .collapsible(false)
                .resizable(true)
                .default_size(egui::vec2(580.0, 360.0))
                .min_width(450.0)
                .min_height(280.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);

                    // Title Header with Icon
                    ui.horizontal(|ui| {
                        ui.heading(
                            egui::RichText::new(&err_data.title)
                                .color(egui::Color32::from_rgb(248, 113, 113))
                                .size(17.0)
                                .strong(),
                        );
                    });

                    // Summary Banner
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(38, 22, 25))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(180, 50, 50)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&err_data.summary)
                                    .color(egui::Color32::from_rgb(255, 210, 210))
                                    .size(13.5)
                                    .strong(),
                            );
                        });

                    // Technical Details Block
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("Teknik Detaylar / Hata Çıktısı:")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(180, 185, 200))
                            .strong(),
                    );

                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(18, 19, 24))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 55, 70)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(160.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut err_data.details.as_str())
                                            .font(egui::TextStyle::Monospace)
                                            .text_color(egui::Color32::from_rgb(235, 200, 150))
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                        });

                    // Log Hint if present
                    if let Some(ref hint) = err_data.log_hint {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(format!("💡 İpucu: {}", hint))
                                .size(11.5)
                                .color(egui::Color32::from_rgb(130, 200, 255))
                                .italics(),
                        );
                    }

                    ui.add_space(4.0);
                    ui.separator();

                    // Action Buttons Footer
                    ui.horizontal(|ui| {
                        // Open Log File button
                        let open_log_btn = egui::Button::new(
                            egui::RichText::new(self.t("open_log_file"))
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(35, 55, 85))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 110, 165)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(150.0, 32.0));

                        if ui.add(open_log_btn).clicked() {
                            open_log = true;
                        }

                        // Copy Error Button
                        let is_recently_copied = self.error_copied_feedback.map_or(false, |t| t.elapsed().as_secs() < 3);
                        let copy_text = if is_recently_copied {
                            self.t("copied_notice")
                        } else {
                            self.t("copy_error")
                        };

                        let copy_btn = egui::Button::new(
                            egui::RichText::new(copy_text)
                                .color(if is_recently_copied { egui::Color32::from_rgb(110, 255, 140) } else { egui::Color32::WHITE })
                                .strong(),
                        )
                        .fill(if is_recently_copied { egui::Color32::from_rgb(30, 60, 40) } else { egui::Color32::from_rgb(45, 48, 60) })
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 95, 120)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(140.0, 32.0));

                        if ui.add(copy_btn).clicked() {
                            copy_error = true;
                        }

                        // Alternative download button if engine is missing
                        if !self.has_engine {
                            let alt_dl_btn = egui::Button::new(
                                egui::RichText::new(self.t("retry_with_alt"))
                                    .color(egui::Color32::from_rgb(255, 230, 140))
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(55, 42, 18))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(190, 140, 45)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .min_size(egui::vec2(160.0, 32.0));

                            if ui.add(alt_dl_btn).clicked() {
                                retry_alt_engine = true;
                            }
                        }

                        // Close Button on Right
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_btn = egui::Button::new(
                                egui::RichText::new(self.t("close")).strong(),
                            )
                            .min_size(egui::vec2(90.0, 32.0))
                            .corner_radius(egui::CornerRadius::same(6));

                            if ui.add(close_btn).clicked() {
                                close_modal = true;
                            }
                        });
                    });
                });

            if retry_alt_engine {
                self.error_modal = None;
                self.start_engine_download(true);
            }

            if copy_error {
                let full_report = format!(
                    "God of War III Launcher Hata Raporu:\n{}\n\nÖzet:\n{}\n\nDetaylar:\n{}\n\nİpucu:\n{}",
                    err_data.title,
                    err_data.summary,
                    err_data.details,
                    err_data.log_hint.as_deref().unwrap_or("-")
                );
                ui.ctx().copy_text(full_report);
                self.error_copied_feedback = Some(std::time::Instant::now());
            }

            if open_log {
                crate::logger::open_log_in_editor(&self.base_dir);
                self.launcher_log_content = crate::logger::read_launcher_log(&self.base_dir);
                self.show_launcher_log_modal = true;
            }

            if close_modal {
                self.error_modal = None;
            }
        }

        // --- MODAL 5: UPDATE CHECKER & INTEGRATION MODAL ---
        if let Some(info) = self.update_modal.clone() {
            let title = if info.has_missing_files {
                "God of War III - Dosya Onarımı"
            } else if info.is_new {
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

                    if info.has_missing_files {
                        ui.heading(
                            egui::RichText::new("⚠️ Eksik / Bozuk Dosya Tespit Edildi")
                                .color(egui::Color32::from_rgb(255, 170, 60))
                                .size(16.0),
                        );
                    } else if info.is_new {
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
                            if info.is_new && !info.has_missing_files {
                                if let Some(ref tag) = info.version_tag {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new("🏷️ Yeni Sürüm:")
                                                .color(egui::Color32::from_rgb(180, 180, 190))
                                                .strong(),
                                        );
                                        ui.label(
                                            egui::RichText::new(tag)
                                                .color(egui::Color32::from_rgb(105, 225, 110))
                                                .strong(),
                                        );
                                    });
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("🏷️ Mevcut Sürüm:")
                                            .color(egui::Color32::from_rgb(180, 180, 190))
                                            .strong(),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{} (Güncel)", self.launcher_version))
                                            .color(egui::Color32::from_rgb(100, 200, 255))
                                            .strong(),
                                    );
                                });
                            }

                            if info.has_missing_files {
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(format!("Eksik Dosyalar: {}", info.missing_files.join(", ")))
                                        .color(egui::Color32::from_rgb(255, 170, 60))
                                        .size(12.0),
                                );
                            }

                            let short_sha = if info.commit_sha.len() > 7 {
                                &info.commit_sha[..7]
                            } else {
                                &info.commit_sha
                            };

                            if !short_sha.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("{} {}", self.t("latest_commit"), short_sha))
                                        .color(egui::Color32::from_rgb(255, 215, 100))
                                        .strong(),
                                );
                            }
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

                    if self.is_applying_update {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                egui::RichText::new(self.t("updating_in_progress"))
                                    .color(egui::Color32::from_rgb(255, 215, 100))
                                    .strong()
                                    .size(13.5),
                            );
                        });
                    } else {
                        ui.horizontal(|ui| {
                            let btn_text = if info.has_missing_files {
                                "Eksik Dosyaları İndir ve Onar"
                            } else if info.is_new {
                                self.t("btn_update_now")
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
                            .min_size(egui::vec2(160.0, 34.0));

                            if ui.add(apply_btn).clicked() {
                                self.is_applying_update = true;
                                let (tx, rx) = channel();
                                self.update_apply_rx = Some(rx);
                                let base_dir = self.base_dir.clone();
                                let info_clone = info.clone();
                                let mut config_clone = self.config.clone();
                                thread::spawn(move || {
                                    let res = apply_github_update(&base_dir, &info_clone, &mut config_clone);
                                    let _ = tx.send((res, config_clone));
                                });
                            }

                            if info.is_new {
                                let remind_btn = egui::Button::new(
                                    egui::RichText::new(self.t("remind_later"))
                                        .color(egui::Color32::from_rgb(220, 225, 235))
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(45, 48, 62))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60)))
                                .corner_radius(egui::CornerRadius::same(6))
                                .min_size(egui::vec2(130.0, 34.0));

                                if ui.add(remind_btn).clicked() {
                                    self.update_dismissed_this_session = true;
                                    self.update_modal = None;
                                }
                            }

                            if ui.button(self.t("view_on_github")).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/ScreamAngel/gow3_launcher"));
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if !info.is_new {
                                    if ui.button(self.t("close")).clicked() {
                                        self.update_modal = None;
                                    }
                                }
                            });
                        });
                    }
                });
        }

        // --- MODAL 6: KEY / MOUSE BINDING MODAL ---
        if let Some(target) = self.listening_key_for {
            let mut picked_key: Option<String> = None;
            let mut close_modal = false;

            let target_name = match target {
                KeyBindingTarget::Triangle => self.t("act_triangle"),
                KeyBindingTarget::Circle => self.t("act_circle"),
                KeyBindingTarget::Cross => self.t("act_cross"),
                KeyBindingTarget::Square => self.t("act_square"),
                KeyBindingTarget::L1 => self.t("act_l1"),
                KeyBindingTarget::R1 => self.t("act_r1"),
                KeyBindingTarget::L2 => self.t("act_l2"),
                KeyBindingTarget::R2 => self.t("act_r2"),
                KeyBindingTarget::L3 => self.t("act_l3"),
                KeyBindingTarget::R3 => self.t("act_r3"),
                KeyBindingTarget::Options => self.t("act_options"),
                KeyBindingTarget::TouchpadCenter => self.t("act_touchpad"),
                KeyBindingTarget::PadUp => self.t("act_dpad_up"),
                KeyBindingTarget::PadDown => self.t("act_dpad_down"),
                KeyBindingTarget::PadLeft => self.t("act_dpad_left"),
                KeyBindingTarget::PadRight => self.t("act_dpad_right"),
                KeyBindingTarget::AxisLeftYMinus => self.t("act_move_up"),
                KeyBindingTarget::AxisLeftYPlus => self.t("act_move_down"),
                KeyBindingTarget::AxisLeftXMinus => self.t("act_move_left"),
                KeyBindingTarget::AxisLeftXPlus => self.t("act_move_right"),
                KeyBindingTarget::AxisRightYMinus => self.t("act_cam_up"),
                KeyBindingTarget::AxisRightYPlus => self.t("act_cam_down"),
                KeyBindingTarget::AxisRightXMinus => self.t("act_cam_left"),
                KeyBindingTarget::AxisRightXPlus => self.t("act_cam_right"),
            };

            egui::Window::new(self.t("key_bind_modal_title"))
                .collapsible(false)
                .resizable(false)
                .default_width(540.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                    ui.horizontal(|ui| {
                        paint_ps_badge(ui, target);
                        ui.label(
                            egui::RichText::new(target_name)
                                .size(16.0)
                                .strong()
                                .color(egui::Color32::from_rgb(255, 215, 100)),
                        );
                    });
                    ui.label(
                        egui::RichText::new(self.t("key_bind_modal_desc"))
                            .size(12.0)
                            .color(egui::Color32::from_rgb(200, 205, 220)),
                    );
                    ui.separator();

                    // Section 1: Mouse Buttons
                    ui.label(
                        egui::RichText::new(self.t("sec_mouse_bindings"))
                            .strong()
                            .color(egui::Color32::from_rgb(140, 210, 255))
                            .size(13.0),
                    );

                    ui.horizontal_wrapped(|ui| {
                        let mouse_options = [
                            (self.t("mouse_left_btn"), "leftbutton"),
                            (self.t("mouse_right_btn"), "rightbutton"),
                            (self.t("mouse_middle_btn"), "middlebutton"),
                            (self.t("mouse_side1_btn"), "sidebuttonback"),
                            (self.t("mouse_side2_btn"), "sidebuttonforward"),
                            (self.t("mouse_wheel_up"), "mousewheelup"),
                            (self.t("mouse_wheel_down"), "mousewheeldown"),
                        ];
                        for (label, key_code) in mouse_options {
                            let btn = egui::Button::new(
                                egui::RichText::new(label).size(11.5).strong().color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(32, 45, 65))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 110, 160)))
                            .corner_radius(egui::CornerRadius::same(5));
                            if ui.add(btn).clicked() {
                                picked_key = Some(key_code.to_string());
                            }
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();

                    // Section 2: Numpad Grid
                    ui.label(
                        egui::RichText::new(self.t("sec_numpad_bindings"))
                            .strong()
                            .color(egui::Color32::from_rgb(255, 180, 100))
                            .size(13.0),
                    );

                    let numpad_rows = [
                        [("7", "kp7"), ("8", "kp8"), ("9", "kp9"), ("/", "kpslash")],
                        [("4", "kp4"), ("5", "kp5"), ("6", "kp6"), ("*", "kpasterisk")],
                        [("1", "kp1"), ("2", "kp2"), ("3", "kp3"), ("-", "kpminus")],
                        [("0", "kp0"), (".", "kpperiod"), ("Enter", "kpenter"), ("+", "kpplus")],
                    ];

                    for row in numpad_rows {
                        ui.horizontal(|ui| {
                            for (label, key_code) in row {
                                let btn = egui::Button::new(
                                    egui::RichText::new(format!("KP {}", label)).size(11.5).strong().color(egui::Color32::WHITE),
                                )
                                .min_size(egui::vec2(72.0, 26.0))
                                .fill(egui::Color32::from_rgb(45, 38, 55))
                                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(130, 90, 160)))
                                .corner_radius(egui::CornerRadius::same(5));
                                if ui.add(btn).clicked() {
                                    picked_key = Some(key_code.to_string());
                                }
                            }
                        });
                    }

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(self.t("sec_digits_bindings"))
                            .strong()
                            .color(egui::Color32::from_rgb(180, 210, 230))
                            .size(12.0),
                    );
                    ui.horizontal(|ui| {
                        for num in ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"] {
                            let btn = egui::Button::new(
                                egui::RichText::new(num).size(12.0).strong().color(egui::Color32::WHITE),
                            )
                            .min_size(egui::vec2(28.0, 24.0))
                            .fill(egui::Color32::from_rgb(30, 36, 48))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 85, 110)))
                            .corner_radius(egui::CornerRadius::same(5));
                            if ui.add(btn).clicked() {
                                picked_key = Some(num.to_string());
                            }
                        }
                    });

                    ui.add_space(6.0);
                    ui.separator();

                    // Cancel button
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let cancel_btn = egui::Button::new(
                            egui::RichText::new(format!("❌ {}", self.t("cancel_btn"))).size(12.0).strong(),
                        )
                        .min_size(egui::vec2(120.0, 30.0))
                        .fill(egui::Color32::from_rgb(70, 25, 25))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(170, 60, 60)))
                        .corner_radius(egui::CornerRadius::same(6));
                        if ui.add(cancel_btn).clicked() {
                            close_modal = true;
                        }
                    });
                });

            if let Some(new_k) = picked_key {
                apply_binding_to_config(&mut self.config, target, new_k);
                self.listening_key_for = None;
            } else if close_modal {
                self.listening_key_for = None;
            }
        }

        // --- MODAL: RELEASE NOTES (GÜNCELLEME NOTLARI) ---
        if self.show_release_notes_modal {
            egui::Window::new(self.t("release_notes_title"))
                .collapsible(false)
                .resizable(true)
                .default_width(680.0)
                .default_height(500.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .show(&ctx, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                    egui::ScrollArea::vertical().max_height(450.0).show(ui, |ui| {
                        // Current Version Banner: v2.0
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.heading(
                                    egui::RichText::new("🚀 God of War III Launcher v2.0 (Güncel Sürüm)")
                                        .color(egui::Color32::from_rgb(255, 215, 100))
                                        .strong()
                                        .size(15.0),
                                );
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(
                                        egui::RichText::new("En Son / Aktif")
                                            .color(egui::Color32::from_rgb(102, 225, 106))
                                            .strong()
                                            .size(11.5),
                                    );
                                });
                            });
                            ui.separator();
                            ui.label(
                                egui::RichText::new(self.t("release_notes_content"))
                                    .size(12.5)
                                    .color(egui::Color32::from_rgb(235, 240, 250)),
                            );
                        });

                        ui.add_space(8.0);

                        // Real-time GitHub Releases Section
                        if self.is_fetching_release_notes {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label(egui::RichText::new("GitHub üzerinden geçmiş sürüm notları yükleniyor...").color(egui::Color32::LIGHT_GRAY));
                            });
                        } else if let Some(ref notes) = self.github_release_notes {
                            if !notes.is_empty() {
                                ui.label(
                                    egui::RichText::new("📦 Önceki GitHub Sürümleri")
                                        .size(14.0)
                                        .color(egui::Color32::from_rgb(140, 200, 255))
                                        .strong(),
                                );
                                ui.separator();
                                for note in notes {
                                    if note.tag_name != "v2.0" {
                                        ui.group(|ui| {
                                            ui.horizontal(|ui| {
                                                ui.label(
                                                    egui::RichText::new(format!("🏷️ {}", note.tag_name))
                                                        .strong()
                                                        .size(13.5)
                                                        .color(egui::Color32::from_rgb(255, 180, 100)),
                                                );
                                                if !note.title.is_empty() && note.title != note.tag_name {
                                                    ui.label(egui::RichText::new(&note.title).strong().color(egui::Color32::WHITE));
                                                }
                                                if !note.published_at.is_empty() {
                                                    let date_str = note.published_at.split('T').next().unwrap_or(&note.published_at);
                                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                        ui.label(egui::RichText::new(date_str).color(egui::Color32::GRAY).size(11.0));
                                                    });
                                                }
                                            });
                                            if !note.body.is_empty() {
                                                ui.separator();
                                                ui.label(
                                                    egui::RichText::new(&note.body)
                                                        .size(12.0)
                                                        .color(egui::Color32::from_rgb(215, 220, 230)),
                                                );
                                            }
                                        });
                                        ui.add_space(4.0);
                                    }
                                }
                            }
                        }
                    });
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("🔄 Yenile").clicked() {
                            self.github_release_notes = None;
                            self.start_fetch_release_notes();
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let close_btn = egui::Button::new(
                                egui::RichText::new(self.t("close")).strong().size(12.5),
                            )
                            .min_size(egui::vec2(90.0, 30.0))
                            .corner_radius(egui::CornerRadius::same(6));

                            if ui.add(close_btn).clicked() {
                                self.show_release_notes_modal = false;
                            }
                        });
                    });
                });
        }
    }
}

impl Drop for LauncherApp {
    fn drop(&mut self) {
        self.discord_rpc.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_key_display() {
        assert!(format_key_display("leftbutton").contains("Sol Tık"));
        assert!(format_key_display("rightbutton").contains("Sağ Tık"));
        assert!(format_key_display("middlebutton").contains("Orta Tık"));
        assert!(format_key_display("sidebuttonback").contains("Yan 1"));
        assert!(format_key_display("sidebuttonforward").contains("Yan 2"));
        assert_eq!(format_key_display("kp8"), "Numpad 8");
        assert_eq!(format_key_display("kp2"), "Numpad 2");
        assert_eq!(format_key_display("kpplus"), "Numpad +");
    }

    #[test]
    fn test_apply_binding_to_config() {
        let mut cfg = LauncherConfig::default();
        apply_binding_to_config(&mut cfg, KeyBindingTarget::Triangle, "leftbutton".to_string());
        assert_eq!(cfg.keyboard_bindings.triangle, "leftbutton");
        apply_binding_to_config(&mut cfg, KeyBindingTarget::Square, "rightbutton".to_string());
        assert_eq!(cfg.keyboard_bindings.square, "rightbutton");
        apply_binding_to_config(&mut cfg, KeyBindingTarget::Cross, "kp2".to_string());
        assert_eq!(cfg.keyboard_bindings.cross, "kp2");
    }
}
