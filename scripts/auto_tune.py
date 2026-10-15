#!/usr/bin/env python3
"""Auto-tuner for God of War III Linux port.

Detects hardware specs (VRAM, System RAM, Monitor Refresh Rate) and
automatically tunes run.sh, gow3.ini, user/config.toml, and XML patches.
"""
import argparse
import glob
import os
import re
import struct
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent


def detect_system():
    # 1. GPU & VRAM Detection
    gpu_name = "Bilinmeyen GPU"
    vram_mb = 4096

    # Try nvidia-smi
    try:
        out = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=name,memory.total", "--format=csv,noheader,nounits"],
            text=True,
            stderr=subprocess.DEVNULL,
        )
        lines = [ln.strip() for ln in out.splitlines() if ln.strip()]
        if lines:
            parts = lines[0].split(",")
            gpu_name = parts[0].strip()
            vram_mb = int(parts[1].strip())
    except Exception:
        # Try AMD / Intel sysfs
        for p in glob.glob("/sys/class/drm/card*/device/mem_info_vram_total"):
            try:
                bytes_val = int(open(p).read().strip())
                vram_mb = bytes_val // (1024 * 1024)
                gpu_name = "AMD/Intel Discrete GPU"
                break
            except Exception:
                pass

    # 2. Total System RAM
    ram_mb = 8192
    try:
        meminfo = open("/proc/meminfo").read()
        m = re.search(r"MemTotal:\s+(\d+)\s+kB", meminfo)
        if m:
            ram_mb = int(m.group(1)) // 1024
    except Exception:
        pass

    # 3. Monitor Refresh Rate (Hz)
    hz = 60
    # Try xrandr
    try:
        out = subprocess.check_output(["xrandr"], text=True, stderr=subprocess.DEVNULL)
        rates = [float(r) for r in re.findall(r"(\d+(?:\.\d+)?)\*", out)]
        if rates:
            hz = round(max(rates))
    except Exception:
        pass

    if hz == 60:
        # Try kscreen-doctor (KDE Wayland)
        try:
            out = subprocess.check_output(["kscreen-doctor", "-o"], text=True, stderr=subprocess.DEVNULL)
            rates = [float(r) for r in re.findall(r"@(\d+(?:\.\d+)?)Hz", out)]
            if rates:
                hz = round(max(rates))
        except Exception:
            pass

    if hz == 60:
        # Try wlr-randr (wlroots / Hyprland / Sway)
        try:
            out = subprocess.check_output(["wlr-randr"], text=True, stderr=subprocess.DEVNULL)
            rates = [float(r) for r in re.findall(r"(\d+(?:\.\d+)?)\s+Hz\s+\(current\)", out)]
            if rates:
                hz = round(max(rates))
        except Exception:
            pass

    if hz == 60:
        # Try hyprctl
        try:
            out = subprocess.check_output(["hyprctl", "monitors"], text=True, stderr=subprocess.DEVNULL)
            rates = [float(r) for r in re.findall(r"@\s*(\d+(?:\.\d+)?)\s*Hz", out)]
            if rates:
                hz = round(max(rates))
        except Exception:
            pass

    return {
        "gpu_name": gpu_name,
        "vram_mb": vram_mb,
        "ram_mb": ram_mb,
        "hz": hz,
    }


def calculate_settings(specs, override_hz=None):
    hz = override_hz if override_hz else specs["hz"]
    vram_mb = specs["vram_mb"]
    ram_mb = specs["ram_mb"]

    # Texture GC Budget (based on VRAM)
    if vram_mb >= 12000:
        gc_budget_mb = 8192
    elif vram_mb >= 7500:
        gc_budget_mb = 6144
    elif vram_mb >= 5500:
        gc_budget_mb = 4608
    elif vram_mb >= 3500:
        gc_budget_mb = 3072
    else:
        gc_budget_mb = 2048

    # Guest DMEM Pool (based on RAM)
    if ram_mb >= 24000:
        dmem_mb = 10240
    elif ram_mb >= 14000:
        dmem_mb = 9216
    elif ram_mb >= 11000:
        dmem_mb = 7168
    else:
        dmem_mb = 6144

    return {
        "target_hz": hz,
        "gc_budget_mb": gc_budget_mb,
        "dmem_mb": dmem_mb,
    }


def float_to_hex(val: float) -> str:
    return struct.pack("<f", float(val)).hex()


