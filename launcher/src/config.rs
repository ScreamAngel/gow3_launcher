use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GraphicPreset {
    #[default]
    Balanced, // Standart / Orijinal (1080p)
    UltraLow, // Ultra Düşük Donanım (480p)
    Low,      // Düşük Sistem / Steam Deck (720p)
    SteamDeck,// Steam Deck 16:10 (800p)
    Ultra,    // Yüksek / 2K - 4K
    Custom,   // Özel (Kullanıcı Ayarlı)
}

impl GraphicPreset {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            GraphicPreset::Balanced => "balanced",
            GraphicPreset::UltraLow => "ultra_low",
            GraphicPreset::Low => "low",
            GraphicPreset::SteamDeck => "steam_deck",
            GraphicPreset::Ultra => "ultra",
            GraphicPreset::Custom => "custom",
        }
    }
}

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
    #[serde(default = "default_cpu_name")]
    pub cpu_name: String,
    #[serde(default = "default_cpu_cores")]
    pub cpu_cores: usize,
    #[serde(default = "default_cpu_threads")]
    pub cpu_threads: usize,
}

fn default_cpu_name() -> String {
    "Bilinmeyen CPU".to_string()
}

fn default_cpu_cores() -> usize {
    4
}

fn default_cpu_threads() -> usize {
    4
}

