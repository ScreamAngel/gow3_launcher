//! Discord Rich Presence (RPC) Implementation for God of War III Launcher
//!
//! Provides zero-overhead, native local IPC communication with the desktop Discord client
//! on both Linux (Unix Domain Sockets) and Windows (Named Pipes).
//!
//! Features real-time Chapter & Boss Fight detection, Challenge of Olympus, Combat Arena,
//! and multi-language automatic system/launcher localization (Turkish, English, German, Spanish).

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
type IpcStream = std::os::unix::net::UnixStream;

#[cfg(windows)]
type IpcStream = std::fs::File;

#[cfg(not(any(unix, windows)))]
type IpcStream = std::io::Cursor<Vec<u8>>;

/// Official Discord Application ID for God of War III: Remastered
pub const DISCORD_GOW3_APP_ID: &str = "1440140183675867156";

/// Official high-resolution icon from Discord CDN for God of War III: Remastered
pub const GOW3_COVER_ICON_URL: &str =
    "https://cdn.discordapp.com/app-icons/1440140183675867156/2cbf4c7faf6444d474ff7b32b2025ec2.png";

/// Custom project signature badge icon
pub const GOW3_BADGE_ICON_URL: &str =
    "https://raw.githubusercontent.com/ScreamAngel/gow3_launcher/main/gow3.png";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RpcLanguage {
    #[default]
    Turkish,
    English,
    German,
    Spanish,
}

impl RpcLanguage {
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "tr" => RpcLanguage::Turkish,
            "en" => RpcLanguage::English,
            "de" => RpcLanguage::German,
            "es" => RpcLanguage::Spanish,
            "auto" | "" => Self::from_system(),
            _ => Self::from_system(),
        }
    }

    pub fn from_system() -> Self {
        #[cfg(target_os = "windows")]
        {
            unsafe extern "system" {
                fn GetUserDefaultUILanguage() -> u16;
            }
            let lang_id = unsafe { GetUserDefaultUILanguage() };
            match lang_id & 0x3ff {
                0x1f => RpcLanguage::Turkish,
                0x07 => RpcLanguage::German,
                0x0a => RpcLanguage::Spanish,
                _ => RpcLanguage::English,
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let check_var = |var: &str| -> Option<RpcLanguage> {
                if let Ok(val) = std::env::var(var) {
                    let v = val.to_lowercase();
                    if v.starts_with("tr") {
                        return Some(RpcLanguage::Turkish);
                    }
                    if v.starts_with("de") {
                        return Some(RpcLanguage::German);
                    }
                    if v.starts_with("es") {
                        return Some(RpcLanguage::Spanish);
                    }
                    if v.starts_with("en") {
                        return Some(RpcLanguage::English);
                    }
                }
                None
            };
            check_var("LC_ALL")
                .or_else(|| check_var("LC_MESSAGES"))
                .or_else(|| check_var("LANG"))
                .unwrap_or(RpcLanguage::Turkish)
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            RpcLanguage::Turkish => "tr",
            RpcLanguage::English => "en",
            RpcLanguage::German => "de",
            RpcLanguage::Spanish => "es",
        }
    }
}

/// Detects the operating system display language
pub fn detect_system_language() -> &'static str {
    RpcLanguage::from_system().code()
}

/// Activity metadata sent to Discord
#[derive(Debug, Clone, PartialEq)]
pub struct DiscordActivity {
    pub details: String,
    pub state: String,
    pub start_timestamp: Option<i64>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SaveParamInfo {
    pub chapter: String,
    pub details: String,
}

pub enum DiscordRpcMsg {
    SetActivity(DiscordActivity),
    StartGameWatcher {
        base_dir: PathBuf,
        start_unix_secs: i64,
        target_fps: u32,
        is_modded: bool,
        lang: String,
    },
    StopGameWatcher,
    ClearActivity,
    UpdateConfig {
        enabled: bool,
    },
    UpdateLanguage(String),
    Shutdown,
}

#[derive(Clone)]
pub struct DiscordRpc {
    tx: Sender<DiscordRpcMsg>,
}

impl DiscordRpc {
    pub fn new(enabled: bool) -> Self {
        let (tx, rx) = channel();

        thread::Builder::new()
            .name("discord-rpc-worker".to_string())
            .spawn(move || {
                run_rpc_worker(rx, enabled);
            })
            .expect("Failed to spawn discord-rpc-worker thread");

        Self { tx }
    }

    pub fn set_in_menu(&self, lang: &str) {
        let rpc_lang = RpcLanguage::from_str(lang);
        let state = match rpc_lang {
            RpcLanguage::Turkish => "Ana Menüde",
            RpcLanguage::German => "Im Hauptmenü",
            RpcLanguage::Spanish => "En el Menú Principal",
            RpcLanguage::English => "In Main Menu",
        };

        let act = DiscordActivity {
            details: "God of War III: Remastered".to_string(),
            state: state.to_string(),
            start_timestamp: None,
            large_image: Some(GOW3_COVER_ICON_URL.to_string()),
            large_text: Some("God of War III: Remastered".to_string()),
            small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
            small_text: Some("PC Launcher".to_string()),
        };
        let _ = self.tx.send(DiscordRpcMsg::StopGameWatcher);
        let _ = self.tx.send(DiscordRpcMsg::SetActivity(act));
    }

    pub fn set_compiling_shaders(&self, percent: Option<f32>, lang: &str) {
        let rpc_lang = RpcLanguage::from_str(lang);
        let state = match rpc_lang {
            RpcLanguage::Turkish => {
                if let Some(pct) = percent {
                    format!("Shader'lar Derleniyor (%{:.0})", pct)
                } else {
                    "Shader'lar Derleniyor...".to_string()
                }
            }
            RpcLanguage::English => {
                if let Some(pct) = percent {
                    format!("Compiling Shaders ({:.0}%)", pct)
                } else {
                    "Compiling Shaders...".to_string()
                }
            }
            RpcLanguage::German => {
                if let Some(pct) = percent {
                    format!("Shader werden kompiliert (%{:.0})", pct)
                } else {
                    "Shader werden kompiliert...".to_string()
                }
            }
            RpcLanguage::Spanish => {
                if let Some(pct) = percent {
                    format!("Compilando Sombreadores (%{:.0})", pct)
                } else {
                    "Compilando Sombreadores...".to_string()
                }
            }
        };

        let act = DiscordActivity {
            details: "God of War III: Remastered".to_string(),
            state,
            start_timestamp: None,
            large_image: Some(GOW3_COVER_ICON_URL.to_string()),
            large_text: Some("Vulkan Pipeline Shader Cache".to_string()),
            small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
            small_text: Some("Vulkan Shader Pre-warming".to_string()),
        };
        let _ = self.tx.send(DiscordRpcMsg::StopGameWatcher);
        let _ = self.tx.send(DiscordRpcMsg::SetActivity(act));
    }

