//! Discord Rich Presence (RPC) Implementation for God of War III Launcher
//!
//! Provides zero-overhead, native local IPC communication with the desktop Discord client
//! on both Linux (Unix Domain Sockets) and Windows (Named Pipes).
//!
//! Features real-time Chapter & Boss Fight detection, Challenge of Olympus, Combat Arena,
//! and accurate story stage status reporting using the official God of War III: Remastered App ID.

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
    },
    StopGameWatcher,
    ClearActivity,
    UpdateConfig {
        enabled: bool,
    },
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
        let state = match lang {
            "tr" => "Ana Menüde",
            "de" => "Im Hauptmenü",
            "es" => "En el Menú Principal",
            _ => "In Main Menu",
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

    pub fn set_compiling_shaders(&self, percent: Option<f32>) {
        let state = if let Some(pct) = percent {
            format!("Shader'lar Derleniyor (%{:.0})", pct)
        } else {
            "Shader'lar Derleniyor...".to_string()
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

    pub fn set_in_game(&self, base_dir: PathBuf, start_unix_secs: i64, target_fps: u32, is_modded: bool) {
        let _ = self.tx.send(DiscordRpcMsg::StartGameWatcher {
            base_dir,
            start_unix_secs,
            target_fps,
            is_modded,
        });
    }

    pub fn clear(&self) {
        let _ = self.tx.send(DiscordRpcMsg::StopGameWatcher);
        let _ = self.tx.send(DiscordRpcMsg::ClearActivity);
    }

    pub fn update_config(&self, enabled: bool) {
        let _ = self.tx.send(DiscordRpcMsg::UpdateConfig { enabled });
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
    last_check: Instant,
    last_detected_state: String,
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
                } => {
                    // Initial game state
                    let (init_state, small_tip) = build_initial_game_state(&base_dir, target_fps, is_modded);
                    let act = DiscordActivity {
                        details: "God of War III: Remastered".to_string(),
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
                        last_check: Instant::now(),
                        last_detected_state: init_state,
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

                        let (new_state, small_tip) = build_initial_game_state(&w.base_dir, w.target_fps, w.is_modded);
                        if new_state != w.last_detected_state {
                            w.last_detected_state = new_state.clone();

                            let act = DiscordActivity {
                                details: "God of War III: Remastered".to_string(),
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

fn build_initial_game_state(base_dir: &Path, target_fps: u32, is_modded: bool) -> (String, String) {
    let save_info = find_latest_save_info(base_dir);
    let log_path = base_dir.join("gow3_log.txt");
    let wads = parse_recent_wads_from_log(&log_path);

    let state = detect_current_state(&wads, save_info.as_ref());

    let mut small_parts = Vec::new();
    if let Some(ref info) = save_info {
        if let Some(diff_line) = info.details.lines().next() {
            let clean_diff = diff_line.replace("Zorluk: ", "").replace("Difficulty: ", "").trim().to_string();
            if !clean_diff.is_empty() {
                small_parts.push(clean_diff);
            }
        }
    }
    if target_fps > 0 {
        small_parts.push(format!("{} FPS", target_fps));
    }
    if is_modded {
        small_parts.push("Modlu".to_string());
    }

    let small_tip = if small_parts.is_empty() {
        "PS4 Remastered on PC".to_string()
    } else {
        small_parts.join(" | ")
    };

    (state, small_tip)
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

/// Scans savedata directories for the latest modified save param.bin
pub fn find_latest_save_info(base_dir: &Path) -> Option<SaveParamInfo> {
    let candidate_dirs = [
        base_dir.join("user/savedata/1/CUSA01715"),
        base_dir.join("gow3-linux/user/savedata/1/CUSA01715"),
    ];

    let mut latest_file: Option<(PathBuf, std::time::SystemTime)> = None;

    for dir in &candidate_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
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

    if let Some((p, _)) = latest_file {
        if let Ok(data) = std::fs::read(p) {
            return parse_param_bin(&data);
        }
    }
    None
}

/// Reads the tail (up to 64 KB) of gow3_log.txt and extracts all recent WAD load markers
pub fn parse_recent_wads_from_log(log_path: &Path) -> Vec<String> {
    if !log_path.is_file() {
        return Vec::new();
    }

    let mut file = match std::fs::File::open(log_path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };

    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let read_start = file_len.saturating_sub(65536);
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
        } else if let Some(pos) = trimmed.find("LoadWadSizeData: WAD_") {
            let part = &trimmed[pos + 21..];
            let name = part.split_whitespace().next().unwrap_or("").trim();
            if !name.is_empty() {
                wads.push(name.to_uppercase());
            }
        }
    }
    wads
}

/// Computes the exact Rich Presence location & boss fight state based on WAD logs and save checkpoints
pub fn detect_current_state(recent_wads: &[String], save_info: Option<&SaveParamInfo>) -> String {
    // 1. Check Challenges, Combat Arena, Main Menu, Epilogue
    for w in recent_wads.iter().rev() {
        if w.starts_with("CHALLENGE") {
            return match w.as_str() {
                "CHALLENGE01" => "Olimpos Mücadelesi 1 (Taşlaşmış)".to_string(),
                "CHALLENGE02" => "Olimpos Mücadelesi 2 (Çıplak ve Cesur)".to_string(),
                "CHALLENGE03" => "Olimpos Mücadelesi 3 (Kıyma Makinesi)".to_string(),
                "CHALLENGE04" => "Olimpos Mücadelesi 4 (Koyun Avı)".to_string(),
                "CHALLENGE05" => "Olimpos Mücadelesi 5 (Temizleme)".to_string(),
                "CHALLENGE06" => "Olimpos Mücadelesi 6 (Yok Edici)".to_string(),
                "CHALLENGE07" => "Olimpos Mücadelesi 7 (Kör Nokta)".to_string(),
                _ => "Olimpos Mücadeleleri (Challenge of Olympus)".to_string(),
            };
        }
        if w.starts_with("FREE0") || w.starts_with("ARENA") {
            return "Savaş Arenası (Combat Arena)".to_string();
        }
        if w == "ENDOFGAME" {
            return "Oyun Sonu (Epilogue)".to_string();
        }
        if w == "SHELL" {
            return "Ana Menüde".to_string();
        }
    }

    // 2. Check Boss Fights (highest gameplay priority)
    for w in recent_wads.iter().rev() {
        if w.starts_with("R_POSHORSE")
            || w.starts_with("R_POSEIDON")
            || w.starts_with("R_POSMAIN")
            || w == "TITANFIGHT"
        {
            return "Olympus Dağı (Poseidon Boss Fight)".to_string();
        }
        if w.starts_with("R_HADES") || w.starts_with("HDS666") || w.starts_with("HDS675") {
            return "Yeraltı Dünyası (Hades Boss Fight)".to_string();
        }
        if w.starts_with("CRN") || w.contains("CRONOS") {
            return "Tartarus (Kronos Boss Fight)".to_string();
        }
        if w.starts_with("R_HERCULES") || w == "BAS500" {
            return "Forum (Herkül Boss Fight)".to_string();
        }
        if w.starts_with("R_ZEUS") || w.contains("KRATOSFEAR") || w.contains("PANDZEUS") {
            return "Olimpos Zirvesi (Zeus Boss Fight)".to_string();
        }
        if w.contains("HEPHAESTUS") {
            return "Demirhane (Hephaistos Boss Fight)".to_string();
        }
        if w.contains("SKORPIUS") {
            return "Mağaralar (Skorpius Boss Fight)".to_string();
        }
        if w.contains("HERMES") {
            return "Olimpos Yolu (Hermes Boss Fight)".to_string();
        }
        if w.contains("HELIOS") {
            return "Olimpiya Şehri (Helios Boss Fight)".to_string();
        }
    }

    // 3. Check Save Checkpoint Chapter
    if let Some(info) = save_info {
        let sc = info
            .chapter
            .replace("(Oto. Kayıt) ", "")
            .replace("(Auto Save) ", "")
            .replace("(Oto. Kayıt)", "")
            .trim()
            .to_string();

        if !sc.is_empty() {
            if sc.contains("Labirent") || sc.contains("Yıkık Odalar") || sc.contains("Işık Odaları") {
                return format!("Labirent ({})", sc);
            }
            if sc.contains("Ares") || sc.contains("Yargıç") || sc.contains("Hades") || sc.contains("Athena") {
                return format!("Yeraltı Dünyası ({})", sc);
            }
            if sc.contains("Olimpiya") {
                return format!("Olimpiya Şehri ({})", sc);
            }
            if sc.contains("Alev") {
                return format!("Alev Odası ({})", sc);
            }
            if sc.contains("Tartarus") {
                return format!("Tartarus ({})", sc);
            }
            if sc.contains("Afrodit") {
                return "Afrodit'in Odası".to_string();
            }
            if sc.contains("Forum") {
                return "Olimpos Forumu".to_string();
            }
            return sc;
        }
    }

    // 4. Fallback WAD Region Mapping
    for w in recent_wads.iter().rev() {
        if w.starts_with("POS") {
            return "Olympus Dağı".to_string();
        }
        if w.starts_with("HDS") {
            return "Yeraltı Dünyası".to_string();
        }
        if w.starts_with("OLY") {
            return "Olimpiya Şehri".to_string();
        }
        if w.starts_with("BAS") {
            return "Olimpos Tapınağı".to_string();
        }
        if w.starts_with("CAV") {
            return "Olimpos Mağaraları".to_string();
        }
        if w.starts_with("GAZ") {
            return "Olimpos Bahçeleri".to_string();
        }
        if w.starts_with("LAB") || w.starts_with("ICA") {
            return "Labirent".to_string();
        }
    }

    "Olympus Dağı".to_string()
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

    // Read Discord dispatch response to verify connection and clear buffer
    let mut header = [0u8; 8];
    if stream.read_exact(&mut header).is_ok() {
        let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        if length > 0 && length < 65536 {
            let mut buf = vec![0u8; length];
            let _ = stream.read_exact(&mut buf);
        }
    }

    Some(stream)
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
            details: "God of War III: Remastered".to_string(),
            state: "Olympus Dağı (Poseidon Boss Fight)".to_string(),
            start_timestamp: Some(1700000000),
            large_image: Some(GOW3_COVER_ICON_URL.to_string()),
            large_text: Some("God of War III: Remastered".to_string()),
            small_image: Some(GOW3_BADGE_ICON_URL.to_string()),
            small_text: Some("Zorluk: Tanrı (Normal)".to_string()),
        };

        let json_str = build_activity_payload(&act, 42);
        assert!(json_str.contains("SET_ACTIVITY"));
        assert!(json_str.contains("Olympus Dağı (Poseidon Boss Fight)"));
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
    fn test_detect_current_state_boss_and_chapters() {
        // Poseidon Boss Fight
        let wads_pos = vec!["POS050".to_string(), "R_POSHORSE00".to_string()];
        assert_eq!(
            detect_current_state(&wads_pos, None),
            "Olympus Dağı (Poseidon Boss Fight)"
        );

        // Underworld Hades Boss Fight
        let wads_hades = vec!["HDS675".to_string(), "R_HADES".to_string()];
        assert_eq!(
            detect_current_state(&wads_hades, None),
            "Yeraltı Dünyası (Hades Boss Fight)"
        );

        // Tartarus Cronos Boss Fight
        let wads_crn = vec!["CRN100".to_string()];
        assert_eq!(
            detect_current_state(&wads_crn, None),
            "Tartarus (Kronos Boss Fight)"
        );

        // Hercules Boss Fight
        let wads_herc = vec!["BAS500".to_string(), "R_HERCULES".to_string()];
        assert_eq!(
            detect_current_state(&wads_herc, None),
            "Forum (Herkül Boss Fight)"
        );

        // Zeus Boss Fight
        let wads_zeus = vec!["R_ZEUS00".to_string()];
        assert_eq!(
            detect_current_state(&wads_zeus, None),
            "Olimpos Zirvesi (Zeus Boss Fight)"
        );

        // Challenges
        let wads_chal = vec!["CHALLENGE01".to_string()];
        assert_eq!(
            detect_current_state(&wads_chal, None),
            "Olimpos Mücadelesi 1 (Taşlaşmış)"
        );

        // Arena
        let wads_arena = vec!["FREE0".to_string()];
        assert_eq!(
            detect_current_state(&wads_arena, None),
            "Savaş Arenası (Combat Arena)"
        );

        // Save Checkpoint
        let save = SaveParamInfo {
            chapter: "(Oto. Kayıt) Ares'in Mezarı".to_string(),
            details: "Zorluk: Tanrı (Normal)\nOynanış süresi: 0:13:36".to_string(),
        };
        let wads_hds_plain = vec!["HDS025".to_string()];
        assert_eq!(
            detect_current_state(&wads_hds_plain, Some(&save)),
            "Yeraltı Dünyası (Ares'in Mezarı)"
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
