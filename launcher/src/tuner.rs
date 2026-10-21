use std::fs;
use std::path::Path;

use crate::config::{DisplayMode, LauncherConfig};

pub fn apply_settings(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    if config.patch_delta_time {
        update_xml_patches(base_dir, config.target_fps)?;
    }
    update_run_sh(base_dir, config)?;
    update_gow3_ini(base_dir, config)?;
    update_user_config_toml(base_dir, config)?;
    update_user_input_config(base_dir, config)?;

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
                let mut new_lines = Vec::new();
                for line in content.lines() {
                    if (line.contains("Delta-Time")
                        || line.contains("120 FPS")
                        || line.contains("144 FPS")
                        || line.contains("60 FPS"))
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

fn update_run_sh(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    let run_sh = base_dir.join("run.sh");
    if !run_sh.is_file() {
        return Ok(());
    }

    let mut content = fs::read_to_string(&run_sh).map_err(|e| e.to_string())?;

    let patches_str = if config.target_fps != 60 {
        format!(
            "Bug Fix - Texture Corruption Fix;Skip Any Video With X Button;Frame Rate Patch - {} FPS",
            config.target_fps
        )
    } else {
        "Bug Fix - Texture Corruption Fix;Skip Any Video With X Button".to_string()
    };

    replace_or_insert_env(&mut content, "GOW3_GC_BUDGET_MB", &config.vram_gc_budget_mb.to_string());
    replace_or_insert_env(&mut content, "GOW3_DMEM_POOL_MB", &config.dmem_mb.to_string());
    replace_or_insert_env(&mut content, "GOW3_DEFAULT_ENGINE_FPS", &config.target_fps.to_string());
    replace_or_insert_env(&mut content, "GOW3_FORCE_READBACK", if config.patch_readbacks { "1" } else { "0" });
    replace_or_insert_env(&mut content, "GOW3_READBACKS", if config.patch_readbacks { "1" } else { "0" });
    replace_or_insert_env(&mut content, "GOW3_READBACK_LINEAR", if config.patch_readbacks { "1" } else { "0" });
    replace_or_insert_env(&mut content, "GOW3_FULL_BARRIERS", "1");
    replace_or_insert_env(&mut content, "GOW3_FORCE_PATCHES", "1");
    replace_or_insert_env(&mut content, "GOW3_PREBUILT", "1");
    replace_or_insert_env(&mut content, "GOW3_STREAM_MEMORY", "cached");
    replace_or_insert_env(&mut content, "GOW3_PATCHES", &patches_str);
    replace_or_insert_env(&mut content, "GOW3_PATCHES_ONLY", "1");
    replace_or_insert_env(&mut content, "GOW3_DMEM_MB", &config.dmem_mb.to_string());
    replace_or_insert_env(&mut content, "GOW3_VBLANK_HZ", &config.target_fps.to_string());
    replace_or_insert_env(
        &mut content,
        "GOW3_FULLSCREEN",
        if config.display_mode != DisplayMode::Windowed { "1" } else { "0" },
    );
    let key_map = config.build_gow3_key_map();
    let pad_map = config.build_gow3_pad_map();
    replace_or_insert_env(&mut content, "GOW3_KEY_MAP", &key_map);
    replace_or_insert_env(&mut content, "GOW3_PAD_MAP", pad_map);
    replace_or_insert_env(
        &mut content,
        "SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS",
        if config.background_controller_input { "1" } else { "0" },
    );

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

    let insertion = format!("export {}=\"{}\"\n", var_name, value);
    if let Some(idx) = content.find("exec ") {
        content.insert_str(idx, &insertion);
    } else {
        content.push_str(&insertion);
    }
}

fn update_gow3_ini(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    let ini_path = base_dir.join("gow3.ini");
    let mut content = if ini_path.is_file() {
        fs::read_to_string(&ini_path).unwrap_or_default()
    } else {
        "# God of War III Remastered settings (auto-tuned)\n\
         render_resolution=native\n\
         engine_fps=60\n\
         fps_limit=60\n\
         upscaler=off\n\
         fsr1=0\n\
         rcas=0\n\
         display_mode=windowed\n\
         vsync=1\n\
         deferred_readback=0\n\
         stale_readback=0\n\
         async_shaders=1\n\
         frames_queued=1\n\
         readbacks=true\n\
         readbackLinearImages=true\n\
         budget=6144\n".to_string()
    };

    let readbacks_val = if config.patch_readbacks { "true" } else { "false" };
    replace_ini_key(&mut content, "readbacks", readbacks_val);
    replace_ini_key(&mut content, "readbackLinearImages", readbacks_val);
    replace_ini_key(&mut content, "stale_readback", "0");
    replace_ini_key(&mut content, "budget", &config.vram_gc_budget_mb.to_string());
    replace_ini_key(&mut content, "engine_fps", &config.target_fps.to_string());
    replace_ini_key(&mut content, "fps_limit", &config.target_fps.to_string());
    replace_ini_key(&mut content, "frames_queued", "1");
    replace_ini_key(&mut content, "vsync", if config.vsync { "1" } else { "0" });
    replace_ini_key(&mut content, "display_mode", config.display_mode.as_str());

    fs::write(&ini_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn replace_ini_key(content: &mut String, key: &str, val: &str) {
    let mut lines = Vec::new();
    let mut found = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some((k, _)) = trimmed.split_once('=') {
            if k.trim() == key {
                lines.push(format!("{}={}", key, val));
                found = true;
                continue;
            }
        }
        lines.push(line.to_string());
    }

    if !found {
        lines.push(format!("{}={}", key, val));
    }

    *content = lines.join("\n");
}

fn update_toml_key_in_section(content: &mut String, section: &str, key: &str, val: &str) {
    let section_header = format!("[{}]", section);
    let mut in_section = false;
    let mut key_replaced = false;
    let mut new_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_section && !key_replaced {
                new_lines.push(format!("{} = {}", key, val));
                key_replaced = true;
            }
            in_section = trimmed == section_header;
            new_lines.push(line.to_string());
            continue;
        }

        if in_section {
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim() == key {
                    new_lines.push(format!("{} = {}", key, val));
                    key_replaced = true;
                    continue;
                }
            }
        }

        new_lines.push(line.to_string());
    }

    if in_section && !key_replaced {
        new_lines.push(format!("{} = {}", key, val));
    } else if !key_replaced {
        new_lines.push(String::new());
        new_lines.push(section_header);
        new_lines.push(format!("{} = {}", key, val));
    }

    *content = new_lines.join("\n");
}

