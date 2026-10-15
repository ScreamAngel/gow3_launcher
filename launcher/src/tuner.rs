use std::fs;
use std::path::Path;

pub fn apply_settings(
    base_dir: &Path,
    target_hz: u32,
    patch_delta_time: bool,
    patch_readbacks: bool,
    vram_gc_budget_mb: u64,
    dmem_mb: u64,
) -> Result<(), String> {
    if patch_delta_time {
        update_xml_patches(base_dir, target_hz)?;
    }
    update_run_sh(base_dir, target_hz, patch_readbacks, vram_gc_budget_mb, dmem_mb)?;
    update_gow3_ini(base_dir, patch_readbacks, vram_gc_budget_mb)?;
    update_user_config_toml(base_dir, vram_gc_budget_mb)?;

    Ok(())
}

fn update_xml_patches(base_dir: &Path, target_hz: u32) -> Result<(), String> {
    let dt = 1.0f32 / (target_hz as f32);
    let bytes = dt.to_le_bytes();
    let hex_val = format!("{:02x}{:02x}{:02x}{:02x}", bytes[0], bytes[1], bytes[2], bytes[3]);

    let patch_files = [
        base_dir.join(".runtime/patches/God_of_War_III_Remastered.xml"),
        base_dir.join("patches/God_of_War_III_Remastered.xml"),
        base_dir.join("CUSA01715-patch/patches/patch.xml"),
        base_dir.join("patches/CUSA01715.xml"),
        base_dir.join("patches/CUSA01623.xml"),
    ];

    for path in &patch_files {
        if path.is_file() {
            if let Ok(content) = fs::read_to_string(path) {
                // Update lines with Delta-Time or 120/144 FPS
                let mut new_lines = Vec::new();
                for line in content.lines() {
                    if (line.contains("Delta-Time") || line.contains("120 FPS") || line.contains("144 FPS") || line.contains("60 FPS"))
                        && line.contains("value=\"")
                    {
                        if let Some(start) = line.find("value=\"") {
                            let prefix = &line[..start + 7];
                            if let Some(end) = line[start + 7..].find('\"') {
                                let suffix = &line[start + 7 + end..];
                                new_lines.push(format!("{}{}{}", prefix, hex_val, suffix));
                                continue;
                            }
                        }
                    }
                    new_lines.push(line.to_string());
                }
                let _ = fs::write(path, new_lines.join("\n"));
            }
        }
    }

    Ok(())
}

