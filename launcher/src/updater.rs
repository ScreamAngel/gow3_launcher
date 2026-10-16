use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config::LauncherConfig;
use crate::tuner::apply_settings;

pub const GITHUB_REPO: &str = "ScreamAngel/gow3_pclauncher";
pub const GITHUB_REPO_URL: &str = "https://github.com/ScreamAngel/gow3_pclauncher.git";

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
        .args(["ls-remote", GITHUB_REPO_URL, "HEAD"])
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
    let mut commit_message = "En son optimizasyonlar ve güncellemeler".to_string();
    let mut commit_date = String::new();

    let api_url = format!("https://api.github.com/repos/{}/commits/HEAD", GITHUB_REPO);
    if let Ok(api_out) = Command::new("curl")
        .args([
            "-s",
            "-H",
            "User-Agent: gow3-launcher",
            &api_url,
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
    let binary_name = if cfg!(target_os = "windows") {
        if base_dir.join("gow3_launcher_win.exe").is_file() {
            "gow3_launcher_win.exe"
        } else {
            "gow3-launcher-win.exe"
        }
    } else {
        "gow3-launcher"
    };

    let is_git_repo = base_dir.join(".git").is_dir();

    if is_git_repo {
        // Path A: User cloned via git, perform git pull
        let pull_output = Command::new("git")
            .args(["pull", "origin", "main"])
            .current_dir(base_dir)
            .output()
            .map_err(|e| format!("'git pull' çalıştırılamadı: {}", e))?;

        if !pull_output.status.success() {
            let err = String::from_utf8_lossy(&pull_output.stderr);
            return Err(format!("Git güncellemesi başarısız oldu: {}", err));
        }

        // Ensure launcher binary is executable
        let launcher_bin = base_dir.join(binary_name);
        if launcher_bin.is_file() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&launcher_bin, fs::Permissions::from_mode(0o755));
            }
        }
    } else {
        // Path B: Standalone folder without .git; download latest launcher binary and patch files
        let raw_launcher_url = format!(
            "https://raw.githubusercontent.com/{}/main/{}",
            GITHUB_REPO, binary_name
        );
        let launcher_bin = base_dir.join(binary_name);
        let launcher_tmp = base_dir.join(format!("{}.new", binary_name));

        let download = Command::new("curl")
            .args(["-fsSL", &raw_launcher_url, "-o", launcher_tmp.to_str().unwrap()])
            .output()
            .map_err(|e| format!("Yeni launcher indirilemedi: {}", e))?;

        if !download.status.success() {
            let _ = fs::remove_file(&launcher_tmp);
            return Err("GitHub üzerinden yeni launcher ikili dosyası indirilemedi.".to_string());
        }

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

        // Atomically replace running binary
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
        let _ = Command::new("curl")
            .args(["-fsSL", &raw_patch_url, "-o", patch_dest.to_str().unwrap()])
            .output();
    }

    // Re-apply optimizations and settings
    let _ = apply_settings(
        base_dir,
        config.target_fps,
        config.patch_delta_time,
        config.patch_readbacks,
        config.vram_gc_budget_mb,
        config.dmem_mb,
    );

    // Update last_git_commit in config
    config.last_git_commit = update.commit_sha.clone();
    let _ = config.save(base_dir);

    Ok("Launcher ve optimizasyonlar başarıyla güncellendi! Yeni sürümün geçerli olması için lütfen launcher'ı yeniden başlatın.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_github_update() {
        let res = check_github_update("test_dummy_sha");
        if let Ok(info) = res {
            assert!(!info.commit_sha.is_empty());
            assert!(info.is_new);
        }
    }
}
