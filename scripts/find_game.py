#!/usr/bin/env python3
"""Game Finder and Auto-Configurator for God of War III Linux port.

Scans the home directory (and common mount points) for CUSA01623 or CUSA01715,
updates run.sh and start-gow3.sh to point to the found game directory,
and then automatically launches auto_tune.sh.
"""
import argparse
import os
import re
import subprocess
import sys
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(BASE_DIR / "scripts"))

try:
    from prepare import sfo
except ImportError:
    sfo = None


def parse_param_sfo(sfo_path: Path):
    if not sfo_path.is_file() or sfo is None:
        return {}
    try:
        return sfo(sfo_path.read_bytes())
    except Exception:
        return {}


def is_valid_gow3_dir(path: Path):
    """Check if a directory contains a valid God of War III Remastered game."""
    eboot = path / "eboot.bin"
    if not eboot.is_file():
        return False, None

    # Check param.sfo
    sfo_data = parse_param_sfo(path / "sce_sys" / "param.sfo")
    title_id = sfo_data.get("TITLE_ID", "")
    category = sfo_data.get("CATEGORY", "")
    title = sfo_data.get("TITLE", "")
    app_ver = sfo_data.get("APP_VER", "")

    # Reject patch-only PKG dumps if category is 'gp' (game patch)
    # unless it also has the full EXEC game content
    has_exec = (path / "EXEC").is_dir()
    is_patch_only = (category == "gp") and not has_exec

    # Name check
    name = path.name
    is_target_name = (
        name in ("CUSA01623", "CUSA01715")
        or title_id in ("CUSA01623", "CUSA01715")
        or "God of War" in title
    )

    if is_target_name and not is_patch_only:
        info = {
            "path": path.resolve(),
            "title": title or "God of War® III Remastered",
            "title_id": title_id or name,
            "version": app_ver or "01.00",
            "category": category or "gd",
            "has_exec": has_exec,
        }
        return True, info

    return False, None


def scan_directories():
    candidates = []
    seen = set()

    # 1. Check current directory first
    for candidate_name in ["CUSA01623", "CUSA01715"]:
        local_p = BASE_DIR / candidate_name
        if local_p.is_dir():
            valid, info = is_valid_gow3_dir(local_p)
            if valid and str(info["path"]) not in seen:
                candidates.append(info)
                seen.add(str(info["path"]))

    # 2. Scan $HOME
    home = Path.home()
    skip_dirs = {
        ".cache",
        ".local/share/Trash",
        ".cargo",
        ".rustup",
        ".npm",
        ".nvm",
        "node_modules",
        ".venv",
        "venv",
        ".git",
        ".var/app",
        "proc",
        "sys",
    }

    for root, dirs, files in os.walk(home):
        rel = os.path.relpath(root, home)
        if any(rel == s or rel.startswith(s + os.sep) for s in skip_dirs):
            dirs.clear()
            continue

        bname = os.path.basename(root)
        if (
            bname in ("CUSA01623", "CUSA01715")
            or "god of war" in bname.lower()
            or "gow3" in bname.lower()
        ):
            if "eboot.bin" in files:
                p = Path(root)
                valid, info = is_valid_gow3_dir(p)
                if valid and str(info["path"]) not in seen:
                    candidates.append(info)
                    seen.add(str(info["path"]))

    # 3. If not found in HOME, search common mount points (/run/media, /media, /mnt)
    if not candidates:
        user = os.environ.get("USER", "")
        mount_roots = [Path(f"/run/media/{user}"), Path("/media"), Path("/mnt")]
        for mroot in mount_roots:
            if not mroot.is_dir():
                continue
            for root, dirs, files in os.walk(mroot):
                # Limit depth to 4
                rel_parts = Path(root).relative_to(mroot).parts
                if len(rel_parts) >= 4:
                    dirs.clear()
                    continue
                bname = os.path.basename(root)
                if (
                    bname in ("CUSA01623", "CUSA01715")
                    or "god of war" in bname.lower()
                    or "gow3" in bname.lower()
                ):
                    if "eboot.bin" in files:
                        p = Path(root)
                        valid, info = is_valid_gow3_dir(p)
                        if valid and str(info["path"]) not in seen:
                            candidates.append(info)
                            seen.add(str(info["path"]))

    return candidates


def update_run_sh(chosen_path: Path):
    run_sh = BASE_DIR / "run.sh"
    if not run_sh.is_file():
        return False

    content = run_sh.read_text(encoding="utf-8")

    # If chosen_path is inside BASE_DIR, use $SCRIPT_DIR/relative_path for portability
    try:
        rel = chosen_path.relative_to(BASE_DIR)
        formatted_path = f'"$SCRIPT_DIR/{rel}"'
    except ValueError:
        formatted_path = f'"{chosen_path}"'

    # Update DEFAULT_GAME_DIR
    if re.search(r'^DEFAULT_GAME_DIR=.*$', content, re.M):
        content = re.sub(r'^DEFAULT_GAME_DIR=.*$', f'DEFAULT_GAME_DIR={formatted_path}', content, flags=re.M)
    else:
        # Fallback to replacing game=${GOW3_GAME_DIR:-...}
        content = re.sub(
            r'game=\$\{GOW3_GAME_DIR:-.*?\}',
            f'game=${{GOW3_GAME_DIR:-{formatted_path}}}',
            content,
        )

    run_sh.write_text(content, encoding="utf-8")
    return True


