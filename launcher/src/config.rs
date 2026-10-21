use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DisplayMode {
    #[default]
    Borderless,
    Fullscreen,
    Windowed,
}

impl DisplayMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayMode::Borderless => "borderless",
            DisplayMode::Fullscreen => "fullscreen",
            DisplayMode::Windowed => "windowed",
        }
    }

    pub fn to_toml_mode(&self) -> (&'static str, bool) {
        match self {
            DisplayMode::Borderless => ("Borderless", true),
            DisplayMode::Fullscreen => ("Fullscreen", true),
            DisplayMode::Windowed => ("Windowed", false),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardBindings {
    // Face buttons
    pub triangle: String,
    pub circle: String,
    pub cross: String,
    pub square: String,

    // Shoulder buttons
    pub l1: String,
    pub r1: String,
    pub l2: String,
    pub r2: String,
    pub l3: String,
    pub r3: String,

    // System
    pub options: String,
    pub touchpad_center: String,

    // D-Pad
    pub pad_up: String,
    pub pad_down: String,
    pub pad_left: String,
    pub pad_right: String,

    // Left Stick (Movement)
    pub axis_left_x_minus: String, // Sol (A)
    pub axis_left_x_plus: String,  // Sağ (D)
    pub axis_left_y_minus: String, // İleri (W)
    pub axis_left_y_plus: String,  // Geri (S)

    // Right Stick (Camera)
    pub axis_right_x_minus: String, // Sol (J)
    pub axis_right_x_plus: String,  // Sağ (L)
    pub axis_right_y_minus: String, // Yukarı (I)
    pub axis_right_y_plus: String,  // Aşağı (K)
}

impl Default for KeyboardBindings {
    fn default() -> Self {
        Self::wasd_cvbn()
    }
}

impl KeyboardBindings {
    pub fn wasd_cvbn() -> Self {
        Self {
            triangle: "c".to_string(),
            circle: "b".to_string(),
            cross: "n".to_string(),
            square: "v".to_string(),
            l1: "q".to_string(),
            r1: "u".to_string(),
            l2: "e".to_string(),
            r2: "o".to_string(),
            l3: "x".to_string(),
            r3: "m".to_string(),
            options: "enter".to_string(),
            touchpad_center: "space".to_string(),
            pad_up: "up".to_string(),
            pad_down: "down".to_string(),
            pad_left: "left".to_string(),
            pad_right: "right".to_string(),
            axis_left_x_minus: "a".to_string(),
            axis_left_x_plus: "d".to_string(),
            axis_left_y_minus: "w".to_string(),
            axis_left_y_plus: "s".to_string(),
            axis_right_x_minus: "j".to_string(),
            axis_right_x_plus: "l".to_string(),
            axis_right_y_minus: "i".to_string(),
            axis_right_y_plus: "k".to_string(),
        }
    }

    pub fn wasd_numpad() -> Self {
        Self {
            triangle: "kp8".to_string(),
            circle: "kp6".to_string(),
            cross: "kp2".to_string(),
            square: "kp4".to_string(),
            l1: "q".to_string(),
            r1: "u".to_string(),
            l2: "e".to_string(),
            r2: "o".to_string(),
            l3: "x".to_string(),
            r3: "m".to_string(),
            options: "enter".to_string(),
            touchpad_center: "space".to_string(),
            pad_up: "up".to_string(),
            pad_down: "down".to_string(),
            pad_left: "left".to_string(),
            pad_right: "right".to_string(),
            axis_left_x_minus: "a".to_string(),
            axis_left_x_plus: "d".to_string(),
            axis_left_y_minus: "w".to_string(),
            axis_left_y_plus: "s".to_string(),
            axis_right_x_minus: "j".to_string(),
            axis_right_x_plus: "l".to_string(),
            axis_right_y_minus: "i".to_string(),
            axis_right_y_plus: "k".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedHardware {
    pub gpu_name: String,
    pub vram_mb: u64,
    pub ram_mb: u64,
    pub refresh_rate_hz: u32,
}

impl Default for CachedHardware {
    fn default() -> Self {
        Self {
            gpu_name: "Bilinmeyen GPU".to_string(),
            vram_mb: 4096,
            ram_mb: 8192,
            refresh_rate_hz: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherConfig {
    #[serde(default)]
    pub first_launch_done: bool,
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub game_dir: String,
    #[serde(default = "default_target_fps")]
    pub target_fps: u32,
    #[serde(default = "default_true")]
    pub patch_delta_time: bool,
    #[serde(default = "default_true")]
    pub patch_readbacks: bool,
    #[serde(default = "default_true")]
    pub skip_cinematics: bool,
    #[serde(default = "default_true")]
    pub use_gamemode: bool,
    #[serde(default = "default_vram_budget")]
    pub vram_gc_budget_mb: u64,
    #[serde(default = "default_dmem")]
    pub dmem_mb: u64,
    #[serde(default)]
    pub cached_hardware: CachedHardware,
    #[serde(default = "default_last_commit")]
    pub last_git_commit: String,

    // Display settings
    #[serde(default)]
    pub display_mode: DisplayMode,
    #[serde(default = "default_true")]
    pub vsync: bool,

    // Controller settings
    #[serde(default)]
    pub background_controller_input: bool,
    #[serde(default = "default_true")]
    pub is_motion_controls_enabled: bool,
    #[serde(default = "default_deadzone")]
    pub deadzone_left: u32,
    #[serde(default = "default_deadzone")]
    pub deadzone_right: u32,

    // Keyboard bindings
    #[serde(default)]
    pub keyboard_bindings: KeyboardBindings,
}

fn default_language() -> String {
    "tr".to_string()
}
fn default_target_fps() -> u32 {
    60
}
fn default_true() -> bool {
    true
}
fn default_deadzone() -> u32 {
    2
}
fn default_vram_budget() -> u64 {
    6144
}
fn default_dmem() -> u64 {
    9216
}
fn default_last_commit() -> String {
    "c5638d9956002b8a9f09a240028f6793ddf7b2f8".to_string()
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            first_launch_done: false,
            language: default_language(),
            game_dir: String::new(),
            target_fps: default_target_fps(),
            patch_delta_time: true,
            patch_readbacks: true,
            skip_cinematics: true,
            use_gamemode: true,
            vram_gc_budget_mb: default_vram_budget(),
            dmem_mb: default_dmem(),
            cached_hardware: CachedHardware::default(),
            last_git_commit: default_last_commit(),
            display_mode: DisplayMode::default(),
            vsync: true,
            background_controller_input: false,
            is_motion_controls_enabled: true,
            deadzone_left: default_deadzone(),
            deadzone_right: default_deadzone(),
            keyboard_bindings: KeyboardBindings::default(),
        }
    }
}

impl LauncherConfig {
    pub fn config_path(base_dir: &Path) -> PathBuf {
        base_dir.join("launcher_config.json")
    }

    pub fn load(base_dir: &Path) -> Self {
        let path = Self::config_path(base_dir);
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = serde_json::from_str::<LauncherConfig>(&content) {
                    return cfg;
                }
            }
        }
        let default_cfg = Self::default();
        let _ = default_cfg.save(base_dir);
        default_cfg
    }

    pub fn save(&self, base_dir: &Path) -> std::io::Result<()> {
        let path = Self::config_path(base_dir);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)
    }

    pub fn build_gow3_key_map(&self) -> String {
        let b = &self.keyboard_bindings;
        let to_sdl = |k: &str| -> String {
            let k_low = k.trim().to_lowercase();
            match k_low.as_str() {
                "enter" | "return" => "Return".to_string(),
                "space" => "Space".to_string(),
                "escape" | "esc" => "Escape".to_string(),
                "backspace" => "Backspace".to_string(),
                "tab" => "Tab".to_string(),
                "up" => "Up".to_string(),
                "down" => "Down".to_string(),
                "left" => "Left".to_string(),
                "right" => "Right".to_string(),
                s if s.starts_with("kp") && s.len() == 3 && s[2..].chars().all(|c| c.is_ascii_digit()) => {
                    format!("Keypad {}", &s[2..])
                }
                "shift" | "lshift" => "Left Shift".to_string(),
                "rshift" => "Right Shift".to_string(),
                "ctrl" | "lctrl" => "Left Ctrl".to_string(),
                "rctrl" => "Right Ctrl".to_string(),
                "alt" | "lalt" => "Left Alt".to_string(),
                "ralt" => "Right Alt".to_string(),
                s if s.len() == 1 => s.to_uppercase(),
                s => s.to_string(),
            }
        };

        format!(
            "triangle={},square={},circle={},cross={},l1={},r1={},l2={},r2={},l3={},r3={},options={},touchpad={},up={},down={},left={},right={}",
            to_sdl(&b.triangle),
            to_sdl(&b.square),
            to_sdl(&b.circle),
            to_sdl(&b.cross),
            to_sdl(&b.l1),
            to_sdl(&b.r1),
            to_sdl(&b.l2),
            to_sdl(&b.r2),
            to_sdl(&b.l3),
            to_sdl(&b.r3),
            to_sdl(&b.options),
            to_sdl(&b.touchpad_center),
            to_sdl(&b.pad_up),
            to_sdl(&b.pad_down),
            to_sdl(&b.pad_left),
            to_sdl(&b.pad_right),
        )
    }

    pub fn build_gow3_pad_map(&self) -> &'static str {
        "cross=a,circle=b,square=x,triangle=y,l1=leftshoulder,r1=rightshoulder,l3=leftstick,r3=rightstick,options=start,touchpad=back,up=dpup,down=dpdown,left=dpleft,right=dpright"
    }
}
