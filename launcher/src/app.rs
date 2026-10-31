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
    Graphics,
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

fn render_nav_tab(ui: &mut egui::Ui, text: &str, is_active: bool) -> bool {
    let id = ui.make_persistent_id(text);
    let font_id = egui::FontId::proportional(12.5);
    let (base_bg, text_col, base_stroke) = if is_active {
        (
            egui::Color32::from_rgb(220, 38, 38),
            egui::Color32::WHITE,
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 150, 150, 200)),
        )
    } else {
        (
            egui::Color32::from_rgba_unmultiplied(22, 24, 34, 180),
            egui::Color32::from_rgb(156, 163, 175),
            egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20)),
        )
    };

    let galley = ui.painter().layout_no_wrap(text.to_string(), font_id, text_col);
    let padding = egui::vec2(24.0, 10.0);
    let desired_size = egui::vec2(galley.rect.width() + padding.x, galley.rect.height() + padding.y);

    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());
    let is_hovered = response.hovered();
    let hover_anim = ui.ctx().animate_bool(id, is_hovered);

    if is_hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    let painter = ui.painter();
    let current_bg = if is_active {
        if hover_anim > 0.01 {
            egui::Color32::from_rgb(235, 45, 45)
        } else {
            base_bg
        }
    } else if hover_anim > 0.01 {
        egui::Color32::from_rgba_unmultiplied(42, 45, 62, 220)
    } else {
        base_bg
    };

    let current_stroke = if is_active {
        base_stroke
    } else if hover_anim > 0.01 {
        egui::Stroke::new(1.2, egui::Color32::from_rgb(239, 68, 68))
    } else {
        base_stroke
    };

    let current_text_col = if is_active || hover_anim > 0.05 {
        egui::Color32::WHITE
    } else {
        text_col
    };

    let draw_rect = if hover_anim > 0.01 {
        rect.expand(1.0 * hover_anim)
    } else {
        rect
    };

    painter.rect_filled(draw_rect, 6.0, current_bg);
    painter.rect_stroke(draw_rect, 6.0, current_stroke, egui::StrokeKind::Inside);

    let text_pos = egui::pos2(
        draw_rect.left() + (draw_rect.width() - galley.rect.width()) / 2.0,
        draw_rect.top() + (draw_rect.height() - galley.rect.height()) / 2.0,
    );
    painter.galley(text_pos, galley, current_text_col);

    response.clicked()
}

