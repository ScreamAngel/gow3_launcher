#[cfg(not(target_os = "windows"))]
use std::fs;
use std::process::Command;
use crate::config::CachedHardware;

pub fn detect_hardware() -> CachedHardware {
    let (gpu_name, vram_mb) = detect_gpu();
    let ram_mb = detect_ram();
    let refresh_rate_hz = detect_refresh_rate();

    CachedHardware {
        gpu_name,
        vram_mb,
        ram_mb,
        refresh_rate_hz,
    }
}

pub fn is_gamemode_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg("gamemoderun")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

#[cfg(not(target_os = "windows"))]
fn detect_gpu() -> (String, u64) {
    // 1. Try nvidia-smi
    if let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = text.lines().next() {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim().to_string();
                    let vram = parts[1].trim().parse::<u64>().unwrap_or(4096);
                    return (name, vram);
                }
            }
        }
    }

    // 2. Try AMD / Intel sysfs
    if let Ok(entries) = glob_vram() {
        if let Some((name, vram)) = entries {
            return (name, vram);
        }
    }

    // 3. Fallback to lspci
    if let Ok(output) = Command::new("lspci").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("VGA compatible controller") || line.contains("3D controller") {
                    let parts: Vec<&str> = line.split(':').collect();
                    if parts.len() >= 3 {
                        return (parts[2].trim().to_string(), 4096);
                    }
                }
            }
        }
    }

    ("Bilinmeyen GPU (Genel)".to_string(), 4096)
}

#[cfg(not(target_os = "windows"))]
fn glob_vram() -> Result<Option<(String, u64)>, std::io::Error> {
    for card_idx in 0..8 {
        let path = format!("/sys/class/drm/card{}/device/mem_info_vram_total", card_idx);
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(bytes) = content.trim().parse::<u64>() {
                let vram_mb = bytes / (1024 * 1024);
                return Ok(Some(("AMD/Intel Discrete GPU".to_string(), vram_mb)));
            }
        }
    }
    Ok(None)
}

#[cfg(not(target_os = "windows"))]
fn detect_ram() -> u64 {
    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u64>() {
                        return kb / 1024;
                    }
                }
            }
        }
    }
    16384 // Fallback
}

#[cfg(not(target_os = "windows"))]
fn detect_refresh_rate() -> u32 {
    let mut rates = Vec::new();

    // 1. Try xrandr
    if let Ok(output) = Command::new("xrandr").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains('*') {
                    for part in line.split_whitespace() {
                        if part.contains('*') {
                            let clean = part.replace('*', "").replace('+', "");
                            if let Ok(r) = clean.parse::<f32>() {
                                rates.push(r.round() as u32);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Try hyprctl monitors
    if rates.is_empty() {
        if let Ok(output) = Command::new("hyprctl").args(["monitors"]).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.contains('@') && line.contains("Hz") {
                        if let Some(at_pos) = line.find('@') {
                            let sub = &line[at_pos + 1..];
                            if let Some(hz_pos) = sub.find("Hz") {
                                let num_str = sub[..hz_pos].trim();
                                if let Ok(r) = num_str.parse::<f32>() {
                                    rates.push(r.round() as u32);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Try kscreen-doctor -o
    if rates.is_empty() {
        if let Ok(output) = Command::new("kscreen-doctor").args(["-o"]).output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.contains('@') && line.contains("Hz") {
                        if let Some(at_pos) = line.find('@') {
                            let sub = &line[at_pos + 1..];
                            if let Some(hz_pos) = sub.find("Hz") {
                                let num_str = sub[..hz_pos].trim();
                                if let Ok(r) = num_str.parse::<f32>() {
                                    rates.push(r.round() as u32);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    rates.into_iter().max().unwrap_or(60)
}

#[cfg(target_os = "windows")]
fn detect_gpu() -> (String, u64) {
    // 1. Try nvidia-smi first (exact VRAM for NVIDIA GPUs, e.g. RTX 4070 Laptop 8GB)
    if let Ok(output) = Command::new("nvidia-smi")
        .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = text.lines().next() {
                let parts: Vec<&str> = line.split(',').collect();
                if parts.len() >= 2 {
                    let name = parts[0].trim().to_string();
                    let vram = parts[1].trim().parse::<u64>().unwrap_or(0);
                    if vram > 0 {
                        return (name, vram);
                    }
                }
            }
        }
    }

    // 2. Try Windows Registry for AMD / Intel / NVIDIA (64-bit QWORD qwMemorySize)
    let ps_script = r#"
        $g = Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\00*' -ErrorAction SilentlyContinue |
             Where-Object { $_.'HardwareInformation.qwMemorySize' -gt 0 } |
             Sort-Object 'HardwareInformation.qwMemorySize' -Descending |
             Select-Object -First 1;
        if ($g) {
            $name = if ($g.DriverDesc) { $g.DriverDesc } else { $g.'Device Description' };
            $vram = [int64]$g.'HardwareInformation.qwMemorySize';
            [PSCustomObject]@{ Name = $name; VRAM = $vram } | ConvertTo-Json -Compress
        }
    "#;
    if let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_script])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                let name = val["Name"].as_str().unwrap_or("Windows DirectX/Vulkan GPU").to_string();
                if let Some(vram_bytes) = val["VRAM"].as_u64() {
                    let vram_mb = vram_bytes / (1024 * 1024);
                    if vram_mb >= 1024 {
                        return (name, vram_mb);
                    }
                }
            }
        }
    }

    // 3. Fallback to Win32_VideoController
    if let Ok(output) = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_VideoController | Sort-Object AdapterRAM -Descending | Select-Object -First 1 -Property Name,AdapterRAM | ConvertTo-Json -Compress",
        ])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                let name = val["Name"].as_str().unwrap_or("Windows DirectX/Vulkan GPU").to_string();
                let vram_bytes = val["AdapterRAM"].as_u64().unwrap_or(4 * 1024 * 1024 * 1024);
                let vram_mb = vram_bytes / (1024 * 1024);
                return (name, vram_mb.max(4096));
            }
        }
    }

    ("Windows DirectX/Vulkan GPU".to_string(), 4096)
}

#[cfg(target_os = "windows")]
fn detect_ram() -> u64 {
    if let Ok(output) = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_OperatingSystem).TotalVisibleMemorySize",
        ])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Ok(kb) = text.trim().parse::<u64>() {
                return kb / 1024;
            }
        }
    }
    16384
}

#[cfg(target_os = "windows")]
fn detect_refresh_rate() -> u32 {
    if let Ok(output) = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_VideoController | Select-Object -First 1).CurrentRefreshRate",
        ])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Ok(hz) = text.trim().parse::<u32>() {
                if hz > 0 {
                    return hz;
                }
            }
        }
    }
    60
}
