use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::config::LauncherConfig;

// Embedded Python runtime scripts
pub const SCRIPT_PREPARE: &str = include_str!("../../scripts/prepare.py");
pub const SCRIPT_LINK_LIBC: &str = include_str!("../../scripts/link_libc.py");
pub const SCRIPT_LINK_MODULES: &str = include_str!("../../scripts/link_modules.py");
pub const SCRIPT_CONTENT_PROFILE: &str = include_str!("../../scripts/content_profile.py");
pub const SCRIPT_LAUNCH_ENV: &str = include_str!("../../scripts/launch_env.py");
pub const SCRIPT_PATCHES: &str = include_str!("../../scripts/patches.py");
pub const SCRIPT_MODS: &str = include_str!("../../scripts/mods.py");
pub const SCRIPT_GRAPHICS_OPTIONS: &str = include_str!("../../scripts/graphics_options.py");
pub const SCRIPT_SAVE_BACKUP: &str = include_str!("../../scripts/save_backup.py");
pub const SCRIPT_AUTO_TUNE: &str = include_str!("../../scripts/auto_tune.py");
pub const SCRIPT_FIND_GAME: &str = include_str!("../../scripts/find_game.py");

// Embedded XML Patches
pub const PATCH_GOW3_XML: &str = include_str!("../../patches/God_of_War_III_Remastered.xml");

/// Ensures that embedded scripts and patches are unpacked into `.runtime`
/// if external directories are missing or need refreshing.
pub fn ensure_runtime_assets(base_dir: &Path) -> Result<(PathBuf, PathBuf), String> {
    let runtime_dir = base_dir.join(".runtime");
    let scripts_dir = runtime_dir.join("scripts");
    let patches_dir = runtime_dir.join("patches");

    fs::create_dir_all(&scripts_dir)
        .map_err(|e| format!("Failed to create runtime scripts dir: {}", e))?;
    fs::create_dir_all(&patches_dir)
        .map_err(|e| format!("Failed to create runtime patches dir: {}", e))?;

    let scripts: &[(&str, &str)] = &[
        ("prepare.py", SCRIPT_PREPARE),
        ("link_libc.py", SCRIPT_LINK_LIBC),
        ("link_modules.py", SCRIPT_LINK_MODULES),
        ("content_profile.py", SCRIPT_CONTENT_PROFILE),
        ("launch_env.py", SCRIPT_LAUNCH_ENV),
        ("patches.py", SCRIPT_PATCHES),
        ("mods.py", SCRIPT_MODS),
        ("graphics_options.py", SCRIPT_GRAPHICS_OPTIONS),
        ("save_backup.py", SCRIPT_SAVE_BACKUP),
        ("auto_tune.py", SCRIPT_AUTO_TUNE),
        ("find_game.py", SCRIPT_FIND_GAME),
    ];

    for (name, content) in scripts {
        let dest = scripts_dir.join(name);
        let need_write = if let Ok(meta) = fs::metadata(&dest) {
            meta.len() != content.len() as u64
        } else {
            true
        };
        if need_write {
            let _ = fs::write(&dest, content);
        }
    }

    let patch_xml = patches_dir.join("God_of_War_III_Remastered.xml");
    let need_write_patch = if let Ok(meta) = fs::metadata(&patch_xml) {
        meta.len() != PATCH_GOW3_XML.len() as u64
    } else {
        true
    };
    if need_write_patch {
        let _ = fs::write(&patch_xml, PATCH_GOW3_XML);
    }

    Ok((scripts_dir, patches_dir))
}

