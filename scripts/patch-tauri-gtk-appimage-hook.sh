#!/usr/bin/env bash
set -euo pipefail

HOOK_FILE="${HOME}/.cache/tauri/linuxdeploy-plugin-gtk.sh"

if [ ! -f "$HOOK_FILE" ]; then
  echo "GTK AppImage hook not found yet, skipping Wayland patch: $HOOK_FILE"
  exit 0
fi

python3 - "$HOOK_FILE" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
text = path.read_text()
old = 'export GDK_BACKEND=x11 # Crash with Wayland backend on Wayland - We tested it without it and ended up with this: https://github.com/tauri-apps/tauri/issues/8541'
new = '''if [ -n "${WAYLAND_DISPLAY:-}" ] && [ "${PANA_STUDIO_FORCE_X11:-}" != "1" ]; then
    export GDK_BACKEND=wayland,x11
else
    export GDK_BACKEND=x11
fi'''

if new in text:
    print(f"GTK AppImage hook already patched: {path}")
    sys.exit(0)

if old not in text:
    raise SystemExit(f"Expected GDK_BACKEND line not found in {path}")

path.write_text(text.replace(old, new))
print(f"Patched GTK AppImage hook for Wayland sharp text: {path}")
PY
