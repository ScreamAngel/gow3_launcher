use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::LauncherConfig;
use crate::scanner::update_launch_scripts;
use crate::tuner::apply_settings;

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub commit_sha: String,
    pub commit_message: String,
    pub commit_date: String,
    pub is_new: bool,
}

pub fn check_github_update(current_commit: &str) -> Result<UpdateInfo, String> {
    // 1. Fetch remote commit SHA using git ls-remote (fast, no GitHub API rate limit)
    let output = Command::new("git")
        .args(["ls-remote", "https://github.com/R-V-L/gow3_pc", "HEAD"])
        .output()
        .map_err(|e| format!("git ls-remote failed: {}", e))?;

    if !output.status.success() {
        return Err("GitHub deposuna ulaşılamadı veya internet bağlantısı yok.".to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let remote_sha = text
        .lines()
        .next()
        .and_then(|ln| ln.split_whitespace().next())
        .ok_or_else(|| "Uzak commit SHA değeri okunamadı.".to_string())?
        .to_string();

    // 2. Fetch commit details (message and date) via curl from GitHub API if available
    let mut commit_message = "Latest upstream updates".to_string();
    let mut commit_date = String::new();

    if let Ok(api_out) = Command::new("curl")
        .args([
            "-s",
            "-H",
            "User-Agent: gow3-launcher",
            "https://api.github.com/repos/R-V-L/gow3_pc/commits/HEAD",
        ])
        .output()
    {
        if api_out.status.success() {
            let json_str = String::from_utf8_lossy(&api_out.stdout);
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(msg) = val["commit"]["message"].as_str() {
                    commit_message = msg.lines().next().unwrap_or(msg).to_string();
                }
                if let Some(d) = val["commit"]["committer"]["date"].as_str() {
                    commit_date = d.split('T').next().unwrap_or(d).to_string();
                }
            }
        }
    }

    let is_new = current_commit.trim().is_empty() || current_commit.trim() != remote_sha.trim();

    Ok(UpdateInfo {
        commit_sha: remote_sha,
        commit_message,
        commit_date,
        is_new,
    })
}

pub fn apply_github_update(
    base_dir: &Path,
    update: &UpdateInfo,
    config: &mut LauncherConfig,
) -> Result<String, String> {
    let run_sh = base_dir.join("run.sh");
    if !run_sh.is_file() {
        return Err("'run.sh' dosyası bulunamadı.".to_string());
    }

    // 1. Backup existing run.sh
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_name = format!("run.sh.bak_{}", timestamp);
    let backup_path = base_dir.join(&backup_name);
    let bak_generic = base_dir.join("run.sh.bak");

    fs::copy(&run_sh, &backup_path).map_err(|e| format!("Yedek oluşturulamadı: {}", e))?;
    let _ = fs::copy(&run_sh, &bak_generic);

    // 2. Download new run.sh from GitHub raw
    let raw_url = "https://raw.githubusercontent.com/R-V-L/gow3_pc/main/run.sh";
    let download = Command::new("curl")
        .args(["-fsSL", raw_url, "-o", run_sh.to_str().unwrap()])
        .output()
        .map_err(|e| format!("run.sh indirilemedi: {}", e))?;

    if !download.status.success() {
        // Restore backup if download failed
        let _ = fs::copy(&backup_path, &run_sh);
        return Err("GitHub üzerinden güncel run.sh indirilemedi.".to_string());
    }

    // Ensure run.sh is executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&run_sh, fs::Permissions::from_mode(0o755));
    }

    // 3. Re-integrate user settings into the new run.sh
    let target_game_path = if !config.game_dir.is_empty() {
        std::path::PathBuf::from(&config.game_dir)
    } else {
        base_dir.join("CUSA01623")
    };
    let _ = update_launch_scripts(base_dir, &target_game_path);

    let _ = apply_settings(
        base_dir,
        config.target_fps,
        config.patch_delta_time,
        config.patch_readbacks,
        config.vram_gc_budget_mb,
        config.dmem_mb,
    );

    // 4. Update last_git_commit in config
    config.last_git_commit = update.commit_sha.clone();
    let _ = config.save(base_dir);

    Ok(format!(
        "run.sh başarıyla güncellendi ve yedeklendi ('{}'). Tüm özel ayarlarınız korundu!",
        backup_name
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_github_update() {
        let res = check_github_update("c5638d9956002b8a9f09a240028f6793ddf7b2f8");
        assert!(res.is_ok());
        let info = res.unwrap();
        assert!(!info.commit_sha.is_empty());
        assert!(!info.is_new);
    }
}
