use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::LauncherConfig;
use crate::tuner::apply_settings;

pub const BUILTIN_VERSION: &str = "v1.7";
pub const GITHUB_REPO: &str = "ScreamAngel/gow3_launcher";
pub const GITHUB_REPO_URL: &str = "https://github.com/ScreamAngel/gow3_launcher.git";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub commit_sha: String,
    pub commit_message: String,
    pub commit_date: String,
    pub is_new: bool,
    pub version_tag: Option<String>,
    pub has_missing_files: bool,
    pub missing_files: Vec<String>,
}

pub fn parse_version_numbers(v: &str) -> Vec<u32> {
    v.trim_start_matches(|c: char| c == 'v' || c == 'V')
        .split('.')
        .filter_map(|s| {
            let num_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
            num_str.parse::<u32>().ok()
        })
        .collect()
}

pub fn compare_version_strings(v1: &str, v2: &str) -> std::cmp::Ordering {
    let p1 = parse_version_numbers(v1);
    let p2 = parse_version_numbers(v2);
    let max_len = p1.len().max(p2.len());
    for i in 0..max_len {
        let n1 = p1.get(i).copied().unwrap_or(0);
        let n2 = p2.get(i).copied().unwrap_or(0);
        match n1.cmp(&n2) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

pub fn detect_launcher_version(base_dir: &Path, config: &LauncherConfig) -> String {
    // 1. If in a git repository and git is installed, ask git for the exact tag
    if base_dir.join(".git").is_dir() && is_git_available() {
        if let Ok(output) = Command::new("git")
            .args(["describe", "--tags", "--abbrev=0"])
            .current_dir(base_dir)
            .output()
        {
            if output.status.success() {
                let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !tag.is_empty() {
                    let formatted = if tag.starts_with('v') || tag.starts_with('V') {
                        tag
                    } else {
                        format!("v{}", tag)
                    };
                    return formatted;
                }
            }
        }
    }

    // 2. If config has a recorded launcher_version and it's valid
    if !config.launcher_version.trim().is_empty() {
        let cv = config.launcher_version.trim();
        let formatted = if cv.starts_with('v') || cv.starts_with('V') {
            cv.to_string()
        } else {
            format!("v{}", cv)
        };
        if compare_version_strings(&formatted, BUILTIN_VERSION) >= std::cmp::Ordering::Equal {
            return formatted;
        }
    }

    // 3. Fallback to built-in compiled version
    BUILTIN_VERSION.to_string()
}

fn is_git_available() -> bool {
    Command::new("git").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn http_get_string(url: &str) -> Option<String> {
    // 1. Try curl (installed on Linux, macOS, and modern Windows 10+)
    if let Ok(out) = Command::new("curl")
        .args(["-fsSL", "-H", "User-Agent: gow3-launcher", url])
        .output()
    {
        if out.status.success() {
            let body = String::from_utf8_lossy(&out.stdout).to_string();
            if !body.trim().is_empty() {
                return Some(body);
            }
        }
    }

    // 2. Fallback to PowerShell (native on all Windows installations)
    #[cfg(target_os = "windows")]
    {
        let ps_code = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
             $wc = New-Object System.Net.WebClient; \
             $wc.Headers.Add('User-Agent', 'gow3-launcher'); \
             $wc.DownloadString('{}')",
            url.replace('\'', "''")
        );
        if let Ok(out) = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_code])
            .output()
        {
            if out.status.success() {
                let body = String::from_utf8_lossy(&out.stdout).to_string();
                if !body.trim().is_empty() {
                    return Some(body);
                }
            }
        }
    }

    None
}

fn http_download_file(url: &str, dest: &Path) -> Result<(), String> {
    // 1. Try curl
    if let Ok(out) = Command::new("curl")
        .args(["-fsSL", "-H", "User-Agent: gow3-launcher", url, "-o", dest.to_str().unwrap_or("")])
        .output()
    {
        if out.status.success() && dest.is_file() {
            return Ok(());
        }
    }

    // 2. Fallback to PowerShell on Windows
    #[cfg(target_os = "windows")]
    {
        let ps_code = format!(
            "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
             $wc = New-Object System.Net.WebClient; \
             $wc.Headers.Add('User-Agent', 'gow3-launcher'); \
             $wc.DownloadFile('{}', '{}')",
            url.replace('\'', "''"),
            dest.to_string_lossy().replace('\'', "''")
        );
        if let Ok(out) = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_code])
            .output()
        {
            if out.status.success() && dest.is_file() {
                return Ok(());
            }
        }
    }

    Err("Dosya indirilemedi.".to_string())
}