fn update_user_config_toml(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    let toml_path = base_dir.join("user/config.toml");
    let mut content = if toml_path.is_file() {
        fs::read_to_string(&toml_path).unwrap_or_default()
    } else {
        let user_dir = base_dir.join("user");
        let _ = fs::create_dir_all(&user_dir);
        "[General]\n\
         [GPU]\n\
         Fullscreen = false\n\
         FullscreenMode = \"Windowed\"\n\
         [Input]\n".to_string()
    };

    // Update vulkan_gc_budget if present
    if let Some(idx) = content.find("vulkan_gc_budget =") {
        if let Some(end) = content[idx..].find('\n') {
            let prefix = &content[..idx];
            let suffix = &content[idx + end..];
            content = format!("{}vulkan_gc_budget = {}{}", prefix, config.vram_gc_budget_mb, suffix);
        }
    }

    // Update Display / GPU settings
    let (fullscreen_mode_str, fullscreen_bool) = config.display_mode.to_toml_mode();
    update_toml_key_in_section(
        &mut content,
        "GPU",
        "Fullscreen",
        if fullscreen_bool { "true" } else { "false" },
    );
    update_toml_key_in_section(
        &mut content,
        "GPU",
        "FullscreenMode",
        &format!("\"{}\"", fullscreen_mode_str),
    );

    // Update Input settings
    update_toml_key_in_section(
        &mut content,
        "Input",
        "backgroundControllerInput",
        if config.background_controller_input { "true" } else { "false" },
    );
    update_toml_key_in_section(
        &mut content,
        "Input",
        "isMotionControlsEnabled",
        if config.is_motion_controls_enabled { "true" } else { "false" },
    );

    fs::write(&toml_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn update_user_input_config(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    let input_dir = base_dir.join("user/input_config");
    let _ = fs::create_dir_all(&input_dir);
    let ini_path = input_dir.join("default.ini");

    let b = &config.keyboard_bindings;
    let content = format!(
        "# God of War III Remastered - Input Configuration\n\
         # Auto-generated by Launcher\n\n\
         # Keyboard bindings\n\n\
         triangle = {triangle}\n\
         circle = {circle}\n\
         cross = {cross}\n\
         square = {square}\n\n\
         l1 = {l1}\n\
         r1 = {r1}\n\
         l2 = {l2}\n\
         r2 = {r2}\n\
         l3 = {l3}\n\
         r3 = {r3}\n\n\
         options = {options}\n\
         touchpad_center = {touchpad}\n\n\
         pad_up = {pad_up}\n\
         pad_down = {pad_down}\n\
         pad_left = {pad_left}\n\
         pad_right = {pad_right}\n\n\
         axis_left_x_minus = {ax_lx_min}\n\
         axis_left_x_plus = {ax_lx_plus}\n\
         axis_left_y_minus = {ax_ly_min}\n\
         axis_left_y_plus = {ax_ly_plus}\n\n\
         axis_right_x_minus = {ax_rx_min}\n\
         axis_right_x_plus = {ax_rx_plus}\n\
         axis_right_y_minus = {ax_ry_min}\n\
         axis_right_y_plus = {ax_ry_plus}\n\n\
         # Controller bindings\n\n\
         triangle = triangle\n\
         cross = cross\n\
         square = square\n\
         circle = circle\n\n\
         l1 = l1\n\
         l2 = l2\n\
         l3 = l3\n\
         r1 = r1\n\
         r2 = r2\n\
         r3 = r3\n\n\
         options = options\n\
         touchpad_center = back\n\n\
         pad_up = pad_up\n\
         pad_down = pad_down\n\
         pad_left = pad_left\n\
         pad_right = pad_right\n\n\
         axis_left_x = axis_left_x\n\
         axis_left_y = axis_left_y\n\
         axis_right_x = axis_right_x\n\
         axis_right_y = axis_right_y\n\n\
         # Range of deadzones: 1 (almost none) to 127 (max)\n\
         analog_deadzone = leftjoystick, {deadzone_l}, 127\n\
         analog_deadzone = rightjoystick, {deadzone_r}, 127\n\n\
         override_controller_color = false, 0, 0, 255\n",
        triangle = b.triangle,
        circle = b.circle,
        cross = b.cross,
        square = b.square,
        l1 = b.l1,
        r1 = b.r1,
        l2 = b.l2,
        r2 = b.r2,
        l3 = b.l3,
        r3 = b.r3,
        options = b.options,
        touchpad = b.touchpad_center,
        pad_up = b.pad_up,
        pad_down = b.pad_down,
        pad_left = b.pad_left,
        pad_right = b.pad_right,
        ax_lx_min = b.axis_left_x_minus,
        ax_lx_plus = b.axis_left_x_plus,
        ax_ly_min = b.axis_left_y_minus,
        ax_ly_plus = b.axis_left_y_plus,
        ax_rx_min = b.axis_right_x_minus,
        ax_rx_plus = b.axis_right_x_plus,
        ax_ry_min = b.axis_right_y_minus,
        ax_ry_plus = b.axis_right_y_plus,
        deadzone_l = config.deadzone_left,
        deadzone_r = config.deadzone_right,
    );

    fs::write(&ini_path, content).map_err(|e| e.to_string())?;
    Ok(())
}