/// Finds an available Python 3 interpreter (checking local runtime bundle first, then system PATH)
pub fn find_python(base_dir: &Path) -> Result<String, String> {
    // 1. Check local runtime python folder (.runtime/python/python.exe or bin/python3)
    let candidates = [
        base_dir.join(".runtime/python/python.exe"),
        base_dir.join(".runtime/python/python"),
        base_dir.join(".runtime/python/bin/python3"),
        base_dir.join(".runtime/python/bin/python"),
        base_dir.join("python/python.exe"),
    ];
    for p in &candidates {
        if p.is_file() {
            if let Ok(out) = Command::new(p).arg("--version").output() {
                if out.status.success() {
                    return Ok(p.to_string_lossy().to_string());
                }
            }
        }
    }

    // 2. Check system-wide Python
    for cmd in &["python3", "python", "py"] {
        if let Ok(out) = Command::new(cmd).arg("--version").output() {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let combined = format!("{} {}", stdout, stderr);
                if combined.contains("Python 3.") {
                    return Ok(cmd.to_string());
                }
            }
        }
    }

    Err("Python 3 bulunamadı. Lütfen taşınabilir Python paketini indirin veya sisteminize kurun.".to_string())
}

/// Automatically downloads and configures official portable Python 3 runtime into `.runtime/python`
pub fn download_portable_python(base_dir: &Path) -> Result<String, String> {
    let runtime_dir = base_dir.join(".runtime");
    let python_dir = runtime_dir.join("python");
    let _ = fs::create_dir_all(&python_dir);

    #[cfg(target_os = "windows")]
    {
        let ps_code = format!(
            r#"
            $ProgressPreference = 'SilentlyContinue';
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12;
            $url = 'https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip';
            $zip = Join-Path $env:TEMP 'gow3_python_embed.zip';
            $dest = '{}';
            if (Test-Path $zip) {{ Remove-Item $zip -Force }};
            Write-Host 'Downloading Python 3.11 embed package...';
            (New-Object System.Net.WebClient).DownloadFile($url, $zip);
            if (!(Test-Path $dest)) {{ New-Item -ItemType Directory -Path $dest -Force | Out-Null }};
            Write-Host 'Extracting archive...';
            Expand-Archive -Path $zip -DestinationPath $dest -Force;
            Remove-Item $zip -Force;
            "#,
            python_dir.to_string_lossy().replace('\'', "''")
        );

        let status = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_code])
            .status()
            .map_err(|e| format!("PowerShell ile Python indirme komutu çalıştırılamadı: {}", e))?;

        if !status.success() {
            return Err("PowerShell üzerinden Python 3 indirilemedi.".to_string());
        }

        // Configure python311._pth to enable import site and recognize scripts directory
        let pth_file = python_dir.join("python311._pth");
        let pth_content = "python311.zip\n.\n..\n..\\scripts\n..\\..\\scripts\nimport site\n";
        let _ = fs::write(pth_file, pth_content);
    }

    #[cfg(not(target_os = "windows"))]
    {
        // On Linux / macOS fallback: if curl and unzip exist, download embed (usable with wine or testing)
        let zip_path = runtime_dir.join("python_embed.zip");
        let curl_res = Command::new("curl")
            .args([
                "-sL",
                "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip",
                "-o",
                &zip_path.to_string_lossy(),
            ])
            .status();

        if let Ok(st) = curl_res {
            if st.success() {
                let _ = Command::new("unzip")
                    .args(["-o", "-q", &zip_path.to_string_lossy(), "-d", &python_dir.to_string_lossy()])
                    .status();
                let _ = fs::remove_file(&zip_path);

                let pth_file = python_dir.join("python311._pth");
                let pth_content = "python311.zip\n.\n..\nimport site\n";
                let _ = fs::write(pth_file, pth_content);
            }
        }
    }

    find_python(base_dir).map_err(|_| "Python 3 indirildi ancak doğrulanamadı.".to_string())
}

/// Ensures Python 3 is available, automatically downloading portable package if missing
pub fn ensure_python(base_dir: &Path) -> Result<String, String> {
    if let Ok(py) = find_python(base_dir) {
        return Ok(py);
    }
    download_portable_python(base_dir)
}

pub fn find_game_engine(base_dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let probe_candidates = [
        base_dir.join("bin/gow3-probe.exe"),
        base_dir.join("out/gow3-probe.exe"),
        base_dir.join("gow3-probe.exe"),
        base_dir.join("bin/gow3.exe"),
        base_dir.join("gow3.exe"),
        base_dir.join("God of War III.exe"),
    ];

    #[cfg(not(target_os = "windows"))]
    let probe_candidates = [
        base_dir.join("bin/gow3-probe"),
        base_dir.join("out/gow3-probe"),
        base_dir.join("gow3-probe"),
    ];

    probe_candidates.into_iter().find(|p| p.is_file())
}

