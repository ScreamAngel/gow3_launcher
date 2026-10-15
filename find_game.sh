#!/usr/bin/env bash
# God of War III - Home ve Disklerde Oyun Klasörü Bulucu ve run.sh Yapılandırıcı
set -euo pipefail
HERE="$(cd -- "$(dirname -- "$0")" && pwd)"
PYTHON=$(command -v python3 || echo "python")
exec "$PYTHON" "$HERE/scripts/find_game.py" "$@"
