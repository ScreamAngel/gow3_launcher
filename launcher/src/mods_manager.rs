use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledMod {
    pub folder_name: String,
    pub title: String,
    pub author: String,
    pub version: String,
    pub description: String,
    pub file_count: usize,
    pub sample_files: Vec<String>,
    pub is_enabled: bool,
}

pub fn resolve_mods_dir(base_dir: &Path) -> PathBuf {
    if base_dir.join("mods").is_dir() {
        base_dir.join("mods")
    } else if base_dir.join("gow3-linux/mods").is_dir() {
        base_dir.join("gow3-linux/mods")
    } else {
        base_dir.join("mods")
    }
}

pub fn open_mods_folder(base_dir: &Path) {
    let dir = resolve_mods_dir(base_dir);
    let _ = std::fs::create_dir_all(&dir);

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(&dir).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
    }
}

pub fn scan_installed_mods(base_dir: &Path, disabled_mods: &[String]) -> Vec<InstalledMod> {
    let mods_dir = resolve_mods_dir(base_dir);

    if !mods_dir.is_dir() {
        let _ = std::fs::create_dir_all(&mods_dir);
        return Vec::new();
    }

    let mut list = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let folder_name = entry.file_name().to_string_lossy().to_string();
            if folder_name.starts_with('.')
                || folder_name.eq_ignore_ascii_case("gow3-windows")
                || folder_name.eq_ignore_ascii_case("gow3-linux")
                || path.join("run.py").is_file()
            {
                continue;
            }

            let mut title = folder_name.clone();
            let mut author = "Bilinmeyen Yazar".to_string();
            let mut version = "v1.0".to_string();
            let mut description = String::new();

            // 1. Try reading mod.json
            let json_path = path.join("mod.json");
            if json_path.is_file() {
                if let Ok(text) = std::fs::read_to_string(&json_path) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(n) = val["name"].as_str() {
                            title = n.to_string();
                        }
                        if let Some(a) = val["author"].as_str() {
                            author = a.to_string();
                        }
                        if let Some(v) = val["version"].as_str() {
                            version = v.to_string();
                        }
                        if let Some(d) = val["description"].as_str() {
                            description = d.to_string();
                        }
                    }
                }
            }

            // 2. Try reading README.txt if description or author is still default
            if description.is_empty() {
                let readme_candidates = [
                    path.join("README.txt"),
                    path.join("readme.txt"),
                    path.join("README.md"),
                    path.join("readme.md"),
                ];
                for r in &readme_candidates {
                    if r.is_file() {
                        if let Ok(text) = std::fs::read_to_string(r) {
                            for line in text.lines() {
                                let l = line.trim();
                                if let Some(stripped) = l.strip_prefix("Mod Name :") {
                                    if title == folder_name {
                                        title = stripped.trim().to_string();
                                    }
                                } else if let Some(stripped) = l.strip_prefix("Author :") {
                                    author = stripped.trim().to_string();
                                } else if let Some(stripped) = l.strip_prefix("Description :") {
                                    description = stripped.trim().to_string();
                                } else if let Some(stripped) = l.strip_prefix("Version :") {
                                    version = stripped.trim().to_string();
                                }
                            }
                            if description.is_empty() {
                                description = text.chars().take(220).collect();
                            }
                            break;
                        }
                    }
                }
            }

            // 3. Scan mod files
            let mut file_count = 0;
            let mut sample_files = Vec::new();
            scan_mod_files_recursive(&path, &mut file_count, &mut sample_files);

            let is_enabled = !disabled_mods.contains(&folder_name);

            list.push(InstalledMod {
                folder_name,
                title,
                author,
                version,
                description,
                file_count,
                sample_files,
                is_enabled,
            });
        }
    }

    list.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    list
}

fn scan_mod_files_recursive(dir: &Path, count: &mut usize, samples: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                scan_mod_files_recursive(&p, count, samples);
            } else if p.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if lower.ends_with(".txt")
                    || lower.ends_with(".md")
                    || lower.ends_with(".json")
                    || lower.ends_with(".jpg")
                    || lower.ends_with(".png")
                    || lower.ends_with(".ini")
                {
                    continue;
                }
                *count += 1;
                if samples.len() < 3 {
                    samples.push(name);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_installed_mods() {
        let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        let mods = scan_installed_mods(&base_dir, &[]);
        assert!(mods.len() >= 2, "Should discover at least CJ and Golden Blades");
        
        let cj = mods.iter().find(|m| m.folder_name.contains("CJ"));
        assert!(cj.is_some(), "CJ mod should be found");
        let cj_mod = cj.unwrap();
        assert_eq!(cj_mod.file_count, 17);
        assert!(cj_mod.is_enabled);

        let gb = mods.iter().find(|m| m.folder_name.contains("Golden Blades"));
        assert!(gb.is_some(), "Golden Blades mod should be found");
        let gb_mod = gb.unwrap();
        assert_eq!(gb_mod.file_count, 1);
        assert_eq!(gb_mod.author, "Machi");
        assert!(gb_mod.is_enabled);
    }

    #[test]
    fn test_disabled_mods() {
        let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf();
        let disabled = vec!["Golden Blades".to_string()];
        let mods = scan_installed_mods(&base_dir, &disabled);
        let gb = mods.iter().find(|m| m.folder_name == "Golden Blades").unwrap();
        assert!(!gb.is_enabled);
    }
}
