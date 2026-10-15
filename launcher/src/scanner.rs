use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GameCandidate {
    pub path: PathBuf,
    pub title: String,
    pub title_id: String,
    pub version: String,
    pub eboot_size_mb: f32,
}

pub fn is_valid_game_dir(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }

    let eboot = path.join("eboot.bin");
    if !eboot.is_file() {
        return false;
    }

    // 1. Verify eboot.bin size (genuine God of War III eboot is > 1MB, typically ~4.5MB)
    if let Ok(meta) = fs::metadata(&eboot) {
        if meta.len() < 1_000_000 {
            return false;
        }
    } else {
        return false;
    }

    // 2. Verify sce_sys/param.sfo exists and has data
    let sfo = path.join("sce_sys/param.sfo");
    if !sfo.is_file() {
        return false;
    }
    if let Ok(meta) = fs::metadata(&sfo) {
        if meta.len() < 100 {
            return false;
        }
    } else {
        return false;
    }

    // 3. Verify at least one essential game directory exists (sce_module, EXEC, or language packs)
    let has_modules = path.join("sce_module").is_dir();
    let has_exec = path.join("EXEC").is_dir();
    let has_lang = [
        "ENGLISH", "DUTCH", "FRENCH", "GERMAN", "ITALIAN", "POLISH", "PORTUGUESE", "RUSSIAN", "SPANISH", "data",
    ]
    .iter()
    .any(|lang| path.join(lang).is_dir());

    has_modules || has_exec || has_lang
}

pub fn scan_for_game() -> Vec<GameCandidate> {
    let mut candidates = Vec::new();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());
    let mut scan_roots = vec![PathBuf::from(&home)];

    for root_str in &["/run/media", "/media", "/mnt"] {
        let p = PathBuf::from(root_str);
        if p.is_dir() {
            scan_roots.push(p);
        }
    }

    for root in scan_roots {
        scan_recursive(&root, 0, 5, &mut candidates);
    }

    // Deduplicate candidates by canonical path
    candidates.sort_by(|a, b| a.path.cmp(&b.path));
    candidates.dedup_by(|a, b| a.path == b.path);

    candidates
}

fn scan_recursive(dir: &Path, depth: usize, max_depth: usize, results: &mut Vec<GameCandidate>) {
    if depth > max_depth {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Skip hidden, system, and cache directories
        if name.starts_with('.') || name == "node_modules" || name == ".cache" || name == ".local" || name == "target" {
            continue;
        }

        if is_valid_game_dir(&path) {
            let (title, title_id, version) = inspect_param_sfo(&path);
            let eboot_size = fs::metadata(path.join("eboot.bin"))
                .map(|m| m.len() as f32 / (1024.0 * 1024.0))
                .unwrap_or(0.0);

            if title_id == "CUSA01623"
                || title_id == "CUSA01715"
                || name.contains("CUSA01623")
                || name.contains("CUSA01715")
                || title.contains("God of War")
            {
                results.push(GameCandidate {
                    path: path.clone(),
                    title,
                    title_id,
                    version,
                    eboot_size_mb: eboot_size,
                });
                continue;
            }
        }

        scan_recursive(&path, depth + 1, max_depth, results);
    }
}

pub fn inspect_param_sfo(game_dir: &Path) -> (String, String, String) {
    let sfo_path = game_dir.join("sce_sys").join("param.sfo");
    let title = "God of War® III Remastered".to_string();
    let mut title_id = "CUSA01623".to_string();
    let mut version = "01.00".to_string();

    if let Ok(bytes) = fs::read(&sfo_path) {
        if let Ok(text) = String::from_utf8(bytes.clone()) {
            if text.contains("CUSA01715") {
                title_id = "CUSA01715".to_string();
            }
        }
        // Extract readable strings
        if let Some(pos) = bytes.windows(4).position(|w| w == b"01.0") {
            if pos + 5 <= bytes.len() {
                if let Ok(v) = std::str::from_utf8(&bytes[pos..pos + 5]) {
                    version = v.to_string();
                }
            }
        }
    }

    (title, title_id, version)
}

pub fn update_launch_scripts(base_dir: &Path, chosen_path: &Path) -> Result<(), String> {
    let run_sh = base_dir.join("run.sh");
    if run_sh.is_file() {
        let mut content = fs::read_to_string(&run_sh).map_err(|e| e.to_string())?;
        let formatted = if chosen_path.starts_with(base_dir) {
            let rel = chosen_path.strip_prefix(base_dir).unwrap();
            format!("\"$SCRIPT_DIR/{}\"", rel.display())
        } else {
            format!("\"{}\"", chosen_path.display())
        };

        // Replace or inject DEFAULT_GAME_DIR
        let re_pattern = "DEFAULT_GAME_DIR=";
        if let Some(idx) = content.find(re_pattern) {
            if let Some(end_line) = content[idx..].find('\n') {
                let prefix = &content[..idx];
                let suffix = &content[idx + end_line..];
                content = format!("{}DEFAULT_GAME_DIR={}{}", prefix, formatted, suffix);
            }
        } else {
            // Fresh upstream run.sh: inject SCRIPT_DIR and DEFAULT_GAME_DIR
            if let Some(idx) = content.find("game=${GOW3_GAME_DIR:-") {
                let inject = format!(
                    "SCRIPT_DIR=\"$( cd \"$( dirname \"${{BASH_SOURCE[0]:-$0}}\" )\" &> /dev/null && pwd )\"\nDEFAULT_GAME_DIR={}\n\n",
                    formatted
                );
                content.insert_str(idx, &inject);
                if let Some(pos) = content.find("game=${GOW3_GAME_DIR:-") {
                    if let Some(close_brace) = content[pos..].find('}') {
                        let full_match = content[pos..pos + close_brace + 1].to_string();
                        content = content.replace(&full_match, "game=${GOW3_GAME_DIR:-$DEFAULT_GAME_DIR}");
                    }
                }
            }
        }
        fs::write(&run_sh, content).map_err(|e| e.to_string())?;
    }

    let start_sh = base_dir.join("start-gow3.sh");
    if start_sh.is_file() {
        let mut content = fs::read_to_string(&start_sh).map_err(|e| e.to_string())?;
        let formatted = if chosen_path.starts_with(base_dir) {
            let rel = chosen_path.strip_prefix(base_dir).unwrap();
            format!("\"$HERE/{}\"", rel.display())
        } else {
            format!("\"{}\"", chosen_path.display())
        };

        if let Some(idx) = content.find("export GOW3_GAME_DIR=") {
            if let Some(end_line) = content[idx..].find('\n') {
                let prefix = &content[..idx];
                let suffix = &content[idx + end_line..];
                content = format!("{}export GOW3_GAME_DIR=\"${{GOW3_GAME_DIR:-{}}}\"{}", prefix, formatted, suffix);
            }
        }
        fs::write(&start_sh, content).map_err(|e| e.to_string())?;
    }

    Ok(())
}
