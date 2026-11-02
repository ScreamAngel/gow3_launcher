use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const SHADER_REPO_URL: &str =
    "https://raw.githubusercontent.com/ScreamAngel/gow3_launcher/main/shaders/gow3-shader-cache.tar.xz";
pub const SHADER_FALLBACK_URL: &str =
    "https://github.com/ScreamAngel/gow3_launcher/releases/download/v1.9/gow3-shader-cache.tar.xz";

/// Resolves the shader cache directory for God of War III.
/// Handles both running from root repo or directly inside `gow3-linux`.
pub fn resolve_cache_dir(base_dir: &Path) -> PathBuf {
    if base_dir.join("gow3-linux/user").is_dir() {
        base_dir.join("gow3-linux/user/cache")
    } else {
        base_dir.join("user/cache")
    }
}

/// Counts total pre-compiled shader and pipeline key files for God of War III.
pub fn count_cached_shaders(base_dir: &Path) -> usize {
    let cache_dir = resolve_cache_dir(base_dir);
    let primary = cache_dir.join("CUSA01715");
    let fallback = cache_dir.join("CUSA01623");
    let target = if primary.is_dir() {
        primary
    } else if fallback.is_dir() {
        fallback
    } else {
        return 0;
    };

    let mut count = 0;
    if let Ok(entries) = fs::read_dir(target) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().map_or(false, |ext| ext == "spv" || ext == "meta" || ext == "key") {
                count += 1;
            }
        }
    }
    count
}

/// Finds a local pre-built shader archive if present in the repo or parent dirs.
pub fn find_local_shader_archive(base_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        base_dir.join("shaders/gow3-shader-cache.tar.xz"),
        base_dir.join("../shaders/gow3-shader-cache.tar.xz"),
    ];
    for c in &candidates {
        if c.is_file() {
            return Some(c.clone());
        }
    }
    if let Some(parent) = base_dir.parent() {
        let p = parent.join("shaders/gow3-shader-cache.tar.xz");
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Downloads and extracts the official complete shader cache.
/// Sets up cross-title compatibility for both CUSA01715 and CUSA01623.
pub fn download_and_install_shaders(base_dir: &Path) -> Result<usize, String> {
    let cache_dir = resolve_cache_dir(base_dir);
    let _ = fs::create_dir_all(&cache_dir);

    // 1. Check for local shader archive first (avoids unnecessary re-download)
    let archive_path = if let Some(local_path) = find_local_shader_archive(base_dir) {
        local_path
    } else {
        let temp_archive = cache_dir.join("gow3-shader-cache.tar.xz.tmp");
        let _ = fs::remove_file(&temp_archive);

        http_download_with_fallback(SHADER_REPO_URL, SHADER_FALLBACK_URL, &temp_archive)?;
        temp_archive
    };

    // 2. Extract archive into cache directory
    let extract_res = extract_tar_xz(&archive_path, &cache_dir);

    // If a temporary download file was used, clean it up
    if archive_path.to_string_lossy().ends_with(".tmp") {
        let _ = fs::remove_file(&archive_path);
    }

    extract_res?;

    // 3. Ensure cross-compatibility between CUSA01715 (EU/TR) and CUSA01623 (US)
    link_regional_caches(&cache_dir);

    let final_count = count_cached_shaders(base_dir);
    Ok(final_count)
}

fn http_download(url: &str, dest: &Path) -> Result<(), String> {
    // 1. Try curl (available on Linux, macOS, and modern Windows 10+)
    if let Ok(out) = Command::new("curl")
        .args(["-fsSL", "-H", "User-Agent: gow3-launcher", url, "-o", dest.to_str().unwrap_or("")])
        .output()
    {
        if out.status.success() && dest.is_file() {
            if let Ok(meta) = fs::metadata(dest) {
                if meta.len() > 1024 {
                    return Ok(());
                }
            }
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
                if let Ok(meta) = fs::metadata(dest) {
                    if meta.len() > 1024 {
                        return Ok(());
                    }
                }
            }
        }
    }

    Err(format!("Dosya indirilemedi: {}", url))
}

fn http_download_with_fallback(primary_url: &str, fallback_url: &str, dest: &Path) -> Result<(), String> {
    if http_download(primary_url, dest).is_ok() {
        return Ok(());
    }
    http_download(fallback_url, dest)
}

fn extract_tar_xz(archive: &Path, dest_dir: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(dest_dir);

    // 1. Try system tar (available on Linux, macOS, and modern Windows 10/11 tar.exe)
    if let Ok(st) = Command::new("tar")
        .args(["-xf", archive.to_str().unwrap_or(""), "-C", dest_dir.to_str().unwrap_or("")])
        .status()
    {
        if st.success() {
            return Ok(());
        }
    }

    // 2. Try 7z fallback (if user has 7z installed)
    if let Ok(st) = Command::new("7z")
        .args([
            "x",
            archive.to_str().unwrap_or(""),
            &format!("-o{}", dest_dir.to_str().unwrap_or("")),
            "-y",
        ])
        .status()
    {
        if st.success() {
            return Ok(());
        }
    }

    Err("Shader arşivi açılamadı (tar veya 7z aracı bulunamadı).".to_string())
}

/// Links CUSA01715 and CUSA01623 caches so both versions share the exact same shaders without duplication.
pub fn link_regional_caches(cache_dir: &Path) {
    let cusa1715 = cache_dir.join("CUSA01715");
    let cusa1623 = cache_dir.join("CUSA01623");

    if cusa1715.is_dir() && !cusa1623.exists() {
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink("CUSA01715", &cusa1623);
        }
        #[cfg(windows)]
        {
            let _ = Command::new("cmd")
                .args(["/C", "mklink", "/J", cusa1623.to_str().unwrap_or(""), cusa1715.to_str().unwrap_or("")])
                .output();
        }
    } else if cusa1623.is_dir() && !cusa1715.exists() {
        #[cfg(unix)]
        {
            let _ = std::os::unix::fs::symlink("CUSA01623", &cusa1715);
        }
        #[cfg(windows)]
        {
            let _ = Command::new("cmd")
                .args(["/C", "mklink", "/J", cusa1715.to_str().unwrap_or(""), cusa1623.to_str().unwrap_or("")])
                .output();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_cached_shaders() {
        let temp_dir = std::env::temp_dir().join("gow3_test_shader_count");
        let cache = temp_dir.join("user/cache/CUSA01715");
        let _ = fs::create_dir_all(&cache);

        fs::write(cache.join("0x123.spv"), b"spv").unwrap();
        fs::write(cache.join("0x123.meta"), b"meta").unwrap();
        fs::write(cache.join("0x123.key"), b"key").unwrap();
        fs::write(cache.join("ignore.txt"), b"txt").unwrap();

        let count = count_cached_shaders(&temp_dir);
        assert_eq!(count, 3);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_link_regional_caches() {
        let temp_dir = std::env::temp_dir().join("gow3_test_shader_link");
        let cache = temp_dir.join("user/cache");
        let cusa1715 = cache.join("CUSA01715");
        let _ = fs::create_dir_all(&cusa1715);
        fs::write(cusa1715.join("test.spv"), b"spv").unwrap();

        link_regional_caches(&cache);
        let cusa1623 = cache.join("CUSA01623");
        assert!(cusa1623.exists());

        let count = count_cached_shaders(&temp_dir);
        assert_eq!(count, 1);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