fn render_section_tag(ui: &mut egui::Ui, tag: &str) {
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(tag)
            .size(11.0)
            .color(egui::Color32::from_rgb(239, 68, 68))
            .strong(),
    );
    ui.add_space(1.0);
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
                        ui.label(egui::RichText::new(tag).color(tag_color).size(10.0).strong());
                    });
                ui.label(egui::RichText::new(title).color(egui::Color32::from_rgb(165, 170, 185)).size(11.0));
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(value).color(egui::Color32::WHITE).strong().size(13.0));
            if let Some((b_text, b_color)) = badge {
                ui.add_space(3.0);
                egui::Frame::new()
                    .fill(egui::Color32::from_rgba_unmultiplied(b_color.r(), b_color.g(), b_color.b(), 35))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(b_color.r(), b_color.g(), b_color.b(), 110)))
                    .corner_radius(egui::CornerRadius::same(4))
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(b_text).color(b_color).size(10.0).strong());
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
        ui.add_space(2.0);
        ui.label(egui::RichText::new(label).size(12.5).color(egui::Color32::from_rgb(230, 230, 235)));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let is_listening = listening_for == Some(target);
            let btn_text = if is_listening {
                "[ Tuşa basın... / ESC ]".to_string()
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
                egui::RichText::new(btn_text).strong().size(12.0).color(text_color),
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
    silent_update_check: bool,
    has_new_update: bool,
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
    icon_texture: Option<egui::TextureHandle>,
    win_texture: Option<egui::TextureHandle>,
    tux_texture: Option<egui::TextureHandle>,

    // Dynamic Launcher Version
    launcher_version: String,
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
            silent_update_check: true,
            has_new_update: false,
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
            icon_texture: None,
            win_texture: None,
            tux_texture: None,
            launcher_version,
        };

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

        let current_commit = self.config.last_git_commit.clone();
        let current_ver = self.launcher_version.clone();
        thread::spawn(move || {
            let res = check_github_update(&current_commit, &current_ver);
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

        let current_commit = self.config.last_git_commit.clone();
        let current_ver = self.launcher_version.clone();
        thread::spawn(move || {
            let res = check_github_update(&current_commit, &current_ver);
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
                        if info.is_new {
                            self.has_new_update = true;
                        }
                        if !self.silent_update_check {
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
                        self.status_message = Some((format!("Hata: {}", e), true));
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
                egui::Color32::from_white_alpha(150),
            );
            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(7, 8, 12, 175),
            );
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

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 12.0);

            // --- TOP NAVBAR ---
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(10, 11, 16, 230))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 50)))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Left: Omega icon + Brand name
                        if let Some(ref icon) = self.icon_texture {
                            ui.add(egui::Image::new(icon).max_height(24.0));
                            ui.add_space(6.0);
                        }
                        ui.label(
                            egui::RichText::new("GOW III")
                                .size(14.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("LAUNCHER")
                                .size(14.0)
                                .color(egui::Color32::from_rgb(239, 68, 68))
                                .strong(),
                        );

                        ui.add_space(18.0);

                        // Center: Navigation Tabs (Embedded right in the top navbar!)
                        let game_active = self.active_tab == ActiveTab::Game;
                        let graphics_active = self.active_tab == ActiveTab::Graphics;
                        let config_active = self.active_tab == ActiveTab::Config;

                        let game_tab_text = format!("⚔ {}", self.t("tab_game"));
                        let graphics_tab_text = format!("🎨 {}", self.t("tab_graphics"));
                        let config_tab_text = format!("⚙ {}", self.t("tab_config_controls"));

                        if render_nav_tab(ui, &game_tab_text, game_active) {
                            self.active_tab = ActiveTab::Game;
                        }
                        ui.add_space(6.0);
                        if render_nav_tab(ui, &graphics_tab_text, graphics_active) {
                            self.active_tab = ActiveTab::Graphics;
                        }
                        ui.add_space(6.0);
                        if render_nav_tab(ui, &config_tab_text, config_active) {
                            self.active_tab = ActiveTab::Config;
                        }

                        // Right: Language & Update actions
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mut selected_lang = self.current_lang;
                            let lang_combo = egui::ComboBox::from_id_salt("lang_selector")
                                .selected_text(format!("🌐 {}", selected_lang.display_name()))
                                .show_ui(ui, |ui| {
                                    for lang in Language::all() {
                                        if ui.selectable_value(&mut selected_lang, *lang, lang.display_name()).clicked() {
                                            self.current_lang = *lang;
                                            self.config.language = lang.code().to_string();
                                            let _ = self.config.save(&self.base_dir);
                                        }
                                    }
                                });
                            if lang_combo.response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            ui.add_space(8.0);

                            if self.is_checking_update {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(
                                        egui::RichText::new(self.t("checking_updates"))
                                            .color(egui::Color32::from_rgb(180, 180, 190))
                                            .size(12.0),
                                    )
                                    .fill(egui::Color32::from_rgb(24, 26, 36))
                                    .corner_radius(egui::CornerRadius::same(6)),
                                );
                            } else {
                                let (update_text, is_pulse) = if self.has_new_update {
                                    (format!("✨ {} (Yeni!)", self.t("check_updates")), true)
                                } else {
                                    (format!("🔄 {}", self.t("check_updates")), false)
                                };

                                let update_btn = egui::Button::new(
                                    egui::RichText::new(update_text)
                                        .color(if self.has_new_update { egui::Color32::from_rgb(255, 235, 120) } else { egui::Color32::from_rgb(220, 220, 230) })
                                        .size(12.0)
                                        .strong(),
                                )
                                .fill(if self.has_new_update { egui::Color32::from_rgb(52, 24, 28) } else { egui::Color32::from_rgb(28, 30, 42) })
                                .stroke(egui::Stroke::new(
                                    1.0,
                                    if self.has_new_update { egui::Color32::from_rgb(239, 68, 68) } else { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25) }
                                ))
                                .corner_radius(egui::CornerRadius::same(6));

                                let res = ui.add(update_btn);
                                if is_pulse {
                                    let time = ui.ctx().input(|i| i.time);
                                    let pulse = ((time * 3.6).sin() * 0.5 + 0.5) as f32;
                                    ui.ctx().request_repaint();
                                    ui.painter().rect_filled(
                                        res.rect.expand(2.0 + 2.5 * pulse),
                                        8.0,
                                        egui::Color32::from_rgba_unmultiplied(239, 68, 68, (60.0 + 75.0 * pulse) as u8),
                                    );
                                }
                                if res.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                if res.clicked() {
                                    self.start_update_check();
                                }
                            }
                        });
                    });
                });

            // --- HERO SECTION (MATCHING WEBSITE HERO) ---
            ui.vertical_centered(|ui| {
                ui.add_space(2.0);
                render_hero_pill_badge(ui, None, self.t("hero_badge"), true, false);
                ui.add_space(6.0);

                if let Some(ref logo) = self.logo_texture {
                    ui.add(egui::Image::new(logo).max_height(82.0));
                }

                ui.heading(
                    egui::RichText::new("GOD OF WAR III PC LAUNCHER")
                        .size(23.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );

                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(self.t("hero_desc"))
                        .size(11.5)
                        .color(egui::Color32::from_rgb(148, 163, 184)),
                );

                ui.add_space(6.0);

                // Feature Pill Badges Row (Centered symmetrically)
                let pill_items: [(Option<&egui::TextureHandle>, String, bool, bool); 5] = [
                    (None, format!("🏷️ {}", self.launcher_version), true, self.has_new_update),
                    (self.win_texture.as_ref(), self.t("badge_windows").to_string(), false, false),
                    (self.tux_texture.as_ref(), self.t("badge_linux").to_string(), false, false),
                    (None, format!("🎮 {}", self.t("badge_steam_deck")), false, false),
                    (None, format!("⚡ {}", self.t("badge_free_oss")), false, false),
                ];

                let font_id = egui::FontId::proportional(11.0);
                let item_spacing = 6.0;
                let mut total_row_w = 0.0;
                for (ico, t, _, _) in &pill_items {
                    let galley = ui.painter().layout_no_wrap(t.clone(), font_id.clone(), egui::Color32::WHITE);
                    let icon_w = if ico.is_some() { 13.0 + 6.0 } else { 0.0 };
                    total_row_w += icon_w + galley.rect.width() + 22.0 + item_spacing;
                }
                total_row_w -= item_spacing;

                let avail = ui.available_width();
                let left_margin = (((avail - total_row_w) / 2.0) - 22.0).max(0.0);

                ui.horizontal(|ui| {
                    if left_margin > 0.0 {
                        ui.add_space(left_margin);
                    }
                    for (ico, text, is_crimson, is_pulse) in &pill_items {
                        render_hero_pill_badge(ui, *ico, text, *is_crimson, *is_pulse);
                        ui.add_space(item_spacing);
                    }
                });
            });

            // Status message
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

                egui::Frame::new()
                    .fill(bg)
                    .stroke(egui::Stroke::new(1.0, border))
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let icon = if is_err { "⚠️" } else { "✅" };
                            ui.label(egui::RichText::new(icon).size(13.0));
                            ui.label(egui::RichText::new(msg).color(text_col).strong().size(12.0));
                        });
                    });
            }

            let card_frame = egui::Frame::group(ui.style())
                .fill(egui::Color32::from_rgba_unmultiplied(13, 14, 21, 235))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(220, 38, 38, 65)))
                .inner_margin(egui::Margin::symmetric(14, 12))
                .corner_radius(egui::CornerRadius::same(12));

            // ==========================================
            // TAB 1: GAME & PERFORMANCE
            // ==========================================
            if self.active_tab == ActiveTab::Game {
                // 1. SYSTEM HARDWARE INFO CARD
                render_section_tag(ui, self.t("tag_hardware"));
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

                    // CPU Rating & Badge
                    let cpu_grade = crate::system::evaluate_cpu(&hw.cpu_name, hw.cpu_cores, hw.cpu_threads);
                    let cpu_badge = cpu_grade.badge_text(&self.config.language);
                    let (cpu_r, cpu_g, cpu_b) = cpu_grade.rgb();
                    let cpu_color = egui::Color32::from_rgb(cpu_r, cpu_g, cpu_b);

                    // GPU Rating & Badge
                    let (gpu_color, gpu_badge) = if hw.vram_mb >= 8192 {
                        (
                            egui::Color32::from_rgb(0, 230, 118),
                            match self.config.language.as_str() {
                                "tr" => "• Üst Seviye (Yüksek FPS)",
                                "de" => "• Spitzenklasse (Hohe FPS)",
                                "es" => "• Gama Alta (Altos FPS)",
                                _ => "• High-End (High FPS)",
                            }.to_string(),
                        )
                    } else if hw.vram_mb >= 6144 {
                        (
                            egui::Color32::from_rgb(102, 225, 106),
                            match self.config.language.as_str() {
                                "tr" => "• Güçlü (Akıcı Deneyim)",
                                "de" => "• Stark (Flüssiges Erlebnis)",
                                "es" => "• Potente (Fluido)",
                                _ => "• Strong (Smooth Experience)",
                            }.to_string(),
                        )
                    } else if hw.vram_mb >= 4096 {
                        (
                            egui::Color32::from_rgb(255, 213, 79),
                            match self.config.language.as_str() {
                                "tr" => "• Orta Seviye (Standart)",
                                "de" => "• Mittelstufe (Standard)",
                                "es" => "• Nivel Medio (Estándar)",
                                _ => "• Mid-Range (Standard)",
                            }.to_string(),
                        )
                    } else {
                        (
                            egui::Color32::from_rgb(239, 83, 80),
                            match self.config.language.as_str() {
                                "tr" => "• Giriş Seviye / Düşük VRAM",
                                "de" => "• Einstiegsklasse / Wenig VRAM",
                                "es" => "• Gama Baja / Poca VRAM",
                                _ => "• Entry-Level / Low VRAM",
                            }.to_string(),
                        )
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
                        (
                            egui::Color32::from_rgb(102, 225, 106),
                            match self.config.language.as_str() {
                                "tr" => "• Yüksek Tazeleme (Ultra Akıcı)",
                                "de" => "• Hohe Bildrate (Ultraflüssig)",
                                "es" => "• Alta Frecuencia (Ultra Fluido)",
                                _ => "• High Refresh (Ultra Smooth)",
                            },
                        )
                    } else {
                        (
                            egui::Color32::from_rgb(130, 215, 140),
                            match self.config.language.as_str() {
                                "tr" => "• Akıcı Ekran (Standart 60Hz)",
                                "de" => "• Flüssig (Standard 60Hz)",
                                "es" => "• Fluido (Estándar 60Hz)",
                                _ => "• Smooth Display (Standard 60Hz)",
                            },
                        )
                    };

                    // Row 1: CPU & GPU (Compute Units)
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

                    ui.add_space(6.0);

                    // Row 2: VRAM, RAM & Display Refresh Rate
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
                            let ram_val = format!("{} MB (~{} GB)", hw.ram_mb, (hw.ram_mb as f32 / 1024.0).round());
                            render_stat_tile(
                                ui,
                                "RAM",
                                egui::Color32::from_rgb(100, 200, 255),
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
                                egui::Color32::from_rgb(200, 140, 255),
                                self.t("refresh_rate"),
                                &hz_val,
                                Some((hz_badge, hz_color)),
                            );
                        });
                    });

                    // CPU Bottleneck Warning Banner:
                    if cpu_grade == crate::system::CpuGrade::Weak || (cpu_grade == crate::system::CpuGrade::Medium && hw.vram_mb >= 8192) {
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgba_unmultiplied(220, 100, 30, 25))
                            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 140, 50, 100)))
                            .corner_radius(egui::CornerRadius::same(6))
                            .inner_margin(egui::Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("⚠️").size(15.0));
                                    let warn_text = if cpu_grade == crate::system::CpuGrade::Weak {
                                        match self.config.language.as_str() {
                                            "tr" => format!(
                                                "İşlemci Darboğaz Riski: Ekran kartınız iyi olsa bile, mevcut işlemciniz ({} çekirdek / {} izlek) emülasyon için sınırda. Takılma ve FPS düşüşlerini önlemek için Grafik sekmesinden '30 FPS' veya '40 FPS' kilidi ve 'Asenkron Geri Okuma' kullanmanız önerilir.",
                                                hw.cpu_cores, hw.cpu_threads
                                            ),
                                            "de" => format!(
                                                "CPU-Bottleneck-Risiko: Auch mit einer guten GPU ist deine CPU ({} Kerne / {} Threads) am Limit. Um Ruckler zu vermeiden, werden '30/40 FPS Lock' und 'Asynchrone Rücklesung' empfohlen.",
                                                hw.cpu_cores, hw.cpu_threads
                                            ),
                                            "es" => format!(
                                                "Riesgo de Cuello de Botella CPU: Aunque tu GPU sea buena, tu CPU ({} núcleos / {} hilos) está al límite. Se recomienda bloquear a 30/40 FPS y activar Lectura Asíncrona para evitar tirones.",
                                                hw.cpu_cores, hw.cpu_threads
                                            ),
                                            _ => format!(
                                                "CPU Bottleneck Risk: Even with a capable GPU, your CPU ({} cores / {} threads) is at the threshold for heavy emulation. Locking to 30 or 40 FPS with 'Async Readbacks' enabled is recommended to prevent stutter.",
                                                hw.cpu_cores, hw.cpu_threads
                                            ),
                                        }
                                    } else {
                                        match self.config.language.as_str() {
                                            "tr" => format!(
                                                "Dengeli Performans Notu: Çok güçlü bir ekran kartınız var ({} GB VRAM); işlemciniz ({} çekirdek / {} izlek) 40-60 FPS için uygundur, 120+ FPS hedeflerde işlemci sınırına takılabilir.",
                                                hw.vram_mb / 1024, hw.cpu_cores, hw.cpu_threads
                                            ),
                                            "de" => format!(
                                                "Leistungs-Hinweis: Du hast eine sehr starke GPU ({} GB VRAM); deine CPU ({} Kerne / {} Threads) ist optimal für 40-60 FPS, könnte aber bei 120+ FPS limitieren.",
                                                hw.vram_mb / 1024, hw.cpu_cores, hw.cpu_threads
                                            ),
                                            "es" => format!(
                                                "Nota de Rendimiento: Tienes una GPU muy potente ({} GB VRAM); tu CPU ({} núcleos / {} hilos) es ideal para 40-60 FPS, pero podría limitar objetivos de 120+ FPS.",
                                                hw.vram_mb / 1024, hw.cpu_cores, hw.cpu_threads
                                            ),
                                            _ => format!(
                                                "Performance Note: You have a high-end GPU ({} GB VRAM); your CPU ({} cores / {} threads) is well-suited for 40-60 FPS, but may limit 120+ FPS targets.",
                                                hw.vram_mb / 1024, hw.cpu_cores, hw.cpu_threads
                                            ),
                                        }
                                    };
                                    ui.label(egui::RichText::new(warn_text).color(egui::Color32::from_rgb(255, 195, 90)).size(11.5));
                                });
                            });
                    }

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
                render_section_tag(ui, self.t("tag_game_location"));
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
                                    ui.label(egui::RichText::new("[ ! ]").size(18.0));
                                    ui.label(
                                        egui::RichText::new(self.t("scan_not_found_inline"))
                                            .color(egui::Color32::from_rgb(255, 140, 140))
                                            .strong(),
                                    );
                                });
                            });
                    }
                });

                // 3. QUICK TOOLS & LOGS
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let log_btn = egui::Button::new(
                        egui::RichText::new(format!("📋 {}", self.t("view_log")))
                            .color(egui::Color32::from_rgb(200, 205, 220)),
                    )
                    .fill(egui::Color32::from_rgb(28, 30, 42))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .min_size(egui::vec2(140.0, 32.0));

                    let res = ui.add(log_btn);
                    if res.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if res.clicked() {
                        let log_path = self.base_dir.join("gow3_log.txt");
                        self.log_content = std::fs::read_to_string(&log_path)
                            .unwrap_or_else(|_| "Log file is empty or not created yet.".to_string());
                        self.show_log_modal = true;
                    }

                    let kill_btn = egui::Button::new(
                        egui::RichText::new(format!("🛑 {}", self.t("kill_leftover_btn")))
                            .color(egui::Color32::from_rgb(255, 140, 140))
                            .size(12.0),
                    )
                    .fill(egui::Color32::from_rgb(45, 20, 22))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(239, 68, 68, 80)))
                    .corner_radius(egui::CornerRadius::same(6))
                    .min_size(egui::vec2(140.0, 32.0));

                    let res = ui.add(kill_btn).on_hover_text(self.t("kill_leftover_tooltip"));
                    if res.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if res.clicked() {
                        self.kill_leftover_processes();
                    }
                });

                // 4. PLAY GAME ACTION AREA
                ui.add_space(12.0);
                ui.vertical_centered(|ui| {
                    if self.is_game_running {
                        ui.horizontal(|ui| {
                            let total_w = 210.0 + 12.0 + 190.0;
                            let available_w = ui.available_width();
                            if available_w > total_w {
                                ui.add_space((available_w - total_w) / 2.0);
                            }

                            let time = ctx.input(|i| i.time);
                            let pulse = ((time * 3.0).sin() * 0.5 + 0.5) as f32;
                            ctx.request_repaint();

                            let emerald_r = (25.0 + 15.0 * pulse) as u8;
                            let emerald_g = (65.0 + 30.0 * pulse) as u8;
                            let emerald_b = (40.0 + 15.0 * pulse) as u8;

                            let running_btn = egui::Button::new(
                                egui::RichText::new(self.t("play_running"))
                                    .size(17.0)
                                    .color(egui::Color32::from_rgb(140, 255, 155))
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(emerald_r, emerald_g, emerald_b))
                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(100, 255, 120, (120.0 + 100.0 * pulse) as u8)))
                            .corner_radius(egui::CornerRadius::same(10))
                            .min_size(egui::vec2(220.0, 56.0));

                            ui.add_enabled(false, running_btn);

                            let force_quit_btn = egui::Button::new(
                                egui::RichText::new(self.t("force_quit_btn"))
                                    .size(17.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(220, 42, 42))
                            .stroke(egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 120, 120, 180)))
                            .corner_radius(egui::CornerRadius::same(10))
                            .min_size(egui::vec2(190.0, 56.0));

                            let res = ui.add(force_quit_btn).on_hover_text(self.t("force_quit_tooltip"));
                            if res.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if res.clicked() {
                                self.force_quit_game();
                            }
                        });
                    } else if self.is_game_valid {
                        let time = ctx.input(|i| i.time);
                        let pulse = ((time * 2.6).sin() * 0.5 + 0.5) as f32; // 0.0 to 1.0
                        ctx.request_repaint();

                        let r = (215.0 + 35.0 * pulse) as u8;
                        let g = (35.0 + 25.0 * pulse) as u8;
                        let b = 35u8;
                        let btn_color = egui::Color32::from_rgb(r, g, b);
                        let border_color = egui::Color32::from_rgba_unmultiplied(
                            255,
                            (110.0 + 90.0 * pulse) as u8,
                            (70.0 + 50.0 * pulse) as u8,
                            (150.0 + 105.0 * pulse) as u8,
                        );

                        let play_btn = egui::Button::new(
                            egui::RichText::new(format!("▶   {}", self.t("play_ready")))
                                .size(21.0)
                                .color(egui::Color32::WHITE)
                                .strong(),
                        )
                        .fill(btn_color)
                        .stroke(egui::Stroke::new(1.8 + pulse * 1.2, border_color))
                        .corner_radius(egui::CornerRadius::same(12))
                        .min_size(egui::vec2(380.0, 58.0));

                        let res = ui.add(play_btn);
                        if res.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if res.clicked() {
                            self.launch_game();
                        }
                        ui.add_space(3.0);
                        ui.label(
                            egui::RichText::new(self.t("play_subtitle"))
                                .color(egui::Color32::from_rgb(148, 163, 184))
                                .size(11.5),
                        );
                    } else {
                        let play_btn = egui::Button::new(
                            egui::RichText::new(format!("▶   {}", self.t("play_ready")))
                                .size(21.0)
                                .color(egui::Color32::from_rgb(140, 140, 140))
                                .strong(),
                        )
                        .fill(egui::Color32::from_rgb(45, 46, 56))
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
            } else if self.active_tab == ActiveTab::Graphics {
                // ==========================================
                // TAB 2: GRAPHICS SETTINGS
                // ==========================================
                render_section_tag(ui, self.t("graphics_header"));
                card_frame.show(ui, |ui| {
                    ui.heading(
                        egui::RichText::new(self.t("graphics_header"))
                            .size(17.0)
                            .color(egui::Color32::from_rgb(150, 205, 255)),
                    );
                    ui.label(
                        egui::RichText::new(self.t("graphics_subtitle"))
                            .size(11.5)
                            .color(egui::Color32::from_rgb(175, 185, 205)),
                    );
                    ui.separator();

                    // --- 1. HAZIR GRAFİK PROFİLİ (GRAPHICS PRESET) ---
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.t("graphic_preset")).strong().color(egui::Color32::WHITE));
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
                    ui.label(egui::RichText::new(self.t("preset_desc")).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // --- 2. ÇÖZÜNÜRLÜK (RENDER RESOLUTION) ---
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.t("render_resolution")).strong().color(egui::Color32::WHITE));
                        let res_text = match self.config.render_resolution.as_str() {
                            "480p" => "480p (854x480 - Ultra Düşük Donanım / Yüksek FPS)",
                            "720p" => "720p (1280x720 - Düşük Sistem & Steam Deck)",
                            "800p" => "800p (1280x800 - Steam Deck 16:10 Özel)",
                            "native" => "Native 1080p (1920x1080 - Orijinal Kalite)",
                            "1440p" => "1440p (2560x1440 - 2K QHD)",
                            "1800p" => "1800p (3200x1800)",
                            "4K" => "4K (3840x2160 - Ultra HD)",
                            other => other,
                        };

                        let prev_res = self.config.render_resolution.clone();
                        egui::ComboBox::from_id_salt("res_combo")
                            .selected_text(res_text)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.config.render_resolution, "480p".to_string(), "480p (854x480 - Ultra Düşük)");
                                ui.selectable_value(&mut self.config.render_resolution, "720p".to_string(), "720p (1280x720 - Düşük / Steam Deck)");
                                ui.selectable_value(&mut self.config.render_resolution, "800p".to_string(), "800p (1280x800 - Steam Deck 16:10)");
                                ui.selectable_value(&mut self.config.render_resolution, "native".to_string(), "Native 1080p (1920x1080 - Orijinal Kalite)");
                                ui.selectable_value(&mut self.config.render_resolution, "1440p".to_string(), "1440p (2560x1440 - 2K QHD)");
                                ui.selectable_value(&mut self.config.render_resolution, "1800p".to_string(), "1800p (3200x1800)");
                                ui.selectable_value(&mut self.config.render_resolution, "4K".to_string(), "4K (3840x2160 - 4K Ultra HD)");
                            });
                        if prev_res != self.config.render_resolution {
                            self.config.check_preset_match();
                        }
                    });
                    ui.label(egui::RichText::new(self.t("res_desc")).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);

                    // --- 3. AMD FSR 1.0 & RCAS ---
                    let is_native = self.config.render_resolution == "native";
                    ui.add_enabled_ui(!is_native, |ui| {
                        ui.horizontal(|ui| {
                            let prev_fsr = self.config.fsr_enabled;
                            toggle_switch(ui, &mut self.config.fsr_enabled);
                            if ui.add(egui::Label::new(egui::RichText::new(self.t("fsr_toggle")).strong()).sense(egui::Sense::click())).clicked() {
                                self.config.fsr_enabled = !self.config.fsr_enabled;
                            }
                            if prev_fsr != self.config.fsr_enabled {
                                self.config.check_preset_match();
                            }
                        });
                    });
                    if is_native {
                        ui.label(egui::RichText::new("ℹ Native 1080p orijinal çözünürlükte FSR ölçekleme kapalıdır (1:1 doğal TAA render).").color(egui::Color32::from_rgb(170, 185, 215)).size(11.0));
                    } else {
                        ui.label(egui::RichText::new(self.t("fsr_desc")).color(egui::Color32::LIGHT_GRAY).size(11.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // --- 4. RPCS3 EFEKT KISMA (MOTION BLUR & DOF) ---
                    ui.horizontal(|ui| {
                        let prev_blur = self.config.disable_motion_blur;
                        toggle_switch(ui, &mut self.config.disable_motion_blur);
                        if ui.add(egui::Label::new(egui::RichText::new(self.t("disable_motion_blur")).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.disable_motion_blur = !self.config.disable_motion_blur;
                        }
                        if prev_blur != self.config.disable_motion_blur {
                            self.config.check_preset_match();
                        }
                    });
                    ui.label(egui::RichText::new(self.t("disable_motion_blur_desc")).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        let prev_dof = self.config.disable_dof;
                        toggle_switch(ui, &mut self.config.disable_dof);
                        if ui.add(egui::Label::new(egui::RichText::new(self.t("disable_dof")).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.disable_dof = !self.config.disable_dof;
                        }
                        if prev_dof != self.config.disable_dof {
                            self.config.check_preset_match();
                        }
                    });
                    ui.label(egui::RichText::new(self.t("disable_dof_desc")).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // --- 5. FPS / HZ KİLİDİ ---
                    let hz_label = self.t("hz_patch");
                    let target_fps_label = self.t("target_fps");
                    let hz_desc = self.t("hz_desc");
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.patch_delta_time);
                        if ui.add(egui::Label::new(egui::RichText::new(hz_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.patch_delta_time = !self.config.patch_delta_time;
                        }
                        ui.add_space(8.0);
                        ui.label(target_fps_label);
                        let current_text = match self.config.target_fps {
                            30 => "30 FPS (Konsol Kilidi & Düşük Sistem - Maksimum Kararlılık)".to_string(),
                            40 => "40 FPS (Steam Deck 40Hz & Düşük Sistem - Sıfır Takılma)".to_string(),
                            60 => "60 FPS (Varsayılan - Akıcı & Kararlı)".to_string(),
                            90 => "90 FPS (Steam Deck OLED 90Hz)".to_string(),
                            120 => "120 FPS (120Hz Ultra Akıcı - 60Hz Fizik Korumalı)".to_string(),
                            144 => "144 FPS (144Hz Ultra Akıcı - 60Hz Fizik Korumalı)".to_string(),
                            165 => "165 FPS (165Hz Ultra Akıcı - 60Hz Fizik Korumalı)".to_string(),
                            240 => "240 FPS (240Hz Maksimum - 60Hz Fizik Korumalı)".to_string(),
                            other => format!("{} FPS", other),
                        };
                        egui::ComboBox::from_id_salt("hz_combo")
                            .selected_text(current_text)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.config.target_fps, 30, "30 FPS (Konsol Kilidi & Düşük Sistem - Maksimum Kararlılık)");
                                ui.selectable_value(&mut self.config.target_fps, 40, "40 FPS (Steam Deck 40Hz & Düşük Sistem - Sıfır Takılma)");
                                ui.selectable_value(&mut self.config.target_fps, 60, "60 FPS (Varsayılan - Akıcı & Kararlı)");
                                ui.selectable_value(&mut self.config.target_fps, 90, "90 FPS (Steam Deck OLED 90Hz)");
                                ui.selectable_value(&mut self.config.target_fps, 120, "120 FPS (120Hz Ultra Akıcı - 60Hz Fizik Korumalı)");
                                ui.selectable_value(&mut self.config.target_fps, 144, "144 FPS (144Hz Ultra Akıcı - 60Hz Fizik Korumalı)");
                                ui.selectable_value(&mut self.config.target_fps, 165, "165 FPS (165Hz Ultra Akıcı - 60Hz Fizik Korumalı)");
                                ui.selectable_value(&mut self.config.target_fps, 240, "240 FPS (240Hz Maksimum - 60Hz Fizik Korumalı)");
                            });
                    });
                    ui.label(egui::RichText::new(hz_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);

                    // --- 6. READBACK PATCH (SARI RENK & DOKU BOZULMASI ÖNLEME) ---
                    let readback_label = self.t("readback_patch");
                    let readback_desc = self.t("readback_desc");
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.patch_readbacks);
                        if ui.add(egui::Label::new(egui::RichText::new(readback_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.patch_readbacks = !self.config.patch_readbacks;
                        }
                    });
                    ui.label(egui::RichText::new(readback_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);

                    // --- 7. ASENKRON GERİ OKUMA (DEFERRED READBACK - ANTİ-STUTTER / DROP ÖNLEME) ---
                    let async_readback_label = self.t("async_readbacks");
                    let async_readback_desc = self.t("async_readbacks_desc");
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.async_readbacks);
                        if ui.add(egui::Label::new(egui::RichText::new(async_readback_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.async_readbacks = !self.config.async_readbacks;
                        }
                    });
                    ui.label(egui::RichText::new(async_readback_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    ui.add_space(8.0);

                    // --- 8. VRAM GC BUDGET ---
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(self.t("vram_budget")).strong().color(egui::Color32::WHITE));
                        let vram_text = match self.config.vram_gc_budget_mb {
                            1280 => "1280 MB (2 GB GPU / Düşük Sistem - PCIe Taşma Korumalı)",
                            2048 => "2048 MB (3 GB GPU / Steam Deck)",
                            3072 => "3072 MB (4 GB GPU)",
                            4096 => "4096 MB (6 GB GPU)",
                            6144 => "6144 MB (8 GB GPU - Varsayılan)",
                            8192 => "8192 MB (10 GB+ GPU / Yüksek)",
                            _ => "Özel Bütçe",
                        };
                        egui::ComboBox::from_id_salt("vram_gc_combo")
                            .selected_text(vram_text)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.config.vram_gc_budget_mb, 1280, "1280 MB (2 GB GPU / Düşük Sistem - PCIe Taşma Korumalı)");
                                ui.selectable_value(&mut self.config.vram_gc_budget_mb, 2048, "2048 MB (3 GB GPU / Steam Deck)");
                                ui.selectable_value(&mut self.config.vram_gc_budget_mb, 3072, "3072 MB (4 GB GPU)");
                                ui.selectable_value(&mut self.config.vram_gc_budget_mb, 4096, "4096 MB (6 GB GPU)");
                                ui.selectable_value(&mut self.config.vram_gc_budget_mb, 6144, "6144 MB (8 GB GPU - Varsayılan)");
                                ui.selectable_value(&mut self.config.vram_gc_budget_mb, 8192, "8192 MB (10 GB+ GPU / Yüksek)");
                            });
                    });
                    ui.label(egui::RichText::new(self.t("vram_budget_desc")).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    let effective = self.config.get_effective_vram_budget();
                    if effective != self.config.vram_gc_budget_mb && self.config.cached_hardware.vram_mb > 0 {
                        ui.label(egui::RichText::new(format!(
                            "🛡️ Donanım Koruması Devrede: Ekran kartınızın kapasitesine ({} MB) göre aktif VRAM bütçesi {} MB olarak güvenli şekilde sınırlandırıldı.",
                            self.config.cached_hardware.vram_mb, effective
                        )).color(egui::Color32::from_rgb(102, 225, 106)).size(11.0));
                    }

                    ui.add_space(8.0);

                    // --- 9. SİNEMATİKLERİ ATLAMA ---
                    let skip_cinematics_label = self.t("skip_cinematics");
                    let skip_cinematics_desc = self.t("skip_cinematics_desc");
                    ui.horizontal(|ui| {
                        toggle_switch(ui, &mut self.config.skip_cinematics);
                        if ui.add(egui::Label::new(egui::RichText::new(skip_cinematics_label).strong()).sense(egui::Sense::click())).clicked() {
                            self.config.skip_cinematics = !self.config.skip_cinematics;
                        }
                    });
                    ui.label(egui::RichText::new(skip_cinematics_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));

                    // --- 9. FERAL GAMEMODE ---
                    if self.has_gamemode {
                        ui.add_space(8.0);
                        let gamemode_label = self.t("gamemode_toggle");
                        let gamemode_desc = self.t("gamemode_desc");
                        ui.horizontal(|ui| {
                            toggle_switch(ui, &mut self.config.use_gamemode);
                            if ui.add(egui::Label::new(egui::RichText::new(gamemode_label).strong()).sense(egui::Sense::click())).clicked() {
                                self.config.use_gamemode = !self.config.use_gamemode;
                            }
                        });
                        ui.label(egui::RichText::new(gamemode_desc).color(egui::Color32::LIGHT_GRAY).size(11.0));
                    }

                    ui.add_space(14.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // KAYDET VE UYGULA BUTONU
                    ui.horizontal(|ui| {
                        let save_btn = egui::Button::new(
                            egui::RichText::new(format!("💾 {}", self.t("save_apply")))
                                .strong()
                                .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(220, 38, 38))
                        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 120, 120, 140)))
                        .corner_radius(egui::CornerRadius::same(6))
                        .min_size(egui::vec2(180.0, 36.0));

                        let res = ui.add(save_btn);
                        if res.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if res.clicked() {
                            self.apply_all_optimizations();
                        }
                    });
                });
            } else {
                // ==========================================
                // TAB 2: CONFIGURATION & CONTROLS
                // ==========================================

                // --- 1. DISPLAY MODE & SCREEN SETTINGS ---
                render_section_tag(ui, self.t("tag_display"));
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
                render_section_tag(ui, self.t("tag_controller"));
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
                render_section_tag(ui, self.t("tag_keyboard"));
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
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let save_btn = egui::Button::new(
                        egui::RichText::new(format!("💾 {}", self.t("save_apply")))
                            .size(15.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    )
                    .fill(egui::Color32::from_rgb(220, 38, 38))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 120, 120, 140)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(220.0, 42.0));

                    let res = ui.add(save_btn);
                    if res.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if res.clicked() {
                        self.apply_all_optimizations();
                    }

                    let log_btn = egui::Button::new(
                        egui::RichText::new(format!("📋 {}", self.t("view_log")))
                            .size(14.0)
                            .color(egui::Color32::from_rgb(200, 205, 220)),
                    )
                    .fill(egui::Color32::from_rgb(28, 30, 42))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25)))
                    .corner_radius(egui::CornerRadius::same(8))
                    .min_size(egui::vec2(140.0, 42.0));

                    let res = ui.add(log_btn);
                    if res.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if res.clicked() {
                        let log_path = self.base_dir.join("gow3_log.txt");
                        self.log_content = std::fs::read_to_string(&log_path)
                            .unwrap_or_else(|_| "Log file is empty or not created yet.".to_string());
                        self.show_log_modal = true;
                    }
                });
            }

            ui.add_space(10.0);
        });

        // Dim background if any modal dialog is currently active
        let modal_active = self.found_candidates_modal.is_some()
            || self.show_game_not_found_modal
            || self.show_first_launch_modal
            || self.active_crash_diagnosis.is_some()
            || self.show_log_modal
            || self.update_modal.is_some();

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

        // --- MODAL 4: LOG MODAL ---
        if self.show_log_modal {
            egui::Window::new("gow3_log.txt")
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
                            if let Some(ref tag) = info.version_tag {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new("🏷️ Sürüm:")
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
                                    self.has_new_update = false;
                                    if let Some(ref tag) = info.version_tag {
                                        self.launcher_version = tag.clone();
                                    } else {
                                        self.launcher_version = crate::updater::detect_launcher_version(&self.base_dir, &self.config);
                                    }
                                    self.update_modal = None;
                                }
                                Err(e) => {
                                    self.status_message = Some((format!("Güncelleme hatası: {}", e), true));
                                }
                            }
                        }

                        if ui.button(self.t("view_on_github")).clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab("https://github.com/ScreamAngel/gow3_launcher"));
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
