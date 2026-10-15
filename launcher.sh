#!/usr/bin/env bash
# God of War III Remastered - Rust Native Launcher
set -euo pipefail
HERE="$(cd -- "$(dirname -- "$0")" && pwd)"
BIN="$HERE/gow3-launcher"

if [ ! -f "$BIN" ]; then
    BIN="$HERE/launcher/target/release/launcher"
    if [ ! -f "$BIN" ]; then
        echo "⚙️ Launcher derleniyor, lütfen bekleyin..."
        cargo build --release --manifest-path "$HERE/launcher/Cargo.toml"
    fi
fi

exec "$BIN" "$@"