    pub fn set_in_game(
        &self,
        base_dir: PathBuf,
        start_unix_secs: i64,
        target_fps: u32,
        is_modded: bool,
        lang: &str,
    ) {
        let _ = self.tx.send(DiscordRpcMsg::StartGameWatcher {
            base_dir,
            start_unix_secs,
            target_fps,
            is_modded,
            lang: lang.to_string(),
        });
    }

    pub fn clear(&self) {
        let _ = self.tx.send(DiscordRpcMsg::StopGameWatcher);
        let _ = self.tx.send(DiscordRpcMsg::ClearActivity);
    }

    pub fn update_config(&self, enabled: bool) {
        let _ = self.tx.send(DiscordRpcMsg::UpdateConfig { enabled });
    }

    pub fn update_language(&self, lang: &str) {
        let _ = self.tx.send(DiscordRpcMsg::UpdateLanguage(lang.to_string()));
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(DiscordRpcMsg::Shutdown);
    }
}

struct GameWatcherState {
    base_dir: PathBuf,
    start_unix_secs: i64,
    target_fps: u32,
    is_modded: bool,
    lang: String,
    last_check: Instant,
    last_detected_state: String,
    last_detected_details: String,
}

fn run_rpc_worker(rx: Receiver<DiscordRpcMsg>, mut enabled: bool) {
    let client_id = DISCORD_GOW3_APP_ID.to_string();
    let mut stream: Option<IpcStream> = None;
    let mut current_activity: Option<DiscordActivity> = None;
    let mut last_connect_attempt = Instant::now() - Duration::from_secs(10);
    let mut watcher: Option<GameWatcherState> = None;
    let mut nonce: u64 = 1;

    loop {
        // Poll messages with a 500ms timeout
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(msg) => match msg {
                DiscordRpcMsg::SetActivity(act) => {
                    current_activity = Some(act.clone());
                    if enabled {
                        if stream.is_none() {
                            stream = try_connect_and_handshake(&client_id);
                        }
                        if let Some(ref mut s) = stream {
                            nonce += 1;
                            if send_activity_payload(s, &act, nonce).is_err() {
                                stream = None;
                            }
                        }
                    }
                }
                DiscordRpcMsg::StartGameWatcher {
                    base_dir,
                    start_unix_secs,
                    target_fps,
                    is_modded,
                    lang,
                } => {
                    let (init_state, init_details, small_tip) =
                        build_initial_game_state(&base_dir, target_fps, is_modded, &lang);
                    let act = DiscordActivity {
                        details: init_details.clone(),
                        state: init_state.clone(),
                        start_timestamp: Some(start_unix_secs),
                        large_image: Some(GOW3_COVER_ICON_URL.to_string()),
                        large_text: Some("God of War III: Remastered".to_string()),
                        small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
                        small_text: Some(small_tip),
                    };
                    current_activity = Some(act.clone());

                    if enabled {
                        if stream.is_none() {
                            stream = try_connect_and_handshake(&client_id);
                        }
                        if let Some(ref mut s) = stream {
                            nonce += 1;
                            if send_activity_payload(s, &act, nonce).is_err() {
                                stream = None;
                            }
                        }
                    }

                    watcher = Some(GameWatcherState {
                        base_dir,
                        start_unix_secs,
                        target_fps,
                        is_modded,
                        lang,
                        last_check: Instant::now(),
                        last_detected_state: init_state,
                        last_detected_details: init_details,
                    });
                }
                DiscordRpcMsg::StopGameWatcher => {
                    watcher = None;
                }
                DiscordRpcMsg::ClearActivity => {
                    watcher = None;
                    current_activity = None;
                    if let Some(ref mut s) = stream {
                        nonce += 1;
                        let _ = send_clear_payload(s, nonce);
                    }
                }
                DiscordRpcMsg::UpdateConfig { enabled: new_enabled } => {
                    enabled = new_enabled;
                    if !enabled {
                        if let Some(ref mut s) = stream {
                            nonce += 1;
                            let _ = send_clear_payload(s, nonce);
                        }
                        stream = None;
                    } else if stream.is_none() {
                        stream = try_connect_and_handshake(&client_id);
                        if let (Some(s), Some(act)) = (&mut stream, &current_activity) {
                            nonce += 1;
                            let _ = send_activity_payload(s, act, nonce);
                        }
                    }
                }
                DiscordRpcMsg::UpdateLanguage(new_lang) => {
                    if let Some(ref mut w) = watcher {
                        w.lang = new_lang.clone();
                        let (new_state, new_details, small_tip) =
                            build_initial_game_state(&w.base_dir, w.target_fps, w.is_modded, &w.lang);
                        w.last_detected_state = new_state.clone();
                        w.last_detected_details = new_details.clone();

                        let act = DiscordActivity {
                            details: new_details,
                            state: new_state,
                            start_timestamp: Some(w.start_unix_secs),
                            large_image: Some(GOW3_COVER_ICON_URL.to_string()),
                            large_text: Some("God of War III: Remastered".to_string()),
                            small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
                            small_text: Some(small_tip),
                        };
                        current_activity = Some(act.clone());

                        if enabled {
                            if stream.is_none() {
                                stream = try_connect_and_handshake(&client_id);
                            }
                            if let Some(ref mut s) = stream {
                                nonce += 1;
                                if send_activity_payload(s, &act, nonce).is_err() {
                                    stream = None;
                                }
                            }
                        }
                    }
                }
                DiscordRpcMsg::Shutdown => {
                    if let Some(ref mut s) = stream {
                        nonce += 1;
                        let _ = send_clear_payload(s, nonce);
                    }
                    break;
                }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Periodic maintenance check: if stream dropped, attempt reconnect every 5s
                if enabled && current_activity.is_some() && stream.is_none() {
                    if last_connect_attempt.elapsed() >= Duration::from_secs(5) {
                        last_connect_attempt = Instant::now();
                        stream = try_connect_and_handshake(&client_id);
                        if let (Some(s), Some(act)) = (&mut stream, &current_activity) {
                            nonce += 1;
                            let _ = send_activity_payload(s, act, nonce);
                        }
                    }
                }

                // Periodic game state watcher check every 2.5s
                if let Some(ref mut w) = watcher {
                    if w.last_check.elapsed() >= Duration::from_millis(2500) {
                        w.last_check = Instant::now();

                        let (new_state, new_details, small_tip) =
                            build_initial_game_state(&w.base_dir, w.target_fps, w.is_modded, &w.lang);
                        if new_state != w.last_detected_state || new_details != w.last_detected_details {
                            w.last_detected_state = new_state.clone();
                            w.last_detected_details = new_details.clone();

                            let act = DiscordActivity {
                                details: new_details,
                                state: new_state,
                                start_timestamp: Some(w.start_unix_secs),
                                large_image: Some(GOW3_COVER_ICON_URL.to_string()),
                                large_text: Some("God of War III: Remastered".to_string()),
                                small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
                                small_text: Some(small_tip),
                            };
                            current_activity = Some(act.clone());

                            if enabled {
                                if stream.is_none() {
                                    stream = try_connect_and_handshake(&client_id);
                                }
                                if let Some(ref mut s) = stream {
                                    nonce += 1;
                                    if send_activity_payload(s, &act, nonce).is_err() {
                                        stream = None;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }
}

/// Resolves the newest active gow3_log.txt across candidate paths
pub fn resolve_game_log_path(base_dir: &Path) -> Option<PathBuf> {
    let mut candidates = vec![
        base_dir.join("gow3_log.txt"),
        base_dir.join("gow3-linux/gow3_log.txt"),
    ];
    if let Some(parent) = base_dir.parent() {
        candidates.push(parent.join("gow3_log.txt"));
        candidates.push(parent.join("gow3-linux/gow3_log.txt"));
    }
    if let Ok(cur) = std::env::current_dir() {
        candidates.push(cur.join("gow3_log.txt"));
        candidates.push(cur.join("gow3-linux/gow3_log.txt"));
    }

    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    for path in candidates {
        if path.is_file() {
            if let Ok(meta) = path.metadata() {
                if let Ok(mod_time) = meta.modified() {
                    if newest.as_ref().map(|(_, t)| mod_time > *t).unwrap_or(true) {
                        newest = Some((path, mod_time));
                    }
                }
            }
        }
    }
    newest.map(|(p, _)| p)
}

fn build_initial_game_state(
    base_dir: &Path,
    target_fps: u32,
    is_modded: bool,
    lang: &str,
) -> (String, String, String) {
    let rpc_lang = RpcLanguage::from_str(lang);
    let save_info = find_latest_save_info(base_dir);
    let log_path = resolve_game_log_path(base_dir).unwrap_or_else(|| base_dir.join("gow3_log.txt"));
    let wads = parse_recent_wads_from_log(&log_path);

    let state = detect_current_state(&wads, save_info.as_ref(), rpc_lang);

    let mut small_parts = Vec::new();
    let mut details = match rpc_lang {
        RpcLanguage::Turkish => "Hikaye Modu",
        RpcLanguage::English => "Story Mode",
        RpcLanguage::German => "Story-Modus",
        RpcLanguage::Spanish => "Modo Historia",
    }
    .to_string();

    if let Some(ref info) = save_info {
        if let Some(diff_line) = info.details.lines().next() {
            let clean_diff = diff_line
                .replace("Zorluk: ", "")
                .replace("Difficulty: ", "")
                .replace("Schwierigkeit: ", "")
                .replace("Dificultad: ", "")
                .trim()
                .to_string();
            if !clean_diff.is_empty() {
                small_parts.push(clean_diff.clone());
                let diff_prefix = match rpc_lang {
                    RpcLanguage::Turkish => "Zorluk",
                    RpcLanguage::English => "Difficulty",
                    RpcLanguage::German => "Schwierigkeit",
                    RpcLanguage::Spanish => "Dificultad",
                };
                details = format!("{}: {}", diff_prefix, clean_diff);
            }
        }
    }

    if state.contains("Mücadele") || state.contains("Challenge") || state.contains("Herausforderung") || state.contains("Desafío") {
        details = match rpc_lang {
            RpcLanguage::Turkish => "Olympus Mücadeleleri",
            RpcLanguage::English => "Challenges of Olympus",
            RpcLanguage::German => "Herausforderungen des Olymp",
            RpcLanguage::Spanish => "Desafíos del Olimpo",
        }
        .to_string();
    } else if state.contains("Arena") || state.contains("Kampfarena") {
        details = match rpc_lang {
            RpcLanguage::Turkish => "Savaş Arenası",
            RpcLanguage::English => "Combat Arena",
            RpcLanguage::German => "Kampfarena",
            RpcLanguage::Spanish => "Arena de Combate",
        }
        .to_string();
    } else if state.contains("Menü") || state.contains("Menu") || state.contains("Menú") {
        details = "God of War III: Remastered".to_string();
    }

    if target_fps > 0 {
        small_parts.push(format!("{} FPS", target_fps));
    }
    if is_modded {
        let mod_label = match rpc_lang {
            RpcLanguage::Turkish => "Modlu",
            RpcLanguage::English => "Modded",
            RpcLanguage::German => "Gemoddet",
            RpcLanguage::Spanish => "Con Mods",
        };
        small_parts.push(mod_label.to_string());
    }

    let small_tip = if small_parts.is_empty() {
        "PS4 Remastered on PC".to_string()
    } else {
        small_parts.join(" | ")
    };

    (state, details, small_tip)
}

/// SFO param.bin parser extracting chapter and difficulty details from God of War III save data
pub fn parse_param_bin(data: &[u8]) -> Option<SaveParamInfo> {
    if data.len() < 0x180 {
        return None;
    }
    let raw_chap = &data[0x80..0x100];
    let chap_end = raw_chap.iter().position(|&b| b == 0).unwrap_or(raw_chap.len());
    let chapter = String::from_utf8_lossy(&raw_chap[..chap_end]).trim().to_string();

    let raw_det = &data[0x100..0x180];
    let det_end = raw_det.iter().position(|&b| b == 0).unwrap_or(raw_det.len());
    let details = String::from_utf8_lossy(&raw_det[..det_end]).trim().to_string();

    if chapter.is_empty() {
        None
    } else {
        Some(SaveParamInfo { chapter, details })
    }
}

/// Scans savedata directories recursively for the latest modified save param.bin
pub fn find_latest_save_info(base_dir: &Path) -> Option<SaveParamInfo> {
    let mut candidate_roots = vec![
        base_dir.join("user/savedata"),
        base_dir.join("gow3-linux/user/savedata"),
    ];
    if let Some(parent) = base_dir.parent() {
        candidate_roots.push(parent.join("user/savedata"));
        candidate_roots.push(parent.join("gow3-linux/user/savedata"));
    }
    if let Ok(cur) = std::env::current_dir() {
        candidate_roots.push(cur.join("user/savedata"));
        candidate_roots.push(cur.join("gow3-linux/user/savedata"));
    }

    let mut latest_file: Option<(PathBuf, std::time::SystemTime)> = None;

    for root in &candidate_roots {
        if !root.is_dir() {
            continue;
        }
        // Scan user directories (e.g. "1", "0", "00000001")
        if let Ok(user_entries) = std::fs::read_dir(root) {
            for u_entry in user_entries.flatten() {
                let u_path = u_entry.path();
                let title_dirs = if u_path.is_dir() {
                    vec![u_path.join("CUSA01715"), u_path]
                } else {
                    vec![]
                };
                for t_dir in title_dirs {
                    if let Ok(entries) = std::fs::read_dir(&t_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                            if file_name.ends_with(".sce_sys") && !file_name.starts_with("USERDATA") {
                                let param_path = path.join("param.bin");
                                if param_path.is_file() {
                                    if let Ok(meta) = param_path.metadata() {
                                        if let Ok(modified) = meta.modified() {
                                            if latest_file.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                                                latest_file = Some((param_path, modified));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some((p, _)) = latest_file {
        if let Ok(data) = std::fs::read(p) {
            return parse_param_bin(&data);
        }
    }
    None
}

/// Reads the tail (up to 2 MB) of gow3_log.txt and extracts all real runtime WAD load markers
pub fn parse_recent_wads_from_log(log_path: &Path) -> Vec<String> {
    if !log_path.is_file() {
        return Vec::new();
    }

    let mut file = match std::fs::File::open(log_path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let read_start = file_len.saturating_sub(2 * 1024 * 1024);
    let _ = file.seek(SeekFrom::Start(read_start));

    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    let text = String::from_utf8_lossy(&buf);

    let mut wads = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(pos) = trimmed.find("WAD_STATS: WAD ") {
            let part = &trimmed[pos + 15..];
            let name = part.split_whitespace().next().unwrap_or("").trim();
            if !name.is_empty() {
                wads.push(name.to_uppercase());
            }
        } else if let Some(pos) = trimmed.find("/WAD/PS4_LE/") {
            let part = &trimmed[pos + 12..];
            let name = part
                .split('.')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() && name != "REGION" && name != "DEPENDENCYGRAPH" {
                wads.push(name.to_uppercase());
            }
        }
    }
    wads
}

/// Identifies primary story level areas (POS050, HDS025, OLY100, etc.)
pub fn is_main_area_wad(w: &str) -> bool {
    let prefixes = ["POS", "HDS", "OLY", "ICA", "BAS", "CAV", "GAZ", "LAB", "CRN"];
    if w.contains("SKYBOX")
        || w.contains("AI")
        || w.contains("TEST")
        || w.contains("HRCHAINS")
        || w == "SHELL"
    {
        return false;
    }
    for p in prefixes {
        if w.starts_with(p) {
            let rest = &w[p.len()..];
            if !rest.is_empty() && rest.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                return true;
            }
        }
    }
    false
}

/// Maps boss encounter WADs to localized boss fight Rich Presence statuses
pub fn check_boss_wad(w: &str, lang: RpcLanguage) -> Option<String> {
    // Poseidon Boss Fight: Mount Olympus
    if w.starts_with("R_POSHORSE")
        || w.starts_with("R_POSEIDON")
        || w.starts_with("R_POSMAIN")
        || w.starts_with("R_POSHADES")
        || w == "TITANFIGHT"
    {
        return Some(match lang {
            RpcLanguage::Turkish => "Olympus Dağı (Poseidon Boss Fight)",
            RpcLanguage::English => "Mount Olympus (Poseidon Boss Fight)",
            RpcLanguage::German => "Olymp (Poseidon Bosskampf)",
            RpcLanguage::Spanish => "Monte Olimpo (Combate de Poseidón)",
        }.to_string());
    }

    // Hades Boss Fight: The Underworld
    if w.starts_with("R_HADES") || w.starts_with("HDS666") || w.starts_with("HDS675") {
        return Some(match lang {
            RpcLanguage::Turkish => "Yeraltı Dünyası (Hades Boss Fight)",
            RpcLanguage::English => "The Underworld (Hades Boss Fight)",
            RpcLanguage::German => "Die Unterwelt (Hades Bosskampf)",
            RpcLanguage::Spanish => "El Inframundo (Combate de Hades)",
        }.to_string());
    }

    // Helios Boss Fight / Chariot crash confrontation: City of Olympia
    if w == "R_HELIOS" || w == "OLY220" || w == "OLY225" || w == "OLY230" {
        return Some(match lang {
            RpcLanguage::Turkish => "Olimpiya Şehri (Helios Boss Fight)",
            RpcLanguage::English => "City of Olympia (Helios Boss Fight)",
            RpcLanguage::German => "Stadt Olympia (Helios Bosskampf)",
            RpcLanguage::Spanish => "Ciudad de Olimpia (Combate de Helios)",
        }.to_string());
    }

    // Hermes Chase & Fight: City of Olympia
    if w == "HERMES"
        || w == "HERMES10"
        || w.starts_with("R_HERMES")
        || w == "OLY900"
        || w == "OLY905"
        || w == "OLY910"
    {
        return Some(match lang {
            RpcLanguage::Turkish => "Olimpiya Şehri (Hermes Kovalamacası)",
            RpcLanguage::English => "City of Olympia (Hermes Chase)",
            RpcLanguage::German => "Stadt Olympia (Hermes Verfolgung)",
            RpcLanguage::Spanish => "Ciudad de Olimpia (Persecución de Hermes)",
        }.to_string());
    }

    // Hercules Boss Fight: Olympus Forum
    if w.starts_with("R_HERCULES") || w == "BAS500" || w == "BAS510" {
        return Some(match lang {
            RpcLanguage::Turkish => "Olympus Forumu (Hercules Boss Fight)",
            RpcLanguage::English => "Forum of Olympus (Hercules Boss Fight)",
            RpcLanguage::German => "Olymp-Forum (Herkules Bosskampf)",
            RpcLanguage::Spanish => "Foro del Olimpo (Combate de Hércules)",
        }.to_string());
    }

    // Cronos Boss Fight: Pit of Tartarus
    if w.starts_with("CRN") {
        return Some(match lang {
            RpcLanguage::Turkish => "Tartarus Çukuru (Cronos Boss Fight)",
            RpcLanguage::English => "Pit of Tartarus (Cronos Boss Fight)",
            RpcLanguage::German => "Grube des Tartaros (Kronos Bosskampf)",
            RpcLanguage::Spanish => "Foso del Tártaro (Combate de Cronos)",
        }.to_string());
    }

    // Hephaestus Boss Fight: The Forge
    if w.starts_with("R_HEPHAESTUS") {
        return Some(match lang {
            RpcLanguage::Turkish => "Demirhane (Hephaestus Boss Fight)",
            RpcLanguage::English => "The Forge (Hephaestus Boss Fight)",
            RpcLanguage::German => "Die Schmiede (Hephaistos Bosskampf)",
            RpcLanguage::Spanish => "La Forja (Combate de Hefesto)",
        }.to_string());
    }

    // Skorpius Boss Fight (Giant Scorpion): Caverns of Olympus
    if w.starts_with("R_SKORPIUS") || w == "CAV700" || w == "CAV800" {
        return Some(match lang {
            RpcLanguage::Turkish => "Olimpos Mağaraları (Skorpius Boss Fight)",
            RpcLanguage::English => "Caverns of Olympus (Skorpius Boss Fight)",
            RpcLanguage::German => "Höhlen des Olymp (Skorpius Bosskampf)",
            RpcLanguage::Spanish => "Cavernas del Olimpo (Combate de Skorpius)",
        }.to_string());
    }

    // Zeus Final Boss Fight: Summit of Olympus / Heart of Gaia
    if w.starts_with("R_ZEUS")
        || w.contains("KRATOSFEAR")
        || w.contains("PANDZEUS")
        || w.contains("ATHENAFEAR")
        || w.contains("WIFEFEAR")
        || w.contains("KIDFEAR")
    {
        return Some(match lang {
            RpcLanguage::Turkish => "Olympus Tepesi (Zeus Boss Fight)",
            RpcLanguage::English => "Summit of Olympus (Zeus Boss Fight)",
            RpcLanguage::German => "Gipfel des Olymp (Zeus Bosskampf)",
            RpcLanguage::Spanish => "Cima del Olimpo (Combate de Zeus)",
        }.to_string());
    }
    None
}

/// Computes the exact Rich Presence location & boss fight state based on WAD logs, save checkpoints, and language
pub fn detect_current_state(
    recent_wads: &[String],
    save_info: Option<&SaveParamInfo>,
    lang: RpcLanguage,
) -> String {
    let raw_save = save_info
        .map(|s| {
            s.chapter
                .replace("(Oto. Kayıt) ", "")
                .replace("(Auto Save) ", "")
                .replace("(Oto. Kayıt)", "")
                .trim()
                .to_string()
        })
        .unwrap_or_default();

    // Iterate backwards: newest gameplay event determines the active chapter/boss
    for w in recent_wads.iter().rev() {
        // 1. Challenges of Olympus
        if w.starts_with("CHALLENGE") {
            return match (w.as_str(), lang) {
                ("CHALLENGE01", RpcLanguage::Turkish) => "Olympus Mücadelesi 1 (Taşlaşmış)".to_string(),
                ("CHALLENGE01", RpcLanguage::English) => "Challenge of Olympus 1 (Stoned)".to_string(),
                ("CHALLENGE01", RpcLanguage::German) => "Herausforderung des Olymp 1 (Versteinert)".to_string(),
                ("CHALLENGE01", RpcLanguage::Spanish) => "Desafío del Olimpo 1 (Petrificado)".to_string(),

                ("CHALLENGE02", RpcLanguage::Turkish) => "Olympus Mücadelesi 2 (Çıplak ve Cesur)".to_string(),
                ("CHALLENGE02", RpcLanguage::English) => "Challenge of Olympus 2 (Bare Hands)".to_string(),
                ("CHALLENGE02", RpcLanguage::German) => "Herausforderung des Olymp 2 (Bloße Hände)".to_string(),
                ("CHALLENGE02", RpcLanguage::Spanish) => "Desafío del Olimpo 2 (Con las Manos Desnudas)".to_string(),

                ("CHALLENGE03", RpcLanguage::Turkish) => "Olympus Mücadelesi 3 (Kıyma Makinesi)".to_string(),
                ("CHALLENGE03", RpcLanguage::English) => "Challenge of Olympus 3 (Get Physical)".to_string(),
                ("CHALLENGE03", RpcLanguage::German) => "Herausforderung des Olymp 3 (Körperkontakt)".to_string(),
                ("CHALLENGE03", RpcLanguage::Spanish) => "Desafío del Olimpo 3 (Contacto Físico)".to_string(),

                ("CHALLENGE04", RpcLanguage::Turkish) => "Olympus Mücadelesi 4 (Koyun Avı)".to_string(),
                ("CHALLENGE04", RpcLanguage::English) => "Challenge of Olympus 4 (El Matador)".to_string(),
                ("CHALLENGE04", RpcLanguage::German) => "Herausforderung des Olymp 4 (El Matador)".to_string(),
                ("CHALLENGE04", RpcLanguage::Spanish) => "Desafío del Olimpo 4 (El Matador)".to_string(),

                ("CHALLENGE05", RpcLanguage::Turkish) => "Olympus Mücadelesi 5 (Temizleme)".to_string(),
                ("CHALLENGE05", RpcLanguage::English) => "Challenge of Olympus 5 (Knockout)".to_string(),
                ("CHALLENGE05", RpcLanguage::German) => "Herausforderung des Olymp 5 (Knockout)".to_string(),
                ("CHALLENGE05", RpcLanguage::Spanish) => "Desafío del Olimpo 5 (Noqueo)".to_string(),

                ("CHALLENGE06", RpcLanguage::Turkish) => "Olympus Mücadelesi 6 (Yok Edici)".to_string(),
                ("CHALLENGE06", RpcLanguage::English) => "Challenge of Olympus 6 (Hades' Kids)".to_string(),
                ("CHALLENGE06", RpcLanguage::German) => "Herausforderung des Olymp 6 (Kinder des Hades)".to_string(),
                ("CHALLENGE06", RpcLanguage::Spanish) => "Desafío del Olimpo 6 (Los Hijos de Hades)".to_string(),

                ("CHALLENGE07", RpcLanguage::Turkish) => "Olympus Mücadelesi 7 (Kör Nokta)".to_string(),
                ("CHALLENGE07", RpcLanguage::English) => "Challenge of Olympus 7 (Defiled)".to_string(),
                ("CHALLENGE07", RpcLanguage::German) => "Herausforderung des Olymp 7 (Entweiht)".to_string(),
                ("CHALLENGE07", RpcLanguage::Spanish) => "Desafío del Olimpo 7 (Profanado)".to_string(),

                (_, RpcLanguage::Turkish) => "Olympus Mücadeleleri".to_string(),
                (_, RpcLanguage::English) => "Challenges of Olympus".to_string(),
                (_, RpcLanguage::German) => "Herausforderungen des Olymp".to_string(),
                (_, RpcLanguage::Spanish) => "Desafíos del Olimpo".to_string(),
            };
        }

        // 2. Arena, Epilogue, Main Menu
        if w.starts_with("FREE0") || w.starts_with("ARENA") {
            return match lang {
                RpcLanguage::Turkish => "Savaş Arenası (Combat Arena)",
                RpcLanguage::English => "Combat Arena",
                RpcLanguage::German => "Kampfarena",
                RpcLanguage::Spanish => "Arena de Combate",
            }
            .to_string();
        }
        if w == "ENDOFGAME" {
            return match lang {
                RpcLanguage::Turkish => "Oyun Sonu (Epilogue)",
                RpcLanguage::English => "Epilogue (End of Game)",
                RpcLanguage::German => "Epilog (Spielende)",
                RpcLanguage::Spanish => "Epílogo (Fin del Juego)",
            }
            .to_string();
        }
        if w == "SHELL" {
            return match lang {
                RpcLanguage::Turkish => "Ana Menüde",
                RpcLanguage::English => "In Main Menu",
                RpcLanguage::German => "Im Hauptmenü",
                RpcLanguage::Spanish => "En el Menú Principal",
            }
            .to_string();
        }

        // 3. Boss Encounter
        if let Some(boss) = check_boss_wad(w, lang) {
            return boss;
        }

        // 4. Main Area / Level
        if is_main_area_wad(w) {
            if w.starts_with("POS") {
                return match lang {
                    RpcLanguage::Turkish => {
                        if raw_save.contains("Gaia") {
                            "Olympus Dağı (Gaia)".to_string()
                        } else {
                            "Olympus Dağı".to_string()
                        }
                    }
                    RpcLanguage::English => {
                        if raw_save.contains("Gaia") {
                            "Mount Olympus (Gaia)".to_string()
                        } else {
                            "Mount Olympus".to_string()
                        }
                    }
                    RpcLanguage::German => {
                        if raw_save.contains("Gaia") {
                            "Olymp (Gaia)".to_string()
                        } else {
                            "Olymp".to_string()
                        }
                    }
                    RpcLanguage::Spanish => {
                        if raw_save.contains("Gaia") {
                            "Monte Olimpo (Gaia)".to_string()
                        } else {
                            "Monte Olimpo".to_string()
                        }
                    }
                };
            }
            if w.starts_with("HDS") {
                return match lang {
                    RpcLanguage::Turkish => {
                        if !raw_save.is_empty() {
                            format!("Yeraltı Dünyası ({})", raw_save)
                        } else {
                            "Yeraltı Dünyası".to_string()
                        }
                    }
                    RpcLanguage::English => {
                        if raw_save.contains("Ares") {
                            "The Underworld (Tomb of Ares)".to_string()
                        } else if raw_save.contains("Yargıç") || raw_save.contains("Judges") {
                            "The Underworld (Three Judges)".to_string()
                        } else if raw_save.contains("Saray") || raw_save.contains("Palace") {
                            "The Underworld (Palace of Hades)".to_string()
                        } else if raw_save.contains("Derinlik") || raw_save.contains("Depths") {
                            "The Underworld (Depths of Hades)".to_string()
                        } else if !raw_save.is_empty() {
                            format!("The Underworld ({})", raw_save)
                        } else {
                            "The Underworld".to_string()
                        }
                    }
                    RpcLanguage::German => {
                        if raw_save.contains("Ares") {
                            "Die Unterwelt (Grab des Ares)".to_string()
                        } else if !raw_save.is_empty() {
                            format!("Die Unterwelt ({})", raw_save)
                        } else {
                            "Die Unterwelt".to_string()
                        }
                    }
                    RpcLanguage::Spanish => {
                        if raw_save.contains("Ares") {
                            "El Inframundo (Tumba de Ares)".to_string()
                        } else if !raw_save.is_empty() {
                            format!("El Inframundo ({})", raw_save)
                        } else {
                            "El Inframundo".to_string()
                        }
                    }
                };
            }
            if w.starts_with("OLY") {
                return match lang {
                    RpcLanguage::Turkish => {
                        if !raw_save.is_empty() {
                            format!("Olimpiya Şehri ({})", raw_save)
                        } else {
                            "Olimpiya Şehri".to_string()
                        }
                    }
                    RpcLanguage::English => "City of Olympia".to_string(),
                    RpcLanguage::German => "Stadt Olympia".to_string(),
                    RpcLanguage::Spanish => "Ciudad de Olimpia".to_string(),
                };
            }
            if w.starts_with("ICA") {
                return match lang {
                    RpcLanguage::Turkish => "Denge Zinciri (Eos Yolu)".to_string(),
                    RpcLanguage::English => "Chain of Balance (Path of Eos)".to_string(),
                    RpcLanguage::German => "Kette der Ausgeglichenheit (Pfad der Eos)".to_string(),
                    RpcLanguage::Spanish => "Cadena del Equilibrio (Camino de Eos)".to_string(),
                };
            }
            if w.starts_with("BAS") {
                return match lang {
                    RpcLanguage::Turkish => {
                        if raw_save.contains("Forum") {
                            "Olympus Forumu".to_string()
                        } else if !raw_save.is_empty() {
                            format!("Olimpos Tapınağı ({})", raw_save)
                        } else {
                            "Olimpos Tapınağı".to_string()
                        }
                    }
                    RpcLanguage::English => {
                        if raw_save.contains("Forum") {
                            "Forum of Olympus".to_string()
                        } else if raw_save.contains("Alev") || raw_save.contains("Flame") {
                            "Temple of Olympus (Chamber of the Flame)".to_string()
                        } else {
                            "Temple of Olympus".to_string()
                        }
                    }
                    RpcLanguage::German => "Tempel des Olymp".to_string(),
                    RpcLanguage::Spanish => "Templo del Olimpo".to_string(),
                };
            }
            if w.starts_with("CAV") {
                return match lang {
                    RpcLanguage::Turkish => "Olimpos Mağaraları".to_string(),
                    RpcLanguage::English => "Caverns of Olympus".to_string(),
                    RpcLanguage::German => "Höhlen des Olymp".to_string(),
                    RpcLanguage::Spanish => "Cavernas del Olimpo".to_string(),
                };
            }
            if w.starts_with("GAZ") {
                return match lang {
                    RpcLanguage::Turkish => {
                        if raw_save.contains("Afrodit") {
                            "Afrodit'in Odası".to_string()
                        } else {
                            "Olimpos Bahçeleri".to_string()
                        }
                    }
                    RpcLanguage::English => {
                        if raw_save.contains("Afrodit") || raw_save.contains("Aphrodite") {
                            "Aphrodite's Chamber".to_string()
                        } else {
                            "Gardens of Olympus".to_string()
                        }
                    }
                    RpcLanguage::German => {
                        if raw_save.contains("Afrodit") || raw_save.contains("Aphrodite") {
                            "Aphrodites Gemach".to_string()
                        } else {
                            "Gärten des Olymp".to_string()
                        }
                    }
                    RpcLanguage::Spanish => {
                        if raw_save.contains("Afrodit") || raw_save.contains("Afrodita") {
                            "Cámara de Afrodita".to_string()
                        } else {
                            "Jardines del Olimpo".to_string()
                        }
                    }
                };
            }
            if w.starts_with("LAB") {
                return match lang {
                    RpcLanguage::Turkish => {
                        if !raw_save.is_empty() {
                            format!("Labirent ({})", raw_save)
                        } else {
                            "Labirent".to_string()
                        }
                    }
                    RpcLanguage::English => {
                        if raw_save.contains("Yıkık") || raw_save.contains("Ruined") {
                            "The Labyrinth (Ruined Corridors)".to_string()
                        } else {
                            "The Labyrinth".to_string()
                        }
                    }
                    RpcLanguage::German => "Das Labyrinth".to_string(),
                    RpcLanguage::Spanish => "El Laberinto".to_string(),
                };
            }
            if w.starts_with("CRN") {
                return match lang {
                    RpcLanguage::Turkish => "Tartarus Çukuru".to_string(),
                    RpcLanguage::English => "Pit of Tartarus".to_string(),
                    RpcLanguage::German => "Grube des Tartaros".to_string(),
                    RpcLanguage::Spanish => "Foso del Tártaro".to_string(),
                };
            }
        }
    }

    // Fallback: Check save checkpoint
    if !raw_save.is_empty() {
        if raw_save.contains("Gaia") {
            return match lang {
                RpcLanguage::Turkish => "Olympus Dağı (Gaia)",
                RpcLanguage::English => "Mount Olympus (Gaia)",
                RpcLanguage::German => "Olymp (Gaia)",
                RpcLanguage::Spanish => "Monte Olimpo (Gaia)",
            }
            .to_string();
        }
        if raw_save.contains("Ares") || raw_save.contains("Yargıç") || raw_save.contains("Hades") {
            return match lang {
                RpcLanguage::Turkish => format!("Yeraltı Dünyası ({})", raw_save),
                RpcLanguage::English => format!("The Underworld ({})", raw_save),
                RpcLanguage::German => format!("Die Unterwelt ({})", raw_save),
                RpcLanguage::Spanish => format!("El Inframundo ({})", raw_save),
            };
        }
        if raw_save.contains("Olimpiya") || raw_save.contains("Olympia") {
            return match lang {
                RpcLanguage::Turkish => format!("Olimpiya Şehri ({})", raw_save),
                RpcLanguage::English => "City of Olympia".to_string(),
                RpcLanguage::German => "Stadt Olympia".to_string(),
                RpcLanguage::Spanish => "Ciudad de Olimpia".to_string(),
            };
        }
        if raw_save.contains("Forum") {
            return match lang {
                RpcLanguage::Turkish => "Olympus Forumu",
                RpcLanguage::English => "Forum of Olympus",
                RpcLanguage::German => "Olymp-Forum",
                RpcLanguage::Spanish => "Foro del Olimpo",
            }
            .to_string();
        }
        if raw_save.contains("Afrodit") || raw_save.contains("Aphrodite") {
            return match lang {
                RpcLanguage::Turkish => "Afrodit'in Odası",
                RpcLanguage::English => "Aphrodite's Chamber",
                RpcLanguage::German => "Aphrodites Gemach",
                RpcLanguage::Spanish => "Cámara de Afrodita",
            }
            .to_string();
        }
        if raw_save.contains("Tartarus") || raw_save.contains("Tártaro") {
            return match lang {
                RpcLanguage::Turkish => "Tartarus Çukuru",
                RpcLanguage::English => "Pit of Tartarus",
                RpcLanguage::German => "Grube des Tartaros",
                RpcLanguage::Spanish => "Foso del Tártaro",
            }
            .to_string();
        }
        if raw_save.contains("Labirent") || raw_save.contains("Labyrinth") || raw_save.contains("Laberinto") {
            return match lang {
                RpcLanguage::Turkish => format!("Labirent ({})", raw_save),
                RpcLanguage::English => format!("The Labyrinth ({})", raw_save),
                RpcLanguage::German => format!("Das Labyrinth ({})", raw_save),
                RpcLanguage::Spanish => format!("El Laberinto ({})", raw_save),
            };
        }
        return raw_save;
    }

    // Default opening state when game boots
    match lang {
        RpcLanguage::Turkish => "Olympus Dağı",
        RpcLanguage::English => "Mount Olympus",
        RpcLanguage::German => "Olymp",
        RpcLanguage::Spanish => "Monte Olimpo",
    }
    .to_string()
}

fn try_connect_and_handshake(client_id: &str) -> Option<IpcStream> {
    let mut stream = try_connect_ipc()?;

    // Send Handshake packet (Opcode 0)
    let handshake_payload = serde_json::json!({
        "v": 1,
        "client_id": client_id
    })
    .to_string();

    if send_frame(&mut stream, 0, &handshake_payload).is_err() {
        return None;
    }

    #[cfg(unix)]
    {
        // Read Discord dispatch response to verify connection and clear buffer
        let mut header = [0u8; 8];
        if stream.read_exact(&mut header).is_ok() {
            let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
            if length > 0 && length < 65536 {
                let mut buf = vec![0u8; length];
                let _ = stream.read_exact(&mut buf);
            }
        }
    }

    #[cfg(windows)]
    {
        // Safe non-blocking peek on Windows Named Pipe
        let mut header = [0u8; 8];
        for _ in 0..10 {
            if peek_pipe_bytes(&stream) >= 8 {
                if stream.read_exact(&mut header).is_ok() {
                    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
                    if length > 0 && length < 65536 {
                        let mut buf = vec![0u8; length];
                        let _ = stream.read_exact(&mut buf);
                    }
                }
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    Some(stream)
}

#[cfg(windows)]
fn peek_pipe_bytes(stream: &IpcStream) -> u32 {
    use std::os::windows::io::AsRawHandle;
    unsafe extern "system" {
        fn PeekNamedPipe(
            hNamedPipe: *mut std::ffi::c_void,
            lpBuffer: *mut std::ffi::c_void,
            nBufferSize: u32,
            lpBytesRead: *mut u32,
            lpTotalBytesAvail: *mut u32,
            lpBytesLeftThisMessage: *mut u32,
        ) -> i32;
    }
    unsafe {
        let handle = stream.as_raw_handle() as *mut std::ffi::c_void;
        let mut bytes_avail: u32 = 0;
        let res = PeekNamedPipe(
            handle,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut bytes_avail,
            std::ptr::null_mut(),
        );
        if res != 0 {
            bytes_avail
        } else {
            0
        }
    }
}

#[cfg(unix)]
fn try_connect_ipc() -> Option<IpcStream> {
    let mut search_dirs = Vec::new();

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(&runtime_dir);
        search_dirs.push(p.clone());
        search_dirs.push(p.join("app/com.discordapp.Discord"));
        search_dirs.push(p.join("app/dev.vencord.Vesktop"));
        search_dirs.push(p.join(".flatpak/dev.vencord.Vesktop/xdg-run"));
        search_dirs.push(p.join(".flatpak/com.discordapp.Discord/xdg-run"));
        search_dirs.push(p.join("snap.discord"));
    }

    if let Ok(home) = std::env::var("HOME") {
        let h = PathBuf::from(&home);
        search_dirs.push(h.join(".var/app/com.discordapp.Discord/data"));
        search_dirs.push(h.join(".var/app/dev.vencord.Vesktop/data"));
    }

    search_dirs.push(PathBuf::from("/tmp"));
    search_dirs.push(PathBuf::from("/tmp/app/com.discordapp.Discord"));

    for dir in &search_dirs {
        for i in 0..10 {
            let candidate = dir.join(format!("discord-ipc-{}", i));
            if candidate.exists() {
                if let Ok(stream) = std::os::unix::net::UnixStream::connect(&candidate) {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                    return Some(stream);
                }
            }
        }
        let vesktop_candidate = dir.join("vesktop-ipc");
        if vesktop_candidate.exists() {
            if let Ok(stream) = std::os::unix::net::UnixStream::connect(&vesktop_candidate) {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                return Some(stream);
            }
        }
    }
    None
}

#[cfg(windows)]
fn try_connect_ipc() -> Option<IpcStream> {
    for i in 0..10 {
        let pipe_name = format!(r"\\.\pipe\discord-ipc-{}", i);
        if let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(&pipe_name) {
            return Some(file);
        }
    }
    None
}

#[cfg(not(any(unix, windows)))]
fn try_connect_ipc() -> Option<IpcStream> {
    None
}

pub fn build_activity_payload(activity: &DiscordActivity, nonce: u64) -> String {
    let mut activity_obj = serde_json::Map::new();
    activity_obj.insert(
        "details".to_string(),
        serde_json::Value::String(activity.details.clone()),
    );
    activity_obj.insert(
        "state".to_string(),
        serde_json::Value::String(activity.state.clone()),
    );

    if let Some(start) = activity.start_timestamp {
        let mut timestamps = serde_json::Map::new();
        timestamps.insert("start".to_string(), serde_json::Value::Number(start.into()));
        activity_obj.insert("timestamps".to_string(), serde_json::Value::Object(timestamps));
    }

    let mut assets = serde_json::Map::new();
    if let Some(ref img) = activity.large_image {
        assets.insert("large_image".to_string(), serde_json::Value::String(img.clone()));
    }
    if let Some(ref txt) = activity.large_text {
        assets.insert("large_text".to_string(), serde_json::Value::String(txt.clone()));
    }
    if let Some(ref img) = activity.small_image {
        assets.insert("small_image".to_string(), serde_json::Value::String(img.clone()));
    }
    if let Some(ref txt) = activity.small_text {
        assets.insert("small_text".to_string(), serde_json::Value::String(txt.clone()));
    }

    if !assets.is_empty() {
        activity_obj.insert("assets".to_string(), serde_json::Value::Object(assets));
    }

    serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": activity_obj
        },
        "nonce": format!("{}", nonce)
    })
    .to_string()
}

pub fn build_clear_payload(nonce: u64) -> String {
    serde_json::json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": null
        },
        "nonce": format!("{}", nonce)
    })
    .to_string()
}

fn send_activity_payload<W: Write>(writer: &mut W, act: &DiscordActivity, nonce: u64) -> std::io::Result<()> {
    let payload = build_activity_payload(act, nonce);
    send_frame(writer, 1, &payload)
}

fn send_clear_payload<W: Write>(writer: &mut W, nonce: u64) -> std::io::Result<()> {
    let payload = build_clear_payload(nonce);
    send_frame(writer, 1, &payload)
}

fn send_frame<W: Write>(writer: &mut W, opcode: u32, payload: &str) -> std::io::Result<()> {
    let payload_bytes = payload.as_bytes();
    let length = payload_bytes.len() as u32;

    writer.write_all(&opcode.to_le_bytes())?;
    writer.write_all(&length.to_le_bytes())?;
    writer.write_all(payload_bytes)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_activity_payload_formatting() {
        let act = DiscordActivity {
            details: "Zorluk: Tanrı (Normal)".to_string(),
            state: "Olympus Dağı (Poseidon Boss Fight)".to_string(),
            start_timestamp: Some(1700000000),
            large_image: Some(GOW3_COVER_ICON_URL.to_string()),
            large_text: Some("God of War III: Remastered".to_string()),
            small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
            small_text: Some("144 FPS | Modlu".to_string()),
        };

        let json_str = build_activity_payload(&act, 42);
        assert!(json_str.contains("SET_ACTIVITY"));
        assert!(json_str.contains("Olympus Dağı (Poseidon Boss Fight)"));
        assert!(json_str.contains("Zorluk: Tanrı (Normal)"));
        assert!(json_str.contains("1700000000"));
        assert!(json_str.contains("\"nonce\":\"42\""));
    }

    #[test]
    fn test_clear_payload_formatting() {
        let json_str = build_clear_payload(99);
        assert!(json_str.contains("SET_ACTIVITY"));
        assert!(json_str.contains("\"activity\":null"));
        assert!(json_str.contains("\"nonce\":\"99\""));
    }

    #[test]
    fn test_detect_current_state_boss_and_chapters_tr() {
        let wads_pos = vec!["SHELL".to_string(), "POS050".to_string(), "R_POSHORSE00".to_string()];
        assert_eq!(
            detect_current_state(&wads_pos, None, RpcLanguage::Turkish),
            "Olympus Dağı (Poseidon Boss Fight)"
        );

        let wads_hades = vec!["HDS025".to_string(), "HDS675".to_string(), "R_HADES".to_string()];
        assert_eq!(
            detect_current_state(&wads_hades, None, RpcLanguage::Turkish),
            "Yeraltı Dünyası (Hades Boss Fight)"
        );

        let wads_crn = vec!["CRN100".to_string()];
        assert_eq!(
            detect_current_state(&wads_crn, None, RpcLanguage::Turkish),
            "Tartarus Çukuru (Cronos Boss Fight)"
        );
    }

    #[test]
    fn test_detect_current_state_multi_language() {
        let wads_pos = vec!["SHELL".to_string(), "POS050".to_string(), "R_POSHORSE00".to_string()];
        assert_eq!(
            detect_current_state(&wads_pos, None, RpcLanguage::English),
            "Mount Olympus (Poseidon Boss Fight)"
        );
        assert_eq!(
            detect_current_state(&wads_pos, None, RpcLanguage::German),
            "Olymp (Poseidon Bosskampf)"
        );
        assert_eq!(
            detect_current_state(&wads_pos, None, RpcLanguage::Spanish),
            "Monte Olimpo (Combate de Poseidón)"
        );

        let wads_hades = vec!["HDS025".to_string(), "HDS675".to_string(), "R_HADES".to_string()];
        assert_eq!(
            detect_current_state(&wads_hades, None, RpcLanguage::English),
            "The Underworld (Hades Boss Fight)"
        );
        assert_eq!(
            detect_current_state(&wads_hades, None, RpcLanguage::Spanish),
            "El Inframundo (Combate de Hades)"
        );

        let wads_chal = vec!["CHALLENGE01".to_string()];
        assert_eq!(
            detect_current_state(&wads_chal, None, RpcLanguage::English),
            "Challenge of Olympus 1 (Stoned)"
        );
    }

    #[test]
    fn test_parse_param_bin() {
        let mut dummy = vec![0u8; 0x200];
        let title = b"God of War\xc2\xae III Remastered";
        dummy[0..title.len()].copy_from_slice(title);

        let chap = "(Oto. Kayıt) Labirent".as_bytes();
        dummy[0x80..0x80 + chap.len()].copy_from_slice(chap);

        let det = "Zorluk: Tanrı (Normal)".as_bytes();
        dummy[0x100..0x100 + det.len()].copy_from_slice(det);

        let parsed = parse_param_bin(&dummy).expect("Should parse dummy SFO param");
        assert_eq!(parsed.chapter, "(Oto. Kayıt) Labirent");
        assert_eq!(parsed.details, "Zorluk: Tanrı (Normal)");
    }
}
