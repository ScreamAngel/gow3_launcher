//! Discord Rich Presence (RPC) Implementation for God of War III Launcher
//!
//! Provides zero-overhead, native local IPC communication with the desktop Discord client
//! on both Linux (Unix Domain Sockets) and Windows (Named Pipes).
//!
//! Never blocks the main GUI thread; runs silently on a background worker thread.

use std::io::Write;
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::io::Read;

#[cfg(unix)]
type IpcStream = std::os::unix::net::UnixStream;

#[cfg(windows)]
type IpcStream = std::fs::File;

#[cfg(not(any(unix, windows)))]
type IpcStream = std::io::Cursor<Vec<u8>>;

/// Activity metadata sent to Discord
#[derive(Debug, Clone)]
pub struct DiscordActivity {
    pub details: String,
    pub state: String,
    pub start_timestamp: Option<i64>,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
}

pub enum DiscordRpcMsg {
    SetActivity(DiscordActivity),
    ClearActivity,
    UpdateConfig {
        enabled: bool,
        client_id: String,
    },
    Shutdown,
}

#[derive(Clone)]
pub struct DiscordRpc {
    tx: Sender<DiscordRpcMsg>,
}

impl DiscordRpc {
    pub fn new(enabled: bool, client_id: String) -> Self {
        let (tx, rx) = channel();

        thread::Builder::new()
            .name("discord-rpc-worker".to_string())
            .spawn(move || {
                run_rpc_worker(rx, enabled, client_id);
            })
            .expect("Failed to spawn discord-rpc-worker thread");

        Self { tx }
    }

    pub fn set_in_menu(&self, lang: &str) {
        let (state, details) = match lang {
            "tr" => ("Ana Menüde", "God of War III Remastered"),
            "de" => ("Im Hauptmenü", "God of War III Remastered"),
            "es" => ("En el Menú Principal", "God of War III Remastered"),
            _ => ("In Main Menu", "God of War III Remastered"),
        };

        let act = DiscordActivity {
            details: details.to_string(),
            state: state.to_string(),
            start_timestamp: None,
            large_image: Some("1124351711486689290".to_string()),
            large_text: Some("God of War III Remastered".to_string()),
            small_image: None,
            small_text: None,
        };
        let _ = self.tx.send(DiscordRpcMsg::SetActivity(act));
    }

    pub fn set_compiling_shaders(&self, percent: Option<f32>) {
        let state = if let Some(pct) = percent {
            format!("Shader'lar Derleniyor (%{:.0})", pct)
        } else {
            "Shader'lar Derleniyor...".to_string()
        };

        let act = DiscordActivity {
            details: "God of War III Remastered".to_string(),
            state,
            start_timestamp: None,
            large_image: Some("1124351711486689290".to_string()),
            large_text: Some("Vulkan Pipeline Shader Cache".to_string()),
            small_image: None,
            small_text: None,
        };
        let _ = self.tx.send(DiscordRpcMsg::SetActivity(act));
    }

    pub fn set_in_game(&self, start_unix_secs: i64, target_fps: u32, is_modded: bool) {
        let fps_tag = if target_fps > 0 {
            format!("{} FPS", target_fps)
        } else {
            "Uncapped FPS".to_string()
        };

        let mod_tag = if is_modded { " | Modlu" } else { "" };
        let state = format!("Olimpos'a Meydan Okuyor ({}{})", fps_tag, mod_tag);

        let act = DiscordActivity {
            details: "God of War III Remastered".to_string(),
            state,
            start_timestamp: Some(start_unix_secs),
            large_image: Some("1124351711486689290".to_string()),
            large_text: Some("God of War III Remastered".to_string()),
            small_image: Some("playstation".to_string()),
            small_text: Some("PS4 Remastered on PC".to_string()),
        };
        let _ = self.tx.send(DiscordRpcMsg::SetActivity(act));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(DiscordRpcMsg::ClearActivity);
    }

    pub fn update_config(&self, enabled: bool, client_id: String) {
        let _ = self.tx.send(DiscordRpcMsg::UpdateConfig { enabled, client_id });
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(DiscordRpcMsg::Shutdown);
    }
}

