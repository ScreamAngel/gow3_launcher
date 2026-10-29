use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct CrashDiagnosis {
    pub title: String,
    pub description: String,
    pub recommendation: String,
    pub log_snippet: String,
}

pub fn analyze_post_game(base_dir: &Path, exit_code: Option<i32>) -> Option<CrashDiagnosis> {
    // If exit code is 0 (normal exit), check if there were severe texture warnings
    let log_path = base_dir.join("gow3_log.txt");
    let log_content = if log_path.is_file() {
        fs::read_to_string(&log_path).unwrap_or_default()
    } else {
        String::new()
    };

    let tail_lines: Vec<&str> = log_content.lines().rev().take(100).collect();
    let tail_str = tail_lines.iter().rev().cloned().collect::<Vec<&str>>().join("\n");

    // 1. Check for Broken Vulkan Layers (lsfg-vk / Lossless Scaling)
    if tail_str.contains("lsfg-vk") && (tail_str.contains("shader DLL does not exist") || tail_str.contains("Failed to create swapchain") || tail_str.contains("ErrorInitializationFailed")) {
        return Some(CrashDiagnosis {
            title: "💥 Hatalı Vulkan Katmanı Algılandı (lsfg-vk / Lossless Scaling)".to_string(),
            description: "Sisteminizde kurulu harici 'lsfg-vk' Vulkan katmanı, Lossless Scaling shader DLL dosyasını bulamadığı için ekran penceresini (swapchain) kilitledi ve oyunu çökertti.".to_string(),
            recommendation: "Launcher bu hatalı lsfg-vk katmanını otomatik engeller. Manuel olarak 'VK_LOADER_LAYERS_DISABLE=\"*lsfg*\"' ortam değişkenini kullanabilir veya '~/.config/lsfg-vk/conf.toml' içindeki Lossless.dll dosya yolunu düzeltebilirsiniz.".to_string(),
            log_snippet: extract_relevant_snippet(&tail_str, "lsfg-vk"),
        });
    }

    // 2. Check for Incomplete Haswell/Intel iGPU on hybrid laptops
    if tail_str.contains("Haswell Vulkan support is incomplete") || (tail_str.contains("MESA-INTEL") && tail_str.contains("DepthStencilAttachment")) {
        return Some(CrashDiagnosis {
            title: "⚠️ Uyumsuz Dahili Intel GPU (Haswell / Eski iGPU)".to_string(),
            description: "Oyun, modern Vulkan özelliklerini desteklemeyen eski Intel Haswell dahili ekran kartında çalıştırılmış. Harici 2GB ekran kartınız (NVIDIA/AMD) devreye girmemiş.".to_string(),
            recommendation: "Launcher harici GPU'yu otomatik seçmeye çalışır. Manuel başlatmak için NVIDIA'da 'prime-run ./gow3-launcher' veya '__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia ./gow3-launcher', AMD'de 'DRI_PRIME=1 ./gow3-launcher' komutunu kullanın.".to_string(),
            log_snippet: extract_relevant_snippet(&tail_str, "Haswell"),
        });
    }

    // 3. Check for Swapchain Creation Failure
    if tail_str.contains("Failed to create swapchain") || tail_str.contains("vk_swapchain.cpp") {
        return Some(CrashDiagnosis {
            title: "💥 Vulkan Swapchain (Görüntü Çıkışı) Başlatılamadı".to_string(),
            description: "Grafik kartı sürücüsü veya araya giren harici bir Vulkan katmanı oyunun pencere/ekran yüzeyini oluşturmasını engelledi.".to_string(),
            recommendation: "Harici Vulkan katmanlarını kapatın, GPU sürücülerinizi güncelleyin ve tam ekran modunu pencereli moda alarak tekrar deneyin.".to_string(),
            log_snippet: extract_relevant_snippet(&tail_str, "swapchain"),
        });
    }

    // 4. Check for Out of Memory
    if tail_str.contains("VK_ERROR_OUT_OF_DEVICE_MEMORY") || tail_str.contains("Out of memory") || tail_str.contains("dmem out of memory") {
        return Some(CrashDiagnosis {
            title: "💥 Bellek / VRAM Yetersizliği Algılandı".to_string(),
            description: "Ekran kartının grafik belleği (VRAM) veya misafir sistem belleği tükendiği için oyun çöktü.".to_string(),
            recommendation: "Launcher üzerinden 'Doku Havuzu (VRAM GC)' bütçesini 4096 MB seviyesine düşürün ve arka plandaki uygulamaları kapatın.".to_string(),
            log_snippet: extract_relevant_snippet(&tail_str, "OUT_OF_DEVICE_MEMORY"),
        });
    }

    // 2. Check for Vulkan Device Lost
    if tail_str.contains("VK_ERROR_DEVICE_LOST") {
        return Some(CrashDiagnosis {
            title: "💥 GPU Sürücüsü Yanıt Vermeyi Kesti (Device Lost)".to_string(),
            description: "Vulkan grafik katmanı ekran kartıyla bağlantısını kaybetti. Aşırı yüklenme veya sürücü kilitlenmesi meydana gelmiş olabilir.".to_string(),
            recommendation: "NVIDIA sürücülerinizin güncel olduğundan emin olun ve varsa GPU hız aşırtmasını (overclock) kapatın.".to_string(),
            log_snippet: extract_relevant_snippet(&tail_str, "DEVICE_LOST"),
        });
    }

    // 3. Check for Texture Readback Glitch
    if tail_str.contains("Readback failed") || tail_str.contains("stale_readback") || tail_str.contains("readbackLinearImages failed") {
        return Some(CrashDiagnosis {
            title: "⚠️ Doku (Texture) Okuma Hatası Algılandı".to_string(),
            description: "GPU'dan sistem belleğine doku transferi sırasında gecikme veya veri bozulması yaşandı (Altın sarısı doku hatası belirtisi).".to_string(),
            recommendation: "Launcher üzerinde 'Sarı Doku Hatası Önleme (Instant Readbacks)' seçeneğinin AÇIK olduğundan emin olun.".to_string(),
            log_snippet: extract_relevant_snippet(&tail_str, "readback"),
        });
    }

    // 4. Check for Assertion Failure or SIGSEGV
    if tail_str.contains("Assertion failed") || tail_str.contains("SIGSEGV") || tail_str.contains("panic") || (exit_code.is_some() && exit_code != Some(0)) {
        let code_str = exit_code.map(|c| c.to_string()).unwrap_or_else(|| "Bilinmeyen".to_string());
        return Some(CrashDiagnosis {
            title: format!("💥 Oyun Beklenmedik Şekilde Kapandı (Çıkış Kodu: {})", code_str),
            description: "Oyun motoru veya emülasyon katmanı beklenmedik bir bellek hatası (Segmentation Fault / Assertion) ile sonlandı.".to_string(),
            recommendation: "144 Hz yamasının ekranınızla tam uyumlu olduğundan emin olun veya shader önbelleğini sıfırlayıp tekrar deneyin.".to_string(),
            log_snippet: tail_lines.into_iter().rev().take(15).collect::<Vec<&str>>().join("\n"),
        });
    }

    None
}

fn extract_relevant_snippet(log: &str, keyword: &str) -> String {
    let mut snippet = Vec::new();
    let lines: Vec<&str> = log.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.to_lowercase().contains(&keyword.to_lowercase()) {
            let start = i.saturating_sub(2);
            let end = (i + 4).min(lines.len());
            for l in &lines[start..end] {
                snippet.push(*l);
            }
            break;
        }
    }
    if snippet.is_empty() {
        lines.iter().rev().take(8).cloned().collect::<Vec<&str>>().join("\n")
    } else {
        snippet.join("\n")
    }
}
