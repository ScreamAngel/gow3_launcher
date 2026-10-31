#[cfg(not(target_os = "windows"))]
use std::fs;
use std::process::Command;
use crate::config::CachedHardware;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuGrade {
    Weak,      // Zayıf / Darboğaz Riski (Kırmızı)
    Medium,    // Orta Seviye (40-60 FPS) (Sarı/Amber)
    Good,      // Güçlü (60 FPS İdeal) (Açık Yeşil)
    Ultra,     // Üst Seviye (60-144 FPS) (Parlak Yeşil)
}

impl CpuGrade {
    pub fn badge_text(&self, lang: &str) -> &'static str {
        match self {
            CpuGrade::Weak => match lang {
                "tr" => "• Zayıf / Darboğaz Riski",
                "de" => "• Schwach / Bottleneck-Risiko",
                "es" => "• Débil / Riesgo Cuello de Botella",
                _ => "• Weak / Bottleneck Risk",
            },
            CpuGrade::Medium => match lang {
                "tr" => "• Orta Seviye (40-60 FPS)",
                "de" => "• Mittelstufe (40-60 FPS)",
                "es" => "• Nivel Medio (40-60 FPS)",
                _ => "• Mid-Range (40-60 FPS)",
            },
            CpuGrade::Good => match lang {
                "tr" => "• Güçlü (60 FPS İdeal)",
                "de" => "• Stark (60 FPS Ideal)",
                "es" => "• Fuerte (Ideal 60 FPS)",
                _ => "• Strong (60 FPS Ideal)",
            },
            CpuGrade::Ultra => match lang {
                "tr" => "• Üst Seviye (Kusursuz)",
                "de" => "• Spitzenklasse (Perfekt)",
                "es" => "• Gama Alta (Perfecto)",
                _ => "• High-End (Flawless)",
            },
        }
    }

    pub fn rgb(&self) -> (u8, u8, u8) {
        match self {
            CpuGrade::Weak => (239, 83, 80),
            CpuGrade::Medium => (255, 213, 79),
            CpuGrade::Good => (130, 225, 140),
            CpuGrade::Ultra => (0, 230, 118),
        }
    }
}

pub fn evaluate_cpu(name: &str, cores: usize, threads: usize) -> CpuGrade {
    let lower = name.to_lowercase();

    // 1. Obsolete or severely underpowered architectures
    if lower.contains("celeron")
        || lower.contains("pentium")
        || lower.contains("core 2")
        || lower.contains("core(tm)2")
        || lower.contains("athlon")
        || lower.contains("sempron")
        || lower.contains("phenom")
        || lower.contains("fx-")
        || lower.contains("a10-")
        || lower.contains("a8-")
        || lower.contains("a6-")
        || lower.contains("a4-")
    {
        return CpuGrade::Weak;
    }

    // 2. Thread count constraints
    if threads <= 4 {
        // Any 2-core or 4-core/4-thread CPU (e.g. i5-4460, i5-6500, older i3) struggles with PS4 emulation threads
        return CpuGrade::Weak;
    }

    // 3. Medium tier (4 cores / 8 threads or 6 cores / 6 threads)
    // Or older 4-core HEDT / 6-core Zen 1
    if threads <= 8 || cores <= 4 {
        return CpuGrade::Medium;
    }

    // 4. Good vs Ultra
    if threads >= 16 || cores >= 8 {
        CpuGrade::Ultra
    } else if threads >= 12 || cores >= 6 {
        CpuGrade::Good
    } else {
        CpuGrade::Medium
    }
}

pub fn clean_cpu_name(name: &str) -> String {
    let mut cleaned = name
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace(" CPU", "");

    if let Some(pos) = cleaned.find("-Core") {
        if let Some(start) = cleaned[..pos].rfind(' ') {
            cleaned.drain(start..);
        }
    }

    let parts: Vec<&str> = cleaned.split_whitespace().collect();
    parts.join(" ")
}

pub fn detect_hardware() -> CachedHardware {
    let (gpu_name, vram_mb) = detect_gpu();
    let ram_mb = detect_ram();
    let refresh_rate_hz = detect_refresh_rate();
    let (cpu_name, cpu_cores, cpu_threads) = detect_cpu();

    CachedHardware {
        gpu_name,
        vram_mb,
        ram_mb,
        refresh_rate_hz,
        cpu_name,
        cpu_cores,
        cpu_threads,
    }
}

