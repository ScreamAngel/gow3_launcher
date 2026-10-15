#!/usr/bin/env bash
# Starts God of War III. Double-click this (choose Execute) or the .desktop file.
# The game dump lives in this same folder (CUSA01623/); set GOW3_GAME_DIR to use another.
set -euo pipefail
HERE="$(cd -- "$(dirname -- "$0")" && pwd)"
export GOW3_PREBUILT=1
export GOW3_PROBE="$HERE/bin/gow3-probe"
export GOW3_DATA_DIR="${GOW3_DATA_DIR:-$HOME/.local/share/gow3}"
# Find the game: GOW3_GAME_DIR, CUSA01623/ here, else any subfolder holding an eboot.bin.
if [[ -z ${GOW3_GAME_DIR:-} || ! -f ${GOW3_GAME_DIR:-/nonexistent}/eboot.bin ]]; then
    for candidate in "$HERE/CUSA01623" "$HERE"/*/; do
        if [[ -f ${candidate%/}/eboot.bin ]]; then
            export GOW3_GAME_DIR="${GOW3_GAME_DIR:-"$HERE/CUSA01623"}"
            break
        fi
    done
fi
export GOW3_GAME_DIR="${GOW3_GAME_DIR:-"$HERE/CUSA01623"}"
mkdir -p "$GOW3_DATA_DIR"
cd "$HERE"
exec bash "$HERE/run.sh" "$@"