fn run_rpc_worker(rx: Receiver<DiscordRpcMsg>, mut enabled: bool, mut client_id: String) {
    let mut stream: Option<IpcStream> = None;
    let mut current_activity: Option<DiscordActivity> = None;
    let mut last_connect_attempt = Instant::now() - Duration::from_secs(10);
    let mut nonce: u64 = 1;

    loop {
        // 1. Process any pending messages with a 500ms timeout
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
                DiscordRpcMsg::ClearActivity => {
                    current_activity = None;
                    if let Some(ref mut s) = stream {
                        nonce += 1;
                        let _ = send_clear_payload(s, nonce);
                    }
                }
                DiscordRpcMsg::UpdateConfig { enabled: new_enabled, client_id: new_id } => {
                    let id_changed = client_id != new_id;
                    enabled = new_enabled;
                    client_id = new_id;

                    if id_changed || !enabled {
                        if let Some(ref mut s) = stream {
                            nonce += 1;
                            let _ = send_clear_payload(s, nonce);
                        }
                        stream = None;
                    }

                    if enabled && stream.is_none() {
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
                // Periodic maintenance check: if enabled and we have an activity but no active connection,
                // try to reconnect every 10 seconds (in case user opened Discord after launcher started)
                if enabled && current_activity.is_some() && stream.is_none() {
                    if last_connect_attempt.elapsed() >= Duration::from_secs(10) {
                        last_connect_attempt = Instant::now();
                        stream = try_connect_and_handshake(&client_id);
                        if let (Some(s), Some(act)) = (&mut stream, &current_activity) {
                            nonce += 1;
                            let _ = send_activity_payload(s, act, nonce);
                        }
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // Parent dropped channel, exit cleanly
                break;
            }
        }
    }
}

fn try_connect_and_handshake(client_id: &str) -> Option<IpcStream> {
    let mut stream = try_connect_ipc()?;

    // Send Handshake packet (Opcode 0)
    let handshake_payload = serde_json::json!({
        "v": 1,
        "client_id": client_id
    }).to_string();

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

    Some(stream)
}

#[cfg(unix)]
fn try_connect_ipc() -> Option<IpcStream> {
    let mut search_dirs = Vec::new();

    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(&runtime_dir);
        search_dirs.push(p.clone());
        search_dirs.push(p.join("app/com.discordapp.Discord"));
        search_dirs.push(p.join("snap.discord"));
    }

    search_dirs.push(PathBuf::from("/tmp"));

    for dir in &search_dirs {
        for i in 0..10 {
            let candidate = dir.join(format!("discord-ipc-{}", i));
            if candidate.exists() {
                if let Ok(stream) = std::os::unix::net::UnixStream::connect(&candidate) {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
                    return Some(stream);
                }
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
    activity_obj.insert("details".to_string(), serde_json::Value::String(activity.details.clone()));
    activity_obj.insert("state".to_string(), serde_json::Value::String(activity.state.clone()));

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
            details: "God of War III Remastered".to_string(),
            state: "Olimpos'a Meydan Okuyor (60 FPS)".to_string(),
            start_timestamp: Some(1700000000),
            large_image: Some("1124351711486689290".to_string()),
            large_text: Some("God of War III".to_string()),
            small_image: Some("playstation".to_string()),
            small_text: Some("PS4".to_string()),
        };

        let json_str = build_activity_payload(&act, 42);
        assert!(json_str.contains("SET_ACTIVITY"));
        assert!(json_str.contains("Olimpos'a Meydan Okuyor"));
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
    fn test_discord_rpc_handle_channels() {
        let rpc = DiscordRpc::new(false, "1124351711486689290".to_string());
        rpc.set_in_menu("tr");
        rpc.set_compiling_shaders(Some(50.0));
        rpc.set_in_game(1700000000, 60, true);
        rpc.clear();
        rpc.update_config(true, "1124351711486689290".to_string());
        rpc.shutdown();
    }
}