#[allow(dead_code)]
pub fn download_windows_engine(base_dir: &Path) -> Result<String, String> {
    let dest_bin = base_dir.join("bin");
    let _ = fs::create_dir_all(&dest_bin);

    #[cfg(target_os = "windows")]
    {
        let ps_code = format!(
            r#"
            $ProgressPreference = 'SilentlyContinue';
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12;
            $url = 'https://github.com/FelipePrado0/gow3_pc/releases/download/windows-v2.4.1/gow3-windows.zip';
            $zip = Join-Path $env:TEMP 'gow3_win_download.zip';
            $tempDir = Join-Path $env:TEMP 'gow3_win_extracted';
            if (Test-Path $tempDir) {{ Remove-Item $tempDir -Recurse -Force }};
            if (Test-Path $zip) {{ Remove-Item $zip -Force }};
            Write-Host 'Downloading Windows engine package (~74MB)...';
            (New-Object System.Net.WebClient).DownloadFile($url, $zip);
            Write-Host 'Extracting Windows package...';
            Expand-Archive -Path $zip -DestinationPath $tempDir -Force;

            $probe = Get-ChildItem -Path $tempDir -Recurse -Filter "gow3-probe.exe" | Select-Object -First 1;
            if ($probe) {{
                $sourceBin = $probe.DirectoryName;
                $destBin = '{dest_bin}';
                if (!(Test-Path $destBin)) {{ New-Item -ItemType Directory -Path $destBin -Force | Out-Null }};
                Copy-Item "$sourceBin\*" -Destination $destBin -Recurse -Force;
            }} else {{
                throw "gow3-probe.exe extracted archive icinde bulunamadi!";
            }}

            $base = '{base_dir}';
            $scripts = Get-ChildItem -Path $tempDir -Recurse -Directory -Filter "scripts" | Select-Object -First 1;
            if ($scripts -and !(Test-Path (Join-Path $base 'scripts'))) {{
                Copy-Item $scripts.FullName -Destination $base -Recurse -Force;
            }}
            $patches = Get-ChildItem -Path $tempDir -Recurse -Directory -Filter "patches" | Select-Object -First 1;
            if ($patches -and !(Test-Path (Join-Path $base 'patches'))) {{
                Copy-Item $patches.FullName -Destination $base -Recurse -Force;
            }}

            Remove-Item $tempDir -Recurse -Force;
            Remove-Item $zip -Force;
            "#,
            dest_bin = dest_bin.to_string_lossy().replace('\'', "''"),
            base_dir = base_dir.to_string_lossy().replace('\'', "''")
        );

        let status = Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", &ps_code])
            .status()
            .map_err(|e| format!("PowerShell Windows motoru indirme komutu çalıştırılamadı: {}", e))?;

        if !status.success() {
            return Err("PowerShell üzerinden Windows motor paketi indirilemedi veya açılamadı.".to_string());
        }

        if find_game_engine(base_dir).is_some() {
            return Ok("Windows oyun motoru başarıyla kuruldu!".to_string());
        }

        Err("Windows oyun motoru ('bin/gow3-probe.exe') arşivden çıkarılamadı.".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Bu fonksiyon yalnızca Windows üzerinde çalışır.".to_string())
    }
}

#[allow(dead_code)]
pub fn download_linux_engine(base_dir: &Path) -> Result<String, String> {
    let dest_bin = base_dir.join("bin");
    let _ = fs::create_dir_all(&dest_bin);

    #[cfg(not(target_os = "windows"))]
    {
        let url = "https://github.com/FelipePrado0/gow3_pc/releases/download/windows-v2.4.1/gow3-x86_64.AppImage";
        let temp_dir = std::env::temp_dir().join("gow3_linux_extracted");
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);
        let appimage_path = temp_dir.join("gow3-x86_64.AppImage");

        let dl_status = Command::new("curl")
            .args(["-sL", url, "-o", &appimage_path.to_string_lossy()])
            .status();

        let success = match dl_status {
            Ok(s) if s.success() => true,
            _ => {
                Command::new("wget")
                    .args(["-q", url, "-O", &appimage_path.to_string_lossy()])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
            }
        };

        if !success {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err("Linux AppImage paketi indirilemedi (curl/wget başarısız oldu).".to_string());
        }

        let _ = Command::new("chmod")
            .args(["+x", &appimage_path.to_string_lossy()])
            .status();

        let ext_status = Command::new(&appimage_path)
            .arg("--appimage-extract")
            .current_dir(&temp_dir)
            .status();

        if let Ok(st) = ext_status {
            if st.success() {
                let squash_bin = temp_dir.join("squashfs-root/bin");
                if squash_bin.is_dir() {
                    let _ = Command::new("cp")
                        .args(["-r", &format!("{}/.", squash_bin.to_string_lossy()), &dest_bin.to_string_lossy()])
                        .status();
                    let _ = Command::new("chmod")
                        .args(["+x", &dest_bin.join("gow3-probe").to_string_lossy()])
                        .status();
                }
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);

        if find_game_engine(base_dir).is_some() {
            return Ok("Linux oyun motoru başarıyla kuruldu!".to_string());
        }

        Err("Linux oyun motoru paketten çıkarılamadı.".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        Err("Bu fonksiyon yalnızca Linux üzerinde çalışır.".to_string())
    }
}

pub fn download_engine(base_dir: &Path) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        download_windows_engine(base_dir)
    }
    #[cfg(not(target_os = "windows"))]
    {
        download_linux_engine(base_dir)
    }
}