def update_xml_patches(target_hz: int):
    xml_path = BASE_DIR / "patches" / "God_of_War_III_Remastered.xml"
    if not xml_path.is_file():
        return False

    content = xml_path.read_text(encoding="utf-8")
    if target_hz != 60:
        patch_name = f"Frame Rate Patch - {target_hz} FPS"
        hz_hex = float_to_hex(target_hz)
        
        # Check if patch already exists in XML
        if f'Name="{patch_name}"' not in content:
            new_patch = f'''    <Metadata Title="God of War III Remastered" Name="{patch_name}" Note="Raises the engine maximum framerate from 59.94 to {target_hz} FPS so the frametime clamp produces a 1/{target_hz} second gameplay delta. Corrects swing energy calculations to match the original 60 Hz simulation. Requires setting VBlank to {target_hz} FPS." Author="auto_tune" PatchVer="1.1" AppVer="01.02" AppElf="eboot.bin" isEnabled="false">
        <PatchList>
            <Line Type="bytes" Address="0x0048ff0a" Value="488d05a7c12600"/>
            <Line Type="bytes" Address="0x0075dbc4" Value="{hz_hex}"/>
        </PatchList>
    </Metadata>
'''
            # Insert right before </Patch>
            if "</Patch>" in content:
                content = content.replace("</Patch>", new_patch + "</Patch>")
                xml_path.write_text(content, encoding="utf-8")
                return True
    return False


def update_graphics_options_py(target_hz: int):
    opts_path = BASE_DIR / "scripts" / "graphics_options.py"
    if not opts_path.is_file():
        return

    content = opts_path.read_text(encoding="utf-8")
    # Make sure target_hz is in ENGINE_FPS
    m = re.search(r"ENGINE_FPS\s*=\s*\((.*?)\)", content)
    if m:
        current_fps = [int(x.strip()) for x in m.group(1).split(",") if x.strip().isdigit()]
        if target_hz not in current_fps:
            current_fps.append(target_hz)
            current_fps = sorted(list(set(current_fps)))
            new_tuple = f"ENGINE_FPS = ({', '.join(str(x) for x in current_fps)})"
            content = content[:m.start()] + new_tuple + content[m.end():]
            opts_path.write_text(content, encoding="utf-8")


def update_run_sh(settings):
    run_sh = BASE_DIR / "run.sh"
    if not run_sh.is_file():
        return

    content = run_sh.read_text(encoding="utf-8")
    target_hz = settings["target_hz"]
    gc_budget = settings["gc_budget_mb"]
    dmem = settings["dmem_mb"]

    # Target patches string
    if target_hz != 60:
        patches_str = f"Bug Fix - Texture Corruption Fix;Skip Any Video With X Button;Frame Rate Patch - {target_hz} FPS"
    else:
        patches_str = "Bug Fix - Texture Corruption Fix;Skip Any Video With X Button"

    # Update or add GOW3_GC_BUDGET_MB near the top
    if re.search(r"^export GOW3_GC_BUDGET_MB=.*$", content, re.M):
        content = re.sub(r"^export GOW3_GC_BUDGET_MB=.*$", f"export GOW3_GC_BUDGET_MB={gc_budget}", content, flags=re.M)
    else:
        content = content.replace("export GOW3_FULL_BARRIERS=1", f"export GOW3_FULL_BARRIERS=1\nexport GOW3_GC_BUDGET_MB={gc_budget}")

    # Ensure stability exports exist
    for var, val in [("GOW3_READBACKS", "1"), ("GOW3_READBACK_LINEAR", "1")]:
        if not re.search(rf"^export {var}=.*$", content, re.M):
            content = content.replace("export GOW3_FULL_BARRIERS=1", f"export GOW3_FULL_BARRIERS=1\nexport {var}={val}")

    # Update GOW3_PATCHES
    content = re.sub(
        r'^export GOW3_PATCHES=.*$',
        f'export GOW3_PATCHES="{patches_str}"',
        content,
        flags=re.M,
    )

    # Ensure GOW3_PATCHES_ONLY=1
    content = re.sub(
        r'^(?:unset|export) GOW3_PATCHES_ONLY(?:=.*)?$',
        'export GOW3_PATCHES_ONLY=1',
        content,
        flags=re.M,
    )

    # Update GOW3_DMEM_MB
    content = re.sub(
        r'^export GOW3_DMEM_MB=\d+$',
        f'export GOW3_DMEM_MB={dmem}',
        content,
        flags=re.M,
    )
    content = re.sub(
        r'export GOW3_DMEM_MB=\$\{\s*GOW3_DMEM_MB:-0\s*\};\s*\(\(\s*GOW3_DMEM_MB\s*>=\s*\d+\s*\)\)\s*\|\|\s*export GOW3_DMEM_MB=\d+',
        f'export GOW3_DMEM_MB=${{GOW3_DMEM_MB:-0}}; (( GOW3_DMEM_MB >= {dmem} )) || export GOW3_DMEM_MB={dmem}',
        content,
    )

    # Update GOW3_VBLANK_HZ
    content = re.sub(
        r'^export GOW3_VBLANK_HZ=.*$',
        f'export GOW3_VBLANK_HZ={target_hz}',
        content,
        flags=re.M,
    )

    run_sh.write_text(content, encoding="utf-8")


