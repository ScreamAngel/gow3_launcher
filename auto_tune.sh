#!/usr/bin/env bash
# God of War III - Otomatik Donanım Algılama ve Yapılandırma Aracı
set -euo pipefail
HERE="$(cd -- "$(dirname -- "$0")" && pwd)"
PYTHON=$(command -v python3 || echo "python")
exec "$PYTHON" "$HERE/scripts/auto_tune.py" "$@"