pub fn detect_cpu_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
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
pub fn detect_has_discrete_nvidia() -> bool {
    if let Ok(output) = Command::new("lspci").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if (line.contains("VGA compatible controller") || line.contains("3D controller"))
                    && line.to_lowercase().contains("nvidia")
                {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
pub fn detect_has_discrete_amd() -> bool {
    if let Ok(output) = Command::new("lspci").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut has_intel = false;
            let mut has_amd = false;
            for line in text.lines() {
                if line.contains("VGA compatible controller") || line.contains("3D controller") {
                    let lower = line.to_lowercase();
                    if lower.contains("intel") {
                        has_intel = true;
                    }
                    if lower.contains("amd") || lower.contains("advanced micro devices") || lower.contains("ati") {
                        has_amd = true;
                    }
                }
            }
            return has_intel && has_amd;
        }
    }
    false
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

#[cfg(not(target_os = "windows"))]
pub fn detect_cpu() -> (String, usize, usize) {
    let threads = detect_cpu_threads();
    let mut name = String::new();
    let mut cores = 0;

    if let Ok(content) = fs::read_to_string("/proc/cpuinfo") {
        for line in content.lines() {
            if name.is_empty() && (line.starts_with("model name") || line.starts_with("Hardware")) {
                if let Some((_, n)) = line.split_once(':') {
                    name = n.trim().to_string();
                }
            }
            if cores == 0 && line.starts_with("cpu cores") {
                if let Some((_, c)) = line.split_once(':') {
                    if let Ok(val) = c.trim().parse::<usize>() {
                        cores = val;
                    }
                }
            }
            if !name.is_empty() && cores > 0 {
                break;
            }
        }
    }

    if name.is_empty() {
        if let Ok(output) = Command::new("lscpu").output() {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                for line in text.lines() {
                    if line.starts_with("Model name:") {
                        if let Some((_, n)) = line.split_once(':') {
                            name = n.trim().to_string();
                            break;
                        }
                    }
                }
            }
        }
    }

    if name.is_empty() {
        name = "Linux x86_64 İşlemci".to_string();
    }

    if cores == 0 {
        cores = if threads > 1 { threads / 2 } else { 1 };
    }

    (name, cores, threads)
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

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn detect_has_discrete_nvidia() -> bool {
    false
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn detect_has_discrete_amd() -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn detect_cpu() -> (String, usize, usize) {
    let threads = detect_cpu_threads();
    let mut name = "Windows x86_64 İşlemci".to_string();
    let mut cores = if threads > 1 { threads / 2 } else { 1 };

    let ps_cmd = "Get-CimInstance Win32_Processor | Select-Object -First 1 -Property Name,NumberOfCores,NumberOfLogicalProcessors | ConvertTo-Json -Compress";
    if let Ok(output) = Command::new("powershell")
        .args(["-NoProfile", "-Command", ps_cmd])
        .output()
    {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                if let Some(n) = val["Name"].as_str() {
                    name = n.trim().to_string();
                }
                if let Some(c) = val["NumberOfCores"].as_u64() {
                    cores = c as usize;
                }
                let detected_threads = val["NumberOfLogicalProcessors"].as_u64().unwrap_or(threads as u64) as usize;
                return (name, cores, detected_threads.max(threads));
            }
        }
    }

    (name, cores, threads)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_cpu_name() {
        assert_eq!(
            clean_cpu_name("12th Gen Intel(R) Core(TM) i7-12700H"),
            "12th Gen Intel Core i7-12700H"
        );
        assert_eq!(
            clean_cpu_name("AMD Ryzen 7 5800X 8-Core Processor"),
            "AMD Ryzen 7 5800X"
        );
        assert_eq!(
            clean_cpu_name("Intel(R) Core(TM) i5-4460  CPU @ 3.20GHz"),
            "Intel Core i5-4460 @ 3.20GHz"
        );
    }

    #[test]
    fn test_evaluate_cpu() {
        // Low-end / Bottleneck CPUs
        assert_eq!(evaluate_cpu("Intel Core i5-4460", 4, 4), CpuGrade::Weak);
        assert_eq!(evaluate_cpu("Intel Core i3-4130", 2, 4), CpuGrade::Weak);
        assert_eq!(evaluate_cpu("AMD FX-8350 Eight-Core", 8, 8), CpuGrade::Weak);
        assert_eq!(evaluate_cpu("Intel Pentium G4560", 2, 4), CpuGrade::Weak);

        // Medium tier CPUs
        assert_eq!(evaluate_cpu("Intel Core i7-4790K", 4, 8), CpuGrade::Medium);
        assert_eq!(evaluate_cpu("AMD Ryzen 5 1600", 6, 6), CpuGrade::Medium);

        // Good tier CPUs
        assert_eq!(evaluate_cpu("AMD Ryzen 5 5600", 6, 12), CpuGrade::Good);
        assert_eq!(evaluate_cpu("Intel Core i5-12400", 6, 12), CpuGrade::Good);

        // Ultra tier CPUs
        assert_eq!(evaluate_cpu("12th Gen Intel Core i7-12700H", 14, 20), CpuGrade::Ultra);
        assert_eq!(evaluate_cpu("AMD Ryzen 7 7800X3D", 8, 16), CpuGrade::Ultra);
    }

    #[test]
    fn test_detect_cpu_host() {
        let (name, cores, threads) = detect_cpu();
        assert!(!name.is_empty());
        assert!(cores > 0);
        assert!(threads > 0);
        println!("Detected CPU: {} ({}C / {}T)", name, cores, threads);
    }
}