pub fn ensure_game_engine(base_dir: &Path) -> Result<PathBuf, String> {
    if let Some(p) = find_game_engine(base_dir) {
        return Ok(p);
    }

    // Try automatic download if missing
    let _ = download_engine(base_dir);
    if let Some(p) = find_game_engine(base_dir) {
        return Ok(p);
    }

    #[cfg(target_os = "windows")]
    {
        Err("Windows için oyun motoru ('bin/gow3-probe.exe') bulunamadı! Lütfen arayüzdeki '📥 Motoru İndir' butonuna basarak indirin.".to_string())
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("Linux için oyun motoru ('bin/gow3-probe') bulunamadı! Lütfen arayüzdeki '📥 Motoru İndir' butonuna basarak indirin.".to_string())
    }
}

/// Force terminates any running game processes (probe / wine / gamemoderun) on the system.
pub fn kill_game_processes(pid: u32) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        if pid > 0 {
            let _ = Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .creation_flags(CREATE_NO_WINDOW)
                .output();
        }
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/IM", "gow3-probe.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        let _ = Command::new("taskkill")
            .args(["/F", "/T", "/IM", "gow3.exe"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let my_pid = std::process::id();
        if pid > 0 && pid != my_pid {
            let _ = Command::new("kill")
                .args(["-9", &pid.to_string()])
                .output();
            let _ = Command::new("pkill")
                .args(["-9", "-P", &pid.to_string()])
                .output();
        }
        let _ = Command::new("pkill")
            .args(["-9", "-x", "gow3-probe"])
            .output();
        let _ = Command::new("pkill")
            .args(["-9", "-f", "gow3-probe"])
            .output();
        let _ = Command::new("pkill")
            .args(["-9", "-f", "gow3-probe.exe"])
            .output();
    }
}

