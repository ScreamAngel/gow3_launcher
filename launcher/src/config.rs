use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

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
}