pub fn check_installation_integrity(base_dir: &Path) -> Vec<String> {
    let mut missing = Vec::new();

    // 1. Icon file
    if !base_dir.join("gow3.png").is_file() {
        missing.push("gow3.png".to_string());
    }

    // 2. Patches XML
    let has_patch = base_dir.join("patches/God_of_War_III_Remastered.xml").is_file()
        || base_dir.join(".runtime/patches/God_of_War_III_Remastered.xml").is_file();
    if !has_patch {
        missing.push("patches/God_of_War_III_Remastered.xml".to_string());
    }

    // 3. Auto tune script
    let has_script = base_dir.join("scripts/auto_tune.py").is_file()
        || base_dir.join(".runtime/scripts/auto_tune.py").is_file();
    if !has_script {
        missing.push("scripts/auto_tune.py".to_string());
    }

    missing
}

pub fn check_github_update(
    base_dir: &Path,
    _current_commit: &str,
    current_version: &str,
) -> Result<UpdateInfo, String> {
    let release_api_url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
    let commit_api_url = format!("https://api.github.com/repos/{}/commits/HEAD", GITHUB_REPO);

    let mut remote_sha = String::new();
    let mut commit_message = "En son optimizasyonlar ve güncellemeler".to_string();
    let mut commit_date = String::new();
    let mut version_tag: Option<String> = None;

    // 1. Fetch release tag and notes via GitHub Releases API
    if let Some(json_str) = http_get_string(&release_api_url) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(tag) = val["tag_name"].as_str() {
                let trimmed = tag.trim().to_string();
                if !trimmed.is_empty() {
                    version_tag = Some(trimmed);
                }
            }
            if let Some(body) = val["body"].as_str().filter(|b| !b.trim().is_empty()) {
                commit_message = body.lines().next().unwrap_or(body).to_string();
            }
            if let Some(d) = val["published_at"].as_str() {
                commit_date = d.split('T').next().unwrap_or(d).to_string();
            }
        }
    }

    // 2. Fetch commit info via GitHub Commits API
    if let Some(json_str) = http_get_string(&commit_api_url) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
            if let Some(sha) = val["sha"].as_str() {
                remote_sha = sha.to_string();
            }
            if let Some(msg) = val["commit"]["message"].as_str() {
                if commit_message == "En son optimizasyonlar ve güncellemeler" {
                    commit_message = msg.lines().next().unwrap_or(msg).to_string();
                }
            }
            if let Some(d) = val["commit"]["committer"]["date"].as_str() {
                if commit_date.is_empty() {
                    commit_date = d.split('T').next().unwrap_or(d).to_string();
                }
            }
        }
    }

    // 3. Fallback: only if GitHub API failed AND git is installed on system
    if remote_sha.is_empty() && is_git_available() {
        if let Ok(output) = Command::new("git")
            .args(["ls-remote", GITHUB_REPO_URL, "HEAD"])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(sha) = text.lines().next().and_then(|ln| ln.split_whitespace().next()) {
                    remote_sha = sha.to_string();
                }
            }
        }

        if version_tag.is_none() {
            if let Ok(output) = Command::new("git")
                .args(["ls-remote", "--tags", GITHUB_REPO_URL])
                .output()
            {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let mut highest_tag: Option<String> = None;
                    for line in text.lines() {
                        if let Some(ref_part) = line.split_whitespace().nth(1) {
                            if let Some(tag) = ref_part.strip_prefix("refs/tags/") {
                                if !tag.ends_with("^{}") && (tag.starts_with('v') || tag.starts_with('V')) {
                                    if let Some(ref h) = highest_tag {
                                        if compare_version_strings(tag, h) > std::cmp::Ordering::Equal {
                                            highest_tag = Some(tag.to_string());
                                        }
                                    } else {
                                        highest_tag = Some(tag.to_string());
                                    }
                                }
                            }
                        }
                    }
                    version_tag = highest_tag;
                }
            }
        }
    }

    if remote_sha.is_empty() && version_tag.is_none() {
        return Err("GitHub deposuna ulaşılamadı. İnternet bağlantınızı kontrol edin veya daha sonra tekrar deneyin.".to_string());
    }

    // Version Check:
    // Only true if the remote version tag is STRICTLY HIGHER than current running launcher version.
    let is_newer_version = if let Some(ref tag) = version_tag {
        compare_version_strings(tag, current_version) > std::cmp::Ordering::Equal
    } else {
        false
    };

    let missing_files = check_installation_integrity(base_dir);
    let has_missing_files = !missing_files.is_empty();

    // Do NOT flag update as needed if current launcher is already at or above the remote release tag!
    // Prevents redundant re-downloads when already running v1.7.
    let is_new = is_newer_version || has_missing_files;

    Ok(UpdateInfo {
        commit_sha: remote_sha,
        commit_message,
        commit_date,
        is_new,
        version_tag,
        has_missing_files,
        missing_files,
    })
}