def update_start_gow3_sh(chosen_path: Path):
    start_sh = BASE_DIR / "start-gow3.sh"
    if not start_sh.is_file():
        return False

    content = start_sh.read_text(encoding="utf-8")
    try:
        rel = chosen_path.relative_to(BASE_DIR)
        formatted_path = f'"$HERE/{rel}"'
    except ValueError:
        formatted_path = f'"{chosen_path}"'

    content = re.sub(
        r'export GOW3_GAME_DIR="\$\{GOW3_GAME_DIR:-.*?\}\"',
        f'export GOW3_GAME_DIR="${{GOW3_GAME_DIR:-{formatted_path}}}"',
        content,
    )
    start_sh.write_text(content, encoding="utf-8")
    return True


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
    parser = argparse.ArgumentParser(description="Find God of War III game directory in HOME")
    parser.add_argument("--no-auto-tune", action="store_true", help="İşlem sonrası auto_tune.sh'ı çalıştırma")
    parser.add_argument("--launch", action="store_true", help="Sorulmadan doğrudan oyunu (run.sh) başlat")
    parser.add_argument("--no-launch", action="store_true", help="Oyunu (run.sh) başlatma ve soru sorma")
    args = parser.parse_args()

    c_cyan = "\033[96m"
    c_green = "\033[92m"
    c_yellow = "\033[93m"
    c_red = "\033[91m"
    c_bold = "\033[1m"
    c_reset = "\033[0m"

    print(f"\n{c_bold}{c_cyan}======================================================{c_reset}")
    print(f"{c_bold}{c_cyan}      🔎  GOD OF WAR III - OYUN KLASÖRÜ BULUCU         ⚔️{c_reset}")
    print(f"{c_bold}{c_cyan}======================================================{c_reset}\n")
    print(f"{c_yellow}Sistemde (Home ve disklerde) CUSA01623 / CUSA01715 taranıyor...{c_reset}")

    candidates = scan_directories()

    if not candidates:
        print(f"\n{c_bold}{c_red}❌ HATA: Sistemde geçerli bir CUSA01623 veya CUSA01715 God of War III klasörü bulunamadı!{c_reset}")
        print(f"{c_yellow}Lütfen oyun klasörünün içinde 'eboot.bin' olduğundan emin olun.{c_reset}\n")
        sys.exit(1)

    print(f"\n{c_bold}{c_green}✅ {len(candidates)} adet geçerli God of War III klasörü bulundu:{c_reset}\n")
    for i, c in enumerate(candidates, 1):
        print(f"  [{c_bold}{i}{c_reset}] {c_cyan}{c['path']}{c_reset}")
        print(f"      🏷️  Başlık : {c['title']} ({c['title_id']}) | Sürüm: {c['version']}")

    chosen = candidates[0]
    if len(candidates) > 1:
        choice_str = ""
        prompt_folder = f"\nKullanmak istediğiniz klasör numarasını seçin [1-{len(candidates)}] (Varsayılan 1): "
        try:
            with open("/dev/tty", "r") as tty_in:
                sys.stdout.write(prompt_folder)
                sys.stdout.flush()
                choice_str = tty_in.readline().strip()
        except Exception:
            try:
                if sys.stdin and not sys.stdin.closed:
                    choice_str = input(prompt_folder).strip()
            except (KeyboardInterrupt, EOFError):
                print("\nİptal edildi.")
                sys.exit(0)

        if choice_str.isdigit() and 1 <= int(choice_str) <= len(candidates):
            chosen = candidates[int(choice_str) - 1]

    print(f"\n{c_bold}Seçilen Oyun Konumu:{c_reset} {c_green}{chosen['path']}{c_reset}")

    # Update run.sh and start-gow3.sh
    update_run_sh(chosen["path"])
    update_start_gow3_sh(chosen["path"])
    print(f"{c_bold}{c_green}✅ run.sh ve start-gow3.sh yeni oyun konumuna göre başarıyla güncellendi!{c_reset}")

    # Launch auto_tune.sh if requested (with --no-launch-prompt so it doesn't double ask)
    if not args.no_auto_tune:
        auto_tune_script = BASE_DIR / "auto_tune.sh"
        if auto_tune_script.is_file():
            print(f"\n{c_cyan}🚀 auto_tune.sh başlatılıyor...{c_reset}")
            subprocess.run(["bash", str(auto_tune_script), "--no-launch-prompt"], cwd=str(BASE_DIR))

    # Ask user if they want to launch run.sh
    if not args.no_launch:
        if args.launch:
            should_launch = True
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
