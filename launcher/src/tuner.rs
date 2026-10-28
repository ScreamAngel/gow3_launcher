use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{key_to_sdl_scancode, DisplayMode, LauncherConfig};

pub fn apply_settings(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    if config.patch_delta_time {
        update_xml_patches(base_dir, config.target_fps)?;
    }
    update_run_sh(base_dir, config)?;
    update_gow3_ini(base_dir, config)?;
    update_user_config_toml(base_dir, config)?;
    update_user_input_config(base_dir, config)?;
    let _ = patch_engine_analog_scancodes(base_dir, config);

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

    let mut patches = vec![
        "Bug Fix - Texture Corruption Fix".to_string(),
        "Bug Fix - Startup Black Screen".to_string(),
        "Skip Any Video With X Button".to_string(),
    ];
    if config.patch_delta_time {
        patches.push(format!("Frame Rate Patch - {} FPS", config.target_fps));
    }
    let patches_str = patches.join(";");

    replace_or_insert_env(&mut content, "GOW3_GC_BUDGET_MB", &config.vram_gc_budget_mb.to_string());
    replace_or_insert_env(&mut content, "GOW3_DMEM_POOL_MB", &config.dmem_mb.to_string());
    replace_or_insert_env(&mut content, "GOW3_DEFAULT_ENGINE_FPS", &config.target_fps.to_string());
    replace_or_insert_env(&mut content, "GOW3_FORCE_READBACK", if config.patch_readbacks { "1" } else { "0" });
    replace_or_insert_env(&mut content, "GOW3_READBACKS", if config.patch_readbacks { "1" } else { "0" });
    replace_or_insert_env(&mut content, "GOW3_READBACK_LINEAR", if config.patch_readbacks { "1" } else { "0" });
    replace_or_insert_env(&mut content, "GOW3_DEFERRED_READBACK", "0");
    replace_or_insert_env(&mut content, "GOW3_STALE_READBACK", "0");
    replace_or_insert_env(&mut content, "GOW3_INTERP_INT_FIX", "1");
    replace_or_insert_env(&mut content, "GOW3_DIRECT_MEMORY_ACCESS", "1");
    replace_or_insert_env(&mut content, "GOW3_PIPELINE_CACHE", "1");
    replace_or_insert_env(&mut content, "GOW3_PARALLEL_WARMUP", "1");
    replace_or_insert_env(&mut content, "GOW3_BACKGROUND_WARMUP", "1");
    replace_or_insert_env(&mut content, "GOW3_VK_RECORD_THREAD", "1");
    replace_or_insert_env(&mut content, "GOW3_PRESENT_THREAD", "1");
    replace_or_insert_env(&mut content, "GOW3_ASYNC_FENCES", "1");
    replace_or_insert_env(&mut content, "GOW3_ASYNC_SHADERS", "1");
    replace_or_insert_env(&mut content, "GOW3_FPS_LIMIT", &config.target_fps.to_string());
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
    replace_ini_key(&mut content, "deferred_readback", "0");
    replace_ini_key(&mut content, "stale_readback", "0");
    replace_ini_key(&mut content, "async_shaders", "1");
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
    update_toml_key_in_section(
        &mut content,
        "GPU",
        "vblankFrequency",
        &config.target_fps.to_string(),
    );
    update_toml_key_in_section(
        &mut content,
        "GPU",
        "directMemoryAccess",
        "true",
    );
    update_toml_key_in_section(
        &mut content,
        "GPU",
        "readbacks",
        if config.patch_readbacks { "true" } else { "false" },
    );
    update_toml_key_in_section(
        &mut content,
        "GPU",
        "readbackLinearImages",
        if config.patch_readbacks { "true" } else { "false" },
    );

    // Update Vulkan settings
    update_toml_key_in_section(
        &mut content,
        "Vulkan",
        "pipelineCacheEnable",
        "true",
    );
    update_toml_key_in_section(
        &mut content,
        "Vulkan",
        "pipelineCacheArchive",
        "true",
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

/// Patches the 8 hardcoded analog-stick scancodes in gow3-probe (Linux) and gow3-probe.exe (Windows).
///
/// The engine binary uses SDL_GetKeyboardState and checks 8 hardcoded 1-byte scancodes:
///   Left Stick:  +X (right), -X (left), +Y (down), -Y (up)
///   Right Stick: +X (right), -X (left), +Y (down), -Y (up)
///
/// These scancodes are found immediately after a known unique anchor byte sequence
/// that appears exactly once in each binary.
fn patch_engine_analog_scancodes(base_dir: &Path, config: &LauncherConfig) -> Result<(), String> {
    let b = &config.keyboard_bindings;

    // Convert user key strings to SDL scancodes
    let sc_lx_plus  = key_to_sdl_scancode(&b.axis_left_x_plus);   // Left  +X (move right → D default)
    let sc_lx_minus = key_to_sdl_scancode(&b.axis_left_x_minus);  // Left  -X (move left  → A default)
    let sc_ly_plus  = key_to_sdl_scancode(&b.axis_left_y_plus);   // Left  +Y (move down  → S default)
    let sc_ly_minus = key_to_sdl_scancode(&b.axis_left_y_minus);  // Left  -Y (move up    → W default)
    let sc_rx_plus  = key_to_sdl_scancode(&b.axis_right_x_plus);  // Right +X (cam right  → Right Arrow default)
    let sc_rx_minus = key_to_sdl_scancode(&b.axis_right_x_minus); // Right -X (cam left   → Left Arrow default)
    let sc_ry_plus  = key_to_sdl_scancode(&b.axis_right_y_plus);  // Right +Y (cam down   → Down Arrow default)
    let sc_ry_minus = key_to_sdl_scancode(&b.axis_right_y_minus); // Right -Y (cam up     → Up Arrow default)

    // ─── Linux binary ──────────────────────────────────────────────────────────
    // Anchor (unique prefix just before the stick scancode block):
    //   f6 c4 01 74 06 41 c6 44 24 08 ff  f6 c4 02 74 06 41 c6 44 24 09 ff
    // Layout after anchor (offsets relative to first byte after anchor):
    //   +0 : 0f b6 55 [LX+]   +4 : 0f b6 45 [LX-]
    //   +24: 0f b6 55 [LY+]   +28: 0f b6 45 [LY-]
    //   +48: 0f b6 55 [RX+]   +52: 0f b6 45 [RX-]
    //   +72: 0f b6 55 [RY+]   +76: 0f b6 45 [RY-]
    let mut probe_candidates: Vec<PathBuf> = vec![
        base_dir.join("bin/gow3-probe"),
        base_dir.join("out/gow3-probe"),
        base_dir.join("gow3-probe"),
    ];
    if !config.game_dir.is_empty() {
        let gd = Path::new(&config.game_dir);
        probe_candidates.push(gd.join("bin/gow3-probe"));
        probe_candidates.push(gd.join("out/gow3-probe"));
        probe_candidates.push(gd.join("gow3-probe"));
    }
    for probe_path in &probe_candidates {
        if probe_path.is_file() {
            patch_binary_linux(
                probe_path,
                sc_lx_plus, sc_lx_minus, sc_ly_plus, sc_ly_minus,
                sc_rx_plus, sc_rx_minus, sc_ry_plus, sc_ry_minus,
            );
            break;
        }
    }

    // ─── Windows binary ────────────────────────────────────────────────────────
    // Anchor:
    //   a9 00 01 00 00 74 04 c6 46 08 ff  a9 00 02 00 00 74 04 c6 46 09 ff
    // Layout after anchor:
    //   +0 : 0f b6 47 [LX-]   +4 : 0f b6 4f [LX+]
    //   +36: 0f b6 4f [LY+]   +40: 0f b6 47 [LY-]
    //   +72: 0f b6 4f [RX+]   +76: 0f b6 47 [RX-]
    //   +108:0f b6 4f [RY+]  +112: 0f b6 47 [RY-]
    let mut probe_exe_candidates: Vec<PathBuf> = vec![
        base_dir.join("bin/gow3-probe.exe"),
        base_dir.join("out/gow3-probe.exe"),
        base_dir.join("gow3-probe.exe"),
        base_dir.join("bin/gow3.exe"),
        base_dir.join("gow3.exe"),
        base_dir.join("God of War III.exe"),
    ];
    if !config.game_dir.is_empty() {
        let gd = Path::new(&config.game_dir);
        probe_exe_candidates.push(gd.join("bin/gow3-probe.exe"));
        probe_exe_candidates.push(gd.join("out/gow3-probe.exe"));
        probe_exe_candidates.push(gd.join("gow3-probe.exe"));
        probe_exe_candidates.push(gd.join("bin/gow3.exe"));
        probe_exe_candidates.push(gd.join("gow3.exe"));
        probe_exe_candidates.push(gd.join("God of War III.exe"));
    }
    for probe_path in &probe_exe_candidates {
        if probe_path.is_file() {
            patch_binary_windows(
                probe_path,
                sc_lx_plus, sc_lx_minus, sc_ly_plus, sc_ly_minus,
                sc_rx_plus, sc_rx_minus, sc_ry_plus, sc_ry_minus,
            );
            break;
        }
    }

    Ok(())
}

fn patch_binary_linux(
    path: &Path,
    sc_lx_plus: Option<u8>, sc_lx_minus: Option<u8>,
    sc_ly_plus: Option<u8>, sc_ly_minus: Option<u8>,
    sc_rx_plus: Option<u8>, sc_rx_minus: Option<u8>,
    sc_ry_plus: Option<u8>, sc_ry_minus: Option<u8>,
) {
    let anchor: &[u8] = &[
        0xf6, 0xc4, 0x01, 0x74, 0x06, 0x41, 0xc6, 0x44, 0x24, 0x08, 0xff,
        0xf6, 0xc4, 0x02, 0x74, 0x06, 0x41, 0xc6, 0x44, 0x24, 0x09, 0xff,
    ];
    let Ok(mut data) = fs::read(path) else { return };
    let Some(pos) = find_bytes(&data, anchor) else { return };
    let base = pos + anchor.len();

    // Verify expected opcodes at each offset before patching
    if !check_opcode(&data, base, 0, &[0x0f, 0xb6, 0x55]) { return }
    if !check_opcode(&data, base, 4, &[0x0f, 0xb6, 0x45]) { return }
    if !check_opcode(&data, base, 24, &[0x0f, 0xb6, 0x55]) { return }
    if !check_opcode(&data, base, 28, &[0x0f, 0xb6, 0x45]) { return }
    if !check_opcode(&data, base, 48, &[0x0f, 0xb6, 0x55]) { return }
    if !check_opcode(&data, base, 52, &[0x0f, 0xb6, 0x45]) { return }
    if !check_opcode(&data, base, 72, &[0x0f, 0xb6, 0x55]) { return }
    if !check_opcode(&data, base, 76, &[0x0f, 0xb6, 0x45]) { return }

    if let Some(v) = sc_lx_plus  { data[base + 3]  = v; }
    if let Some(v) = sc_lx_minus { data[base + 7]  = v; }
    if let Some(v) = sc_ly_plus  { data[base + 27] = v; }
    if let Some(v) = sc_ly_minus { data[base + 31] = v; }
    if let Some(v) = sc_rx_plus  { data[base + 51] = v; }
    if let Some(v) = sc_rx_minus { data[base + 55] = v; }
    if let Some(v) = sc_ry_plus  { data[base + 75] = v; }
    if let Some(v) = sc_ry_minus { data[base + 79] = v; }

    if let Err(_) = fs::write(path, &data) {
        let tmp = path.with_extension("tmp_probe_patch");
        if fs::write(&tmp, &data).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    }
    println!("[Input Patch] Analog stick keys patched in {:?}", path);
}

fn patch_binary_windows(
    path: &Path,
    sc_lx_plus: Option<u8>, sc_lx_minus: Option<u8>,
    sc_ly_plus: Option<u8>, sc_ly_minus: Option<u8>,
    sc_rx_plus: Option<u8>, sc_rx_minus: Option<u8>,
    sc_ry_plus: Option<u8>, sc_ry_minus: Option<u8>,
) {
    let anchor: &[u8] = &[
        0xa9, 0x00, 0x01, 0x00, 0x00, 0x74, 0x04, 0xc6, 0x46, 0x08, 0xff,
        0xa9, 0x00, 0x02, 0x00, 0x00, 0x74, 0x04, 0xc6, 0x46, 0x09, 0xff,
    ];
    let Ok(mut data) = fs::read(path) else { return };
    let Some(pos) = find_bytes(&data, anchor) else { return };
    let base = pos + anchor.len();

    // Verify expected opcodes at each offset before patching
    if !check_opcode(&data, base, 0,   &[0x0f, 0xb6, 0x47]) { return }
    if !check_opcode(&data, base, 4,   &[0x0f, 0xb6, 0x4f]) { return }
    if !check_opcode(&data, base, 36,  &[0x0f, 0xb6, 0x4f]) { return }
    if !check_opcode(&data, base, 40,  &[0x0f, 0xb6, 0x47]) { return }
    if !check_opcode(&data, base, 72,  &[0x0f, 0xb6, 0x4f]) { return }
    if !check_opcode(&data, base, 76,  &[0x0f, 0xb6, 0x47]) { return }
    if !check_opcode(&data, base, 108, &[0x0f, 0xb6, 0x4f]) { return }
    if !check_opcode(&data, base, 112, &[0x0f, 0xb6, 0x47]) { return }

    if let Some(v) = sc_lx_minus { data[base + 3]   = v; }
    if let Some(v) = sc_lx_plus  { data[base + 7]   = v; }
    if let Some(v) = sc_ly_plus  { data[base + 39]  = v; }
    if let Some(v) = sc_ly_minus { data[base + 43]  = v; }
    if let Some(v) = sc_rx_plus  { data[base + 75]  = v; }
    if let Some(v) = sc_rx_minus { data[base + 79]  = v; }
    if let Some(v) = sc_ry_plus  { data[base + 111] = v; }
    if let Some(v) = sc_ry_minus { data[base + 115] = v; }

    if let Err(_) = fs::write(path, &data) {
        let tmp = path.with_extension("tmp_probe_patch");
        if fs::write(&tmp, &data).is_ok() {
            let _ = fs::rename(&tmp, path);
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    }
    println!("[Input Patch] Analog stick keys patched in {:?}", path);
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn check_opcode(data: &[u8], base: usize, offset: usize, expected: &[u8]) -> bool {
    let start = base + offset;
    if start + expected.len() > data.len() {
        return false;
    }
    &data[start..start + expected.len()] == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_patch_analog_scancodes_synthetic() {
        // Construct a synthetic buffer with the Linux anchor and opcodes
        let anchor = [
            0xf6, 0xc4, 0x01, 0x74, 0x06, 0x41, 0xc6, 0x44, 0x24, 0x08, 0xff,
            0xf6, 0xc4, 0x02, 0x74, 0x06, 0x41, 0xc6, 0x44, 0x24, 0x09, 0xff,
        ];
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0x90; 16]); // nop padding
        buf.extend_from_slice(&anchor);
        let base = buf.len();

        // 8 instructions:
        // +0:  0f b6 55 07 (D)
        // +4:  0f b6 45 04 (A)
        // +8..+23: 16 dummy bytes
        // +24: 0f b6 55 16 (S)
        // +28: 0f b6 45 1a (W)
        // +32..+47: 16 dummy bytes
        // +48: 0f b6 55 4f (Right)
        // +52: 0f b6 45 50 (Left)
        // +56..+71: 16 dummy bytes
        // +72: 0f b6 55 51 (Down)
        // +76: 0f b6 45 52 (Up)
        buf.extend_from_slice(&[0x0f, 0xb6, 0x55, 0x07]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x45, 0x04]);
        buf.extend_from_slice(&[0x00; 16]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x55, 0x16]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x45, 0x1a]);
        buf.extend_from_slice(&[0x00; 16]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x55, 0x4f]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x45, 0x50]);
        buf.extend_from_slice(&[0x00; 16]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x55, 0x51]);
        buf.extend_from_slice(&[0x0f, 0xb6, 0x45, 0x52]);
        buf.extend_from_slice(&[0x90; 32]);

        let tmp_file = std::env::temp_dir().join("test_gow3_probe_synthetic");
        fs::write(&tmp_file, &buf).unwrap();

        // Test patching: Left stick unchanged, Right stick -> I(12), K(14), J(13), L(15)
        patch_binary_linux(
            &tmp_file,
            Some(7), Some(4), Some(22), Some(26),
            Some(15), // RX+ = L
            Some(13), // RX- = J
            Some(14), // RY+ = K
            Some(12), // RY- = I
        );

        let patched = fs::read(&tmp_file).unwrap();
        let _ = fs::remove_file(&tmp_file);

        assert_eq!(patched[base + 51], 15); // RX+ = L
        assert_eq!(patched[base + 55], 13); // RX- = J
        assert_eq!(patched[base + 75], 14); // RY+ = K
        assert_eq!(patched[base + 79], 12); // RY- = I
    }
}