def update_ini_files(settings):
    target_hz = settings["target_hz"]
    ini_content = f"""# God of War III Remastered settings (auto-tuned)
render_resolution=native
engine_fps={target_hz}
fps_limit={target_hz}
upscaler=off
fsr1=0
rcas=0
display_mode=borderless
vsync=1
deferred_readback=0
stale_readback=0
async_shaders=1
frames_queued=1
"""
    # 1. Local gow3.ini
    (BASE_DIR / "gow3.ini").write_text(ini_content, encoding="utf-8")

    # 2. ~/.local/share/gow3/gow3.ini
    user_share_ini = Path.home() / ".local" / "share" / "gow3" / "gow3.ini"
    if user_share_ini.parent.exists():
        user_share_ini.write_text(ini_content, encoding="utf-8")


def update_user_config_toml(settings):
    toml_path = BASE_DIR / "user" / "config.toml"
    if not toml_path.is_file():
        return

    content = toml_path.read_text(encoding="utf-8")
    target_hz = settings["target_hz"]

    content = re.sub(r"^vblankFrequency\s*=\s*\d+", f"vblankFrequency = {target_hz}", content, flags=re.M)
    content = re.sub(r"^readbacks\s*=\s*\w+", "readbacks = true", content, flags=re.M)
    content = re.sub(r"^readbackLinearImages\s*=\s*\w+", "readbackLinearImages = true", content, flags=re.M)
    content = re.sub(r"^copyGPUBuffers\s*=\s*\w+", "copyGPUBuffers = true", content, flags=re.M)

    toml_path.write_text(content, encoding="utf-8")


def update_graphics_state_json(settings):
    state_path = BASE_DIR / "out" / "graphics_state.json"
    if not state_path.is_file():
        return

    target_hz = settings["target_hz"]
    import json
    try:
        data = json.loads(state_path.read_text(encoding="utf-8"))
        if "last_good" in data:
            data["last_good"]["engine_fps"] = str(target_hz)
            data["last_good"]["deferred_readback"] = "0"
            data["last_good"]["stale_readback"] = "0"
        state_path.write_text(json.dumps(data, indent=2), encoding="utf-8")
    except Exception:
        pass


def ask_user_launch() -> bool:
    c_bold = "\033[1m"
    c_reset = "\033[0m"
    prompt_text = f"\n{c_bold}🎮 God of War III başlatılsın mı (run.sh çalıştırılsın mı)? [E/h]: {c_reset}"

    # 1. Direct TTY input (/dev/tty): Most reliable in Linux terminals, even if stdin is redirected
    try:
        with open("/dev/tty", "r") as tty_in:
            sys.stdout.write(prompt_text)
            sys.stdout.flush()
            line = tty_in.readline()
            if line is not None:
                ans = line.strip().lower()
                return ans in ("", "e", "evet", "y", "yes", "1")
    except Exception:
        pass

    # 2. Standard sys.stdin
    try:
        if sys.stdin and not sys.stdin.closed:
            ans = input(prompt_text).strip().lower()
            return ans in ("", "e", "evet", "y", "yes", "1")
    except (KeyboardInterrupt, EOFError, OSError):
        pass

    # 3. GUI fallback via zenity (if running without terminal from Dolphin/Thunar/Desktop)
    if os.environ.get("DISPLAY") or os.environ.get("WAYLAND_DISPLAY"):
        import shutil
        if shutil.which("zenity"):
            try:
                res = subprocess.run([
                    "zenity", "--question",
                    "--title=God of War III Remastered",
                    "--text=God of War III başlatılsın mı (run.sh çalıştırılsın mı)?",
                    "--ok-label=Evet, Başlat",
                    "--cancel-label=Hayır, Çık"
                ], timeout=120)
                return res.returncode == 0
            except Exception:
                pass

    return False


