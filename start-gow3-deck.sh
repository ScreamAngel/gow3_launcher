#!/usr/bin/env bash
# Steam Deck: writes the tuned preset (720p, engine 60, FPS 60, FSR 1) into the data directory
# on first run, then starts. Afterwards the in-game menu (Insert) edits ~/.local/share/gow3/gow3.ini.
set -euo pipefail
HERE="$(cd -- "$(dirname -- "$0")" && pwd)"
export GOW3_DATA_DIR="${GOW3_DATA_DIR:-$HOME/.local/share/gow3}"
mkdir -p "$GOW3_DATA_DIR"
if [[ ! -f $GOW3_DATA_DIR/gow3.ini && -f $HERE/gow3-deck.ini ]]; then
    cp "$HERE/gow3-deck.ini" "$GOW3_DATA_DIR/gow3.ini"
    echo "Steam Deck preset written to $GOW3_DATA_DIR/gow3.ini"
fi
exec bash "$HERE/start-gow3.sh" "$@"