fn update_run_sh(
    base_dir: &Path,
    target_hz: u32,
    patch_readbacks: bool,
    vram_gc_budget_mb: u64,
    dmem_mb: u64,
) -> Result<(), String> {
    let run_sh = base_dir.join("run.sh");
    if !run_sh.is_file() {
        return Ok(());
    }

    let mut content = fs::read_to_string(&run_sh).map_err(|e| e.to_string())?;

    let patches_str = if target_hz != 60 {
        format!("Bug Fix - Texture Corruption Fix;Skip Any Video With X Button;Frame Rate Patch - {} FPS", target_hz)
    } else {
        "Bug Fix - Texture Corruption Fix;Skip Any Video With X Button".to_string()
    };

    // Update GOW3_GC_BUDGET_MB
    replace_or_insert_env(&mut content, "GOW3_GC_BUDGET_MB", &vram_gc_budget_mb.to_string());
    // Update GOW3_DMEM_POOL_MB
    replace_or_insert_env(&mut content, "GOW3_DMEM_POOL_MB", &dmem_mb.to_string());
    // Update GOW3_DEFAULT_ENGINE_FPS
    replace_or_insert_env(&mut content, "GOW3_DEFAULT_ENGINE_FPS", &target_hz.to_string());
    // Update GOW3_FORCE_READBACK
    replace_or_insert_env(&mut content, "GOW3_FORCE_READBACK", if patch_readbacks { "1" } else { "0" });
    // Update GOW3_READBACKS
    replace_or_insert_env(&mut content, "GOW3_READBACKS", if patch_readbacks { "1" } else { "0" });
    // Update GOW3_READBACK_LINEAR
    replace_or_insert_env(&mut content, "GOW3_READBACK_LINEAR", if patch_readbacks { "1" } else { "0" });
    // Update GOW3_FULL_BARRIERS
    replace_or_insert_env(&mut content, "GOW3_FULL_BARRIERS", "1");
    // Update GOW3_FORCE_PATCHES
    replace_or_insert_env(&mut content, "GOW3_FORCE_PATCHES", "1");
    // Update GOW3_PREBUILT
    replace_or_insert_env(&mut content, "GOW3_PREBUILT", "1");
    // Update GOW3_STREAM_MEMORY
    replace_or_insert_env(&mut content, "GOW3_STREAM_MEMORY", "cached");
    // Update GOW3_PATCHES
    replace_or_insert_env(&mut content, "GOW3_PATCHES", &patches_str);
    // Update GOW3_PATCHES_ONLY
    replace_or_insert_env(&mut content, "GOW3_PATCHES_ONLY", "1");
    // Update GOW3_DMEM_MB
    replace_or_insert_env(&mut content, "GOW3_DMEM_MB", &dmem_mb.to_string());
    // Update GOW3_VBLANK_HZ
    replace_or_insert_env(&mut content, "GOW3_VBLANK_HZ", &target_hz.to_string());

    fs::write(&run_sh, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_or_insert_env(content: &mut String, var_name: &str, value: &str) {
    let pattern_quote = format!("export {}=\"", var_name);
    let pattern_noquote = format!("export {}=", var_name);

    let match_idx = content.find(&pattern_quote).or_else(|| content.find(&pattern_noquote));

    if let Some(idx) = match_idx {
        if let Some(end) = content[idx..].find('\n') {
            let prefix = &content[..idx];
            let suffix = &content[idx + end..];
            *content = format!("{}export {}=\"{}\"{}", prefix, var_name, value, suffix);
            return;
        }
    }

    // Insert before the last launch command if not present
    let insertion = format!("export {}=\"{}\"\n", var_name, value);
    if let Some(idx) = content.find("exec ") {
        content.insert_str(idx, &insertion);
    } else {
        content.push_str(&insertion);
    }
}

fn update_gow3_ini(base_dir: &Path, patch_readbacks: bool, vram_gc_budget_mb: u64) -> Result<(), String> {
    let ini_path = base_dir.join("gow3.ini");
    if !ini_path.is_file() {
        return Ok(());
    }

    let mut content = fs::read_to_string(&ini_path).map_err(|e| e.to_string())?;

    let readbacks_val = if patch_readbacks { "true" } else { "false" };
    replace_ini_key(&mut content, "readbacks", readbacks_val);
    replace_ini_key(&mut content, "readbackLinearImages", readbacks_val);
    replace_ini_key(&mut content, "stale_readback", "0");
    replace_ini_key(&mut content, "budget", &vram_gc_budget_mb.to_string());

    fs::write(&ini_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_ini_key(content: &mut String, key: &str, val: &str) {
    let mut lines = Vec::new();
    let mut found = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) && trimmed.contains('=') {
            lines.push(format!("{} = {}", key, val));
            found = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if !found {
        lines.push(format!("{} = {}", key, val));
    }

    *content = lines.join("\n");
}

fn update_user_config_toml(base_dir: &Path, vram_gc_budget_mb: u64) -> Result<(), String> {
    let toml_path = base_dir.join("user/config.toml");
    if !toml_path.is_file() {
        return Ok(());
    }

    let mut content = fs::read_to_string(&toml_path).map_err(|e| e.to_string())?;
    if let Some(idx) = content.find("vulkan_gc_budget =") {
        if let Some(end) = content[idx..].find('\n') {
            let prefix = &content[..idx];
            let suffix = &content[idx + end..];
            content = format!("{}vulkan_gc_budget = {}{}", prefix, vram_gc_budget_mb, suffix);
        }
    }

    fs::write(&toml_path, content).map_err(|e| e.to_string())?;
    Ok(())
}