def main():
    parser = argparse.ArgumentParser(description="God of War III Linux Auto-Tuner")
    parser.add_argument("--hz", type=int, help="Manuel ekran yenileme hızı (Hz) belirle")
    parser.add_argument("--silent", action="store_true", help="Sessiz mod (bilgi mesajlarını gizle)")
    parser.add_argument("--dry-run", action="store_true", help="Dosyaları değiştirmeden ayarları göster")
    parser.add_argument("--no-launch-prompt", action="store_true", help="Oyun başlatma sorusunu sorma")
    parser.add_argument("--launch", action="store_true", help="Sorulmadan doğrudan oyunu başlat")
    parser.add_argument("--no-launch", action="store_true", help="Oyunu başlatma")
    args = parser.parse_args()

    specs = detect_system()
    settings = calculate_settings(specs, override_hz=args.hz)

    if not args.silent:
        c_cyan = "\033[96m"
        c_green = "\033[92m"
        c_yellow = "\033[93m"
        c_bold = "\033[1m"
        c_reset = "\033[0m"

        print(f"\n{c_bold}{c_cyan}======================================================{c_reset}")
        print(f"{c_bold}{c_cyan}      ⚔️  GOD OF WAR III - OTOMATİK SİSTEM AYARLAYICI  ⚔️{c_reset}")
        print(f"{c_bold}{c_cyan}======================================================{c_reset}\n")
        print(f"{c_bold}🔍 Algılanan Donanım:{c_reset}")
        print(f"  🖥️  Ekran Yenileme Hızı : {c_green}{settings['target_hz']} Hz{c_reset}" + (" (Manuel)" if args.hz else ""))
        print(f"  🎮 Ekran Kartı (GPU)    : {c_yellow}{specs['gpu_name']}{c_reset}")
        print(f"  📼 GPU Belleği (VRAM)   : {c_yellow}{specs['vram_mb']} MB{c_reset}")
        print(f"  🧠 Sistem Belleği (RAM) : {c_yellow}{specs['ram_mb']} MB{c_reset} (~{round(specs['ram_mb']/1024)} GB)\n")

        print(f"{c_bold}⚙️  Hesaplanan Optimize Değerler:{c_reset}")
        print(f"  🎯 Hedef Oyun Hızı (FPS): {c_green}{settings['target_hz']} FPS (Senkronize 1.0x hız){c_reset}")
        print(f"  📦 Doku Havuzu (VRAM GC): {c_green}{settings['gc_budget_mb']} MB{c_reset}")
        print(f"  💾 Bellek Havuzu (DMEM) : {c_green}{settings['dmem_mb']} MB{c_reset}")
        print(f"  🎨 Doku Kararlılığı      : {c_green}Anlık Okuma Açık (Sarı Doku Hatası Önleme){c_reset}\n")

    if not args.dry_run:
        update_xml_patches(settings["target_hz"])
        update_graphics_options_py(settings["target_hz"])
        update_run_sh(settings)
        update_ini_files(settings)
        update_user_config_toml(settings)
        update_graphics_state_json(settings)

        if not args.silent:
            print(f"{c_bold}{c_green}✅ run.sh, gow3.ini ve config.toml başarıyla güncellendi!{c_reset}")

        if not args.no_launch_prompt and not args.silent:
            if args.launch:
                should_launch = True
            elif args.no_launch:
                should_launch = False
            else:
                should_launch = ask_user_launch()

            if should_launch:
                run_sh_path = BASE_DIR / "run.sh"
                if run_sh_path.is_file():
                    print(f"\n{c_bold}{c_green}🚀 Oyun başlatılıyor (run.sh)...{c_reset}\n")
                    subprocess.run(["bash", str(run_sh_path)], cwd=str(BASE_DIR))
            else:
                print(f"\n{c_yellow}İşlem tamamlandı. İstediğin zaman './run.sh' komutuyla oyuna girebilirsin.{c_reset}\n")


if __name__ == "__main__":
    main()