pub fn apply_github_update(
    base_dir: &Path,
    update: &UpdateInfo,
    config: &mut LauncherConfig,
) -> Result<String, String> {
    let binary_name = if cfg!(target_os = "windows") {
        "gow3-launcher-win.exe"
    } else {
        "gow3-launcher"
    };

    let is_git_repo = base_dir.join(".git").is_dir() && is_git_available();

    if is_git_repo {
        // Path A: User cloned via git and git is available on system
        let pull_output = Command::new("git")
            .args(["pull", "--tags", "origin", "main"])
            .current_dir(base_dir)
            .output()
            .map_err(|e| format!("'git pull' çalıştırılamadı: {}", e))?;

        if !pull_output.status.success() {
            let err = String::from_utf8_lossy(&pull_output.stderr);
            return Err(format!("Git güncellemesi başarısız oldu: {}", err));
        }

        let launcher_bin = base_dir.join(binary_name);
        if launcher_bin.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&launcher_bin, fs::Permissions::from_mode(0o755));
            }
        }
    } else {
        // Path B: Standalone folder or git not installed; download latest launcher binary and patch files
        let raw_launcher_url = format!(
            "https://raw.githubusercontent.com/{}/main/{}",
            GITHUB_REPO, binary_name
        );
        let launcher_bin = base_dir.join(binary_name);
        let launcher_tmp = base_dir.join(format!("{}.new", binary_name));

        http_download_file(&raw_launcher_url, &launcher_tmp)
            .map_err(|e| format!("Yeni launcher indirilemedi ({}): {}", raw_launcher_url, e))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&launcher_tmp, fs::Permissions::from_mode(0o755));
        }

        #[cfg(target_os = "windows")]
        {
            let launcher_old = base_dir.join(format!("{}.old", binary_name));
            let _ = fs::remove_file(&launcher_old);
            let _ = fs::rename(&launcher_bin, &launcher_old);
        }

        fs::rename(&launcher_tmp, &launcher_bin)
            .map_err(|e| format!("Launcher güncellenemedi: {}", e))?;

        // Also fetch latest patch XML if present
        let patches_dir = base_dir.join("patches");
        let _ = fs::create_dir_all(&patches_dir);
        let raw_patch_url = format!(
            "https://raw.githubusercontent.com/{}/main/patches/God_of_War_III_Remastered.xml",
            GITHUB_REPO
        );
        let patch_dest = patches_dir.join("God_of_War_III_Remastered.xml");
        let _ = http_download_file(&raw_patch_url, &patch_dest);

        // Also fetch gow3.png if missing
        let icon_file = base_dir.join("gow3.png");
        if !icon_file.is_file() {
            let raw_icon_url = format!("https://raw.githubusercontent.com/{}/main/gow3.png", GITHUB_REPO);
            let _ = http_download_file(&raw_icon_url, &icon_file);
        }

        // Also ensure runtime assets are unpacked
        let _ = crate::runner::ensure_runtime_assets(base_dir);
    }

    let _ = apply_settings(base_dir, config);

    config.last_git_commit = update.commit_sha.clone();
    if let Some(ref tag) = update.version_tag {
        config.launcher_version = tag.clone();
    }
    let _ = config.save(base_dir);

    Ok("Launcher ve optimizasyonlar başarıyla güncellendi! Yeni sürümün geçerli olması için lütfen launcher'ı yeniden başlatın.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_github_update() {
        let res = check_github_update(Path::new(".."), "test_dummy_sha", "v0.1");
        if let Ok(info) = res {
            assert!(!info.commit_sha.is_empty() || info.version_tag.is_some());
            assert!(info.is_new);
        }

        // Test with current version v1.7: should NOT report as an update
        let res_current = check_github_update(Path::new(".."), "test_dummy_sha", "v1.7");
        if let Ok(info) = res_current {
            // Because remote release on GitHub is v1.6.1.6, and current is v1.7, and files are present, is_new must be false!
            assert!(!info.is_new);
        }
    }

    #[test]
    fn test_version_compare() {
        assert_eq!(compare_version_strings("v1.3", "v1.4"), std::cmp::Ordering::Less);
        assert_eq!(compare_version_strings("v1.4", "v1.4"), std::cmp::Ordering::Equal);
        assert_eq!(compare_version_strings("v1.5", "v1.4"), std::cmp::Ordering::Greater);
        assert_eq!(compare_version_strings("v1.10", "v1.9"), std::cmp::Ordering::Greater);
        assert_eq!(compare_version_strings("1.4", "v1.4"), std::cmp::Ordering::Equal);
    }
}