#[cfg(target_os = "windows")]
pub fn adjust_windows_windowed_mode(target_pid: u32) {
    std::thread::spawn(move || {
        use std::ffi::c_void;
        use std::time::{Duration, Instant};

        #[repr(C)]
        struct RECT {
            left: i32,
            top: i32,
            right: i32,
            bottom: i32,
        }

        struct EnumContext {
            pid: u32,
            hwnd: *mut c_void,
        }

        unsafe extern "system" {
            fn GetWindowThreadProcessId(hwnd: *mut c_void, lpdwProcessId: *mut u32) -> u32;
            fn EnumWindows(lpEnumFunc: unsafe extern "system" fn(*mut c_void, isize) -> i32, lParam: isize) -> i32;
            fn IsWindowVisible(hwnd: *mut c_void) -> i32;
            fn GetWindowLongPtrA(hWnd: *mut c_void, nIndex: i32) -> isize;
            fn SetWindowLongPtrA(hWnd: *mut c_void, nIndex: i32, dwNewLong: isize) -> isize;
            fn SetWindowPos(hWnd: *mut c_void, hWndInsertAfter: *mut c_void, X: i32, Y: i32, cx: i32, cy: i32, uFlags: u32) -> i32;
            fn ShowWindow(hWnd: *mut c_void, nCmdShow: i32) -> i32;
            fn SystemParametersInfoA(uiAction: u32, uiParam: u32, pvParam: *mut c_void, fWinIni: u32) -> i32;
            fn GetSystemMetrics(nIndex: i32) -> i32;
        }

        unsafe extern "system" fn enum_proc(hwnd: *mut c_void, lparam: isize) -> i32 {
            unsafe {
                let ctx = &mut *(lparam as *mut EnumContext);
                let mut proc_id = 0u32;
                GetWindowThreadProcessId(hwnd, &mut proc_id);
                if proc_id == ctx.pid && IsWindowVisible(hwnd) != 0 {
                    ctx.hwnd = hwnd;
                    return 0;
                }
            }
            1
        }

        let start = Instant::now();
        let mut target_hwnd: *mut c_void = std::ptr::null_mut();

        while start.elapsed() < Duration::from_secs(15) {
            let mut ctx = EnumContext {
                pid: target_pid,
                hwnd: std::ptr::null_mut(),
            };
            unsafe {
                EnumWindows(enum_proc, &mut ctx as *mut _ as isize);
            }
            if !ctx.hwnd.is_null() {
                target_hwnd = ctx.hwnd;
                break;
            }
            std::thread::sleep(Duration::from_millis(150));
        }

        if target_hwnd.is_null() {
            return;
        }

        std::thread::sleep(Duration::from_millis(600));

        unsafe {
            let mut work_area: RECT = std::mem::zeroed();
            const SPI_GETWORKAREA: u32 = 0x0030;
            let ok = SystemParametersInfoA(SPI_GETWORKAREA, 0, &mut work_area as *mut _ as *mut c_void, 0);

            let screen_w = if ok != 0 { work_area.right - work_area.left } else { GetSystemMetrics(0) };
            let screen_h = if ok != 0 { work_area.bottom - work_area.top } else { GetSystemMetrics(1) };

            let (win_w, win_h) = if screen_w >= 2560 && screen_h >= 1440 {
                (1920, 1080)
            } else if screen_w > 1680 && screen_h > 940 {
                (1600, 900)
            } else if screen_w > 1360 && screen_h > 760 {
                (1280, 720)
            } else {
                ((screen_w * 85 / 100).max(800), (screen_h * 85 / 100).max(600))
            };

            let pos_x = work_area.left + ((screen_w - win_w) / 2).max(10);
            let pos_y = work_area.top + ((screen_h - win_h) / 2).max(10);

            const GWL_STYLE: i32 = -16;
            const WS_OVERLAPPEDWINDOW: isize = 0x00CF0000;
            const WS_VISIBLE: isize = 0x10000000;
            let current_style = GetWindowLongPtrA(target_hwnd, GWL_STYLE);
            SetWindowLongPtrA(target_hwnd, GWL_STYLE, current_style | WS_OVERLAPPEDWINDOW | WS_VISIBLE);

            const SWP_FRAMECHANGED: u32 = 0x0020;
            const SWP_SHOWWINDOW: u32 = 0x0040;
            const SWP_NOZORDER: u32 = 0x0004;
            const SW_RESTORE: i32 = 9;

            ShowWindow(target_hwnd, SW_RESTORE);
            SetWindowPos(
                target_hwnd,
                std::ptr::null_mut(),
                pos_x,
                pos_y,
                win_w,
                win_h,
                SWP_NOZORDER | SWP_FRAMECHANGED | SWP_SHOWWINDOW,
            );
        }
    });
}