impl Default for CachedHardware {
    fn default() -> Self {
        Self {
            gpu_name: "Bilinmeyen GPU".to_string(),
            vram_mb: 4096,
            ram_mb: 8192,
            refresh_rate_hz: 60,
            cpu_name: "Bilinmeyen CPU".to_string(),
            cpu_cores: 4,
            cpu_threads: 4,
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
    pub async_readbacks: bool,
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
    #[serde(default = "default_launcher_version")]
    pub launcher_version: String,

    // Graphics Settings
    #[serde(default = "default_render_resolution")]
    pub render_resolution: String,
    #[serde(default = "default_true")]
    pub fsr_enabled: bool,
    #[serde(default)]
    pub disable_motion_blur: bool,
    #[serde(default)]
    pub disable_dof: bool,
    #[serde(default)]
    pub graphic_preset: GraphicPreset,

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
fn default_launcher_version() -> String {
    "v1.7".to_string()
}
fn default_render_resolution() -> String {
    "native".to_string()
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            first_launch_done: false,
            language: default_language(),
            launcher_version: default_launcher_version(),
            game_dir: String::new(),
            target_fps: default_target_fps(),
            patch_delta_time: true,
            patch_readbacks: true,
            async_readbacks: true,
            skip_cinematics: true,
            use_gamemode: true,
            vram_gc_budget_mb: default_vram_budget(),
            dmem_mb: default_dmem(),
            cached_hardware: CachedHardware::default(),
            last_git_commit: default_last_commit(),
            render_resolution: default_render_resolution(),
            fsr_enabled: false,
            disable_motion_blur: false,
            disable_dof: false,
            graphic_preset: GraphicPreset::Balanced,
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
    pub fn apply_preset(&mut self, preset: GraphicPreset) {
        self.graphic_preset = preset;
        match preset {
            GraphicPreset::UltraLow => {
                self.render_resolution = "480p".to_string();
                self.fsr_enabled = false;
                self.disable_motion_blur = true;
                self.disable_dof = true;
                self.vram_gc_budget_mb = 1280;
                self.target_fps = 60;
            }
            GraphicPreset::Low => {
                self.render_resolution = "720p".to_string();
                self.fsr_enabled = true;
                self.disable_motion_blur = true;
                self.disable_dof = true;
                self.vram_gc_budget_mb = 2048;
                self.target_fps = 60;
            }
            GraphicPreset::SteamDeck => {
                self.render_resolution = "800p".to_string();
                self.fsr_enabled = true;
                self.disable_motion_blur = true;
                self.disable_dof = true;
                self.vram_gc_budget_mb = 2048;
                self.target_fps = 40;
            }
            GraphicPreset::Balanced => {
                self.render_resolution = "native".to_string();
                self.fsr_enabled = false;
                self.disable_motion_blur = false;
                self.disable_dof = false;
                self.vram_gc_budget_mb = 6144;
                self.target_fps = 60;
            }
            GraphicPreset::Ultra => {
                self.render_resolution = "1440p".to_string();
                self.fsr_enabled = false;
                self.disable_motion_blur = false;
                self.disable_dof = false;
                self.vram_gc_budget_mb = 8192;
                self.target_fps = 120;
            }
            GraphicPreset::Custom => {
                // Keep customized values
            }
        }
    }

    pub fn check_preset_match(&mut self) {
        if self.render_resolution == "480p" && !self.fsr_enabled && self.disable_motion_blur && self.disable_dof {
            self.graphic_preset = GraphicPreset::UltraLow;
        } else if self.render_resolution == "720p" && self.fsr_enabled && self.disable_motion_blur && self.disable_dof {
            self.graphic_preset = GraphicPreset::Low;
        } else if self.render_resolution == "800p" && self.fsr_enabled && self.disable_motion_blur && self.disable_dof {
            self.graphic_preset = GraphicPreset::SteamDeck;
        } else if self.render_resolution == "native" && !self.fsr_enabled && !self.disable_motion_blur && !self.disable_dof {
            self.graphic_preset = GraphicPreset::Balanced;
        } else if (self.render_resolution == "1440p" || self.render_resolution == "4K") && !self.fsr_enabled && !self.disable_motion_blur && !self.disable_dof {
            self.graphic_preset = GraphicPreset::Ultra;
        } else {
            self.graphic_preset = GraphicPreset::Custom;
        }
    }

    pub fn get_resolution_dimensions(&self) -> (u32, u32) {
        match self.render_resolution.as_str() {
            "480p" => (854, 480),
            "720p" => (1280, 720),
            "800p" => (1280, 800),
            "1080p" => (1920, 1080),
            "1440p" => (2560, 1440),
            "4K" | "2160p" => (3840, 2160),
            _ => (1920, 1080),
        }
    }

    pub fn get_render_res_env_value(&self) -> Option<String> {
        if self.render_resolution == "native" {
            None
        } else {
            let (w, h) = self.get_resolution_dimensions();
            Some(format!("{}x{}", w, h))
        }
    }

    pub fn get_effective_vram_budget(&self) -> u64 {
        let vram = self.cached_hardware.vram_mb;
        if vram > 0 {
            // Guard against PCIe memory thrashing / swapping to system RAM over slow bus:
            // Desktop environment, window compositor and GPU drivers consume ~350-512MB of VRAM.
            // Texture GC budget must stay within available physical VRAM.
            let safe_ceiling = if vram <= 2048 {
                1280
            } else if vram <= 3072 {
                2048
            } else if vram <= 4096 {
                3072
            } else if vram <= 6144 {
                4608
            } else if vram <= 8192 {
                6144
            } else {
                vram.saturating_sub(1024)
            };
            self.vram_gc_budget_mb.min(safe_ceiling)
        } else {
            self.vram_gc_budget_mb
        }
    }
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

pub fn key_to_sdl_scancode(key: &str) -> Option<u8> {
    let lower = key.trim().to_lowercase();
    match lower.as_str() {
        // Letters (SDL_SCANCODE_A = 4 .. SDL_SCANCODE_Z = 29)
        "a" => Some(4),
        "b" => Some(5),
        "c" => Some(6),
        "d" => Some(7),
        "e" => Some(8),
        "f" => Some(9),
        "g" => Some(10),
        "h" => Some(11),
        "i" => Some(12),
        "j" => Some(13),
        "k" => Some(14),
        "l" => Some(15),
        "m" => Some(16),
        "n" => Some(17),
        "o" => Some(18),
        "p" => Some(19),
        "q" => Some(20),
        "r" => Some(21),
        "s" => Some(22),
        "t" => Some(23),
        "u" => Some(24),
        "v" => Some(25),
        "w" => Some(26),
        "x" => Some(27),
        "y" => Some(28),
        "z" => Some(29),

        // Number row (SDL_SCANCODE_1 = 30 .. SDL_SCANCODE_0 = 39)
        "1" => Some(30),
        "2" => Some(31),
        "3" => Some(32),
        "4" => Some(33),
        "5" => Some(34),
        "6" => Some(35),
        "7" => Some(36),
        "8" => Some(37),
        "9" => Some(38),
        "0" => Some(39),

        // Controls
        "enter" | "return" => Some(40),
        "escape" | "esc" => Some(41),
        "backspace" => Some(42),
        "tab" => Some(43),
        "space" => Some(44),

        // Function keys (SDL_SCANCODE_F1 = 58 ... F12 = 69)
        "f1" => Some(58),
        "f2" => Some(59),
        "f3" => Some(60),
        "f4" => Some(61),
        "f5" => Some(62),
        "f6" => Some(63),
        "f7" => Some(64),
        "f8" => Some(65),
        "f9" => Some(66),
        "f10" => Some(67),
        "f11" => Some(68),
        "f12" => Some(69),

        // Navigation
        "insert" => Some(73),
        "home" => Some(74),
        "pageup" | "prior" => Some(75),
        "delete" => Some(76),
        "end" => Some(77),
        "pagedown" | "next" => Some(78),

        // Arrow keys
        "right" => Some(79),
        "left" => Some(80),
        "down" => Some(81),
        "up" => Some(82),

        // Numpad (SDL_SCANCODE_KP_1 = 89 .. SDL_SCANCODE_KP_0 = 98)
        "kp_divide" | "kpdivide" | "numdivide" => Some(84),
        "kp_multiply" | "kpmultiply" | "nummultiply" => Some(85),
        "kp_minus" | "kpminus" | "numminus" => Some(86),
        "kp_plus" | "kpplus" | "numplus" => Some(87),
        "kp_enter" | "kpenter" | "numenter" => Some(88),
        "kp1" | "num1" | "numpad1" | "numpad 1" => Some(89),
        "kp2" | "num2" | "numpad2" | "numpad 2" => Some(90),
        "kp3" | "num3" | "numpad3" | "numpad 3" => Some(91),
        "kp4" | "num4" | "numpad4" | "numpad 4" => Some(92),
        "kp5" | "num5" | "numpad5" | "numpad 5" => Some(93),
        "kp6" | "num6" | "numpad6" | "numpad 6" => Some(94),
        "kp7" | "num7" | "numpad7" | "numpad 7" => Some(95),
        "kp8" | "num8" | "numpad8" | "numpad 8" => Some(96),
        "kp9" | "num9" | "numpad9" | "numpad 9" => Some(97),
        "kp0" | "num0" | "numpad0" | "numpad 0" => Some(98),
        "kp_period" | "kpperiod" | "kp_decimal" | "kpdecimal" => Some(99),

        // Modifiers
        "ctrl" | "lctrl" => Some(224),
        "shift" | "lshift" => Some(225),
        "alt" | "lalt" => Some(226),
        "rctrl" => Some(228),
        "rshift" => Some(229),
        "ralt" => Some(230),

        _ => None,
    }
}