/// Executes the full God of War III preparation and probe execution natively from Rust.
pub fn launch_gow3(
    base_dir: &Path,
    config: &LauncherConfig,
    use_gamemode: bool,
    running_pid: Option<Arc<AtomicU32>>,
) -> Result<i32, String> {
    let game_dir = Path::new(&config.game_dir);
    if !game_dir.join("eboot.bin").is_file() {
        return Err("Oyun klasöründe 'eboot.bin' bulunamadı!".to_string());
    }

    let probe_bin = ensure_game_engine(base_dir)?;

    let out_dir = base_dir.join("out");
    fs::create_dir_all(&out_dir).map_err(|e| format!("'out' klasörü oluşturulamadı: {}", e))?;

    // Ensure assets exist (uses .runtime if scripts/ or patches/ are missing from disk)
    let (runtime_scripts, runtime_patches) = ensure_runtime_assets(base_dir)?;
    let scripts_path = if base_dir.join("scripts").is_dir() {
        base_dir.join("scripts")
    } else {
        runtime_scripts
    };
    let patches_path = if base_dir.join("patches").is_dir() {
        base_dir.join("patches")
    } else {
        runtime_patches
    };

    let log_path = base_dir.join("gow3_log.txt");
    let log_file = File::create(&log_path).map_err(|e| format!("Log dosyası oluşturulamadı: {}", e))?;
    let log_file_err = log_file
        .try_clone()
        .map_err(|e| format!("Log dosyası çoğaltılamadı: {}", e))?;

    let python = ensure_python(base_dir)?;

    // 1. Run mods.py
    let _ = Command::new(&python)
        .arg(scripts_path.join("mods.py"))
        .arg(game_dir)
        .arg("--out")
        .arg(&out_dir)
        .arg("--enabled")
        .arg("1")
        .env("PYTHONPATH", &scripts_path)
        .current_dir(base_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file_err.try_clone().unwrap()))
        .status();

    // 2. Run prepare.py (ELF parsing and analysis)
    let status = Command::new(&python)
        .arg(scripts_path.join("prepare.py"))
        .arg(game_dir)
        .arg("--out")
        .arg(&out_dir)
        .env("PYTHONPATH", &scripts_path)
        .current_dir(base_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file_err.try_clone().unwrap()))
        .status()
        .map_err(|e| format!("prepare.py çalıştırılamadı: {}", e))?;

    if !status.success() {
        return Err("Oyun ikili dosyaları hazırlanamadı (prepare.py başarısız oldu).".to_string());
    }

    // 3. Run link_libc.py
    let status = Command::new(&python)
        .arg(scripts_path.join("link_libc.py"))
        .arg(game_dir)
        .arg("--out")
        .arg(&out_dir)
        .env("PYTHONPATH", &scripts_path)
        .current_dir(base_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file_err.try_clone().unwrap()))
        .status()
        .map_err(|e| format!("link_libc.py çalıştırılamadı: {}", e))?;

    if !status.success() {
        return Err("Sistem kitaplığı bağlanamadı (link_libc.py başarısız oldu).".to_string());
    }

    // 4. Run link_modules.py
    let status = Command::new(&python)
        .arg(scripts_path.join("link_modules.py"))
        .arg(game_dir)
        .arg("--out")
        .arg(&out_dir)
        .env("PYTHONPATH", &scripts_path)
        .current_dir(base_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file_err.try_clone().unwrap()))
        .status()
        .map_err(|e| format!("link_modules.py çalıştırılamadı: {}", e))?;

    if !status.success() {
        return Err("Oyun modülleri bağlanamadı (link_modules.py başarısız oldu).".to_string());
    }

    // 5. Run content_profile.py
    let _ = Command::new(&python)
        .arg(scripts_path.join("content_profile.py"))
        .arg(game_dir)
        .arg("--out")
        .arg(&out_dir)
        .arg("--sku")
        .arg("full")
        .env("PYTHONPATH", &scripts_path)
        .current_dir(base_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file_err.try_clone().unwrap()))
        .status();

    // 6. Run launch_env.py to update graphics state
    let config_ini = base_dir.join("gow3.ini");
    let state_json = out_dir.join("graphics_state.json");
    let mut launch_env_vars = Vec::new();

    if let Ok(out) = Command::new(&python)
        .arg(scripts_path.join("launch_env.py"))
        .arg("--game")
        .arg(game_dir)
        .arg("--config")
        .arg(&config_ini)
        .arg("--state")
        .arg(&state_json)
        .env("PYTHONPATH", &scripts_path)
        .current_dir(base_dir)
        .output()
    {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(updates) = val.get("updates").and_then(|u| u.as_object()) {
                for (k, v) in updates {
                    if let Some(s) = v.as_str() {
                        launch_env_vars.push((k.clone(), s.to_string()));
                    }
                }
            }
        }
    }

    // 7. Run patches.py
    let mut patch_list = Vec::new();
    if config.patch_readbacks {
        patch_list.push("Bug Fix - Texture Corruption Fix".to_string());
    }
    patch_list.push("Bug Fix - Startup Black Screen".to_string());
    if config.skip_cinematics {
        patch_list.push("Skip Any Video With X Button".to_string());
    }
    if config.patch_delta_time {
        patch_list.push(format!("Frame Rate Patch - {} FPS", config.target_fps));
    }
    let patches_str = patch_list.join(";");

    let _ = Command::new(&python)
        .arg(scripts_path.join("patches.py"))
        .arg("--out")
        .arg(&out_dir)
        .arg("--extra")
        .arg(&patches_str)
        .arg("--game-dir")
        .arg(game_dir)
        .arg("--patches-dir")
        .arg(&patches_path)
        .env("PYTHONPATH", &scripts_path)
        .env("GOW3_FORCE_PATCHES", "1")
        .env("GOW3_PATCHES_ONLY", "1")
        .current_dir(base_dir)
        .stdout(Stdio::from(log_file.try_clone().unwrap()))
        .stderr(Stdio::from(log_file_err.try_clone().unwrap()))
        .status();

    // 8. Execute probe binary
    let boot_linked = out_dir.join("boot-linked.bin");
    let content_bin = out_dir.join("content.bin");
    let patches_bin = out_dir.join("patches.bin");
    let user_dir = base_dir.join("user");

    let mut cmd = if use_gamemode {
        let mut c = Command::new("gamemoderun");
        c.arg(&probe_bin);
        c
    } else {
        Command::new(&probe_bin)
    };

    cmd.arg(&boot_linked)
        .arg("--content-profile")
        .arg(&content_bin)
        .arg("--patches")
        .arg(&patches_bin)
        .arg("--app0")
        .arg(game_dir)
        .arg("--user")
        .arg(&user_dir)
        .arg("--timeout")
        .arg("0")
        .current_dir(base_dir);

    // Set base launch environment variables
    for (k, v) in launch_env_vars {
        cmd.env(k, v);
    }

    // Apply launcher optimization overrides
    cmd.env("GOW3_PREBUILT", "1");
    cmd.env("GOW3_FORCE_PATCHES", "1");
    cmd.env("GOW3_STREAM_MEMORY", "cached");
    cmd.env("GOW3_FULL_BARRIERS", "1");
    cmd.env("GOW3_READBACKS", if config.patch_readbacks { "1" } else { "0" });
    cmd.env("GOW3_READBACK_LINEAR", if config.patch_readbacks { "1" } else { "0" });
    cmd.env("GOW3_FORCE_READBACK", if config.patch_readbacks { "1" } else { "0" });
    cmd.env("GOW3_DEFERRED_READBACK", "0");
    cmd.env("GOW3_STALE_READBACK", "0");
    cmd.env("GOW3_INTERP_INT_FIX", "1");
    cmd.env("GOW3_DIRECT_MEMORY_ACCESS", "1");
    cmd.env("GOW3_PIPELINE_CACHE", "1");
    cmd.env("GOW3_PARALLEL_WARMUP", "1");
    cmd.env("GOW3_BACKGROUND_WARMUP", "1");
    cmd.env("GOW3_VK_RECORD_THREAD", "1");
    cmd.env("GOW3_PRESENT_THREAD", "1");
    cmd.env("GOW3_ASYNC_FENCES", "1");
    cmd.env("GOW3_ASYNC_SHADERS", "1");
    cmd.env("GOW3_FPS_LIMIT", config.target_fps.to_string());
    cmd.env("GOW3_GC_BUDGET_MB", config.vram_gc_budget_mb.to_string());
    cmd.env("GOW3_DMEM_POOL_MB", config.dmem_mb.to_string());
    cmd.env("GOW3_DMEM_MB", config.dmem_mb.to_string());
    cmd.env("GOW3_DEFAULT_ENGINE_FPS", config.target_fps.to_string());
    cmd.env("GOW3_VBLANK_HZ", config.target_fps.to_string());
    cmd.env("GOW3_PATCHES", &patches_str);
    cmd.env("GOW3_PATCHES_ONLY", "1");
    cmd.env("GOW3_CONFIG", &config_ini);
    cmd.env("GOW3_GAME_DIR", game_dir);
    cmd.env(
        "GOW3_FULLSCREEN",
        if config.display_mode != crate::config::DisplayMode::Windowed {
            "1"
        } else {
            "0"
        },
    );

    // Apply Controller and Keyboard mapping overrides
    let key_map = config.build_gow3_key_map();
    let pad_map = config.build_gow3_pad_map();
    cmd.env("GOW3_KEY_MAP", &key_map);
    cmd.env("GOW3_PAD_MAP", pad_map);
    cmd.env(
        "SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS",
        if config.background_controller_input {
            "1"
        } else {
            "0"
        },
    );
    if config.deadzone_left > 0 {
        let dz = (config.deadzone_left as f32 / 100.0 * 32767.0) as u32;
        cmd.env("SDL_LINUX_JOYSTICK_DEADZONE", dz.to_string());
    }

    // Prepend bin directory to PATH so Windows/Linux loaders find bundled libraries
    let bin_dir = base_dir.join("bin");
    if bin_dir.is_dir() {
        if let Some(existing_path) = std::env::var_os("PATH") {
            let mut paths = vec![bin_dir.clone()];
            paths.extend(std::env::split_paths(&existing_path));
            if let Ok(new_path) = std::env::join_paths(paths) {
                cmd.env("PATH", new_path);
            }
        } else {
            cmd.env("PATH", &bin_dir);
        }
    }

    // Pipe stdout & stderr to gow3_log.txt
    cmd.stdout(Stdio::from(log_file));
    cmd.stderr(Stdio::from(log_file_err));

    let mut child = cmd.spawn().map_err(|e| format!("Oyun başlatılamadı: {}", e))?;
    if let Some(ref pid_holder) = running_pid {
        pid_holder.store(child.id(), Ordering::SeqCst);
    }

    #[cfg(target_os = "windows")]
    if config.display_mode == crate::config::DisplayMode::Windowed {
        adjust_windows_windowed_mode(child.id());
    }

    let exit_status = child.wait().map_err(|e| format!("Oyun beklenirken hata oluştu: {}", e));

    if let Some(ref pid_holder) = running_pid {
        pid_holder.store(0, Ordering::SeqCst);
    }

    let status = exit_status?;
    Ok(status.code().unwrap_or(-1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_python() {
        let py = find_python(Path::new("."));
        assert!(py.is_ok(), "Python 3 should be detected on the system");
    }

    #[test]
    fn test_ensure_runtime_assets() {
        let temp_dir = std::env::temp_dir().join("gow3_test_runtime");
        let _ = fs::remove_dir_all(&temp_dir);

        let res = ensure_runtime_assets(&temp_dir);
        assert!(res.is_ok());

        let (scripts_dir, patches_dir) = res.unwrap();
        assert!(scripts_dir.join("prepare.py").is_file());
        assert!(scripts_dir.join("link_libc.py").is_file());
        assert!(scripts_dir.join("link_modules.py").is_file());
        assert!(scripts_dir.join("patches.py").is_file());
        assert!(patches_dir.join("God_of_War_III_Remastered.xml").is_file());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
