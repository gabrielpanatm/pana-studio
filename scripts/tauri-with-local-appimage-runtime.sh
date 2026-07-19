#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_BIN="$PROJECT_DIR/node_modules/.bin/tauri"
LOCAL_APPIMAGE_RUNTIME="${HOME}/.cache/tauri/runtime-x86_64"

if [ -x "$LOCAL_APPIMAGE_RUNTIME" ]; then
  export LDAI_RUNTIME_FILE="$LOCAL_APPIMAGE_RUNTIME"
fi

if [ "${1:-}" != "dev" ]; then
  exec "$TAURI_BIN" "$@"
fi

# Tauri CLI, Vite, esbuild and the desktop binary form one development stack.
# Keep them in a dedicated process group so Ctrl+C/TERM always tears down the
# entire stack instead of leaving a listener on the fixed Vite or MCP port.
DEV_OWNER_FILE="/tmp/pana-studio-tauri-dev-$(id -u).pid"
if [ -f "$DEV_OWNER_FILE" ]; then
  read -r recorded_pid recorded_project < "$DEV_OWNER_FILE" || true
  if [ -n "${recorded_pid:-}" ] && kill -0 "$recorded_pid" 2>/dev/null; then
    echo "Pană Studio dev rulează deja (supervisor PID $recorded_pid, proiect $recorded_project)." >&2
    echo "Oprește acea comandă cu Ctrl+C înainte de a porni o a doua instanță." >&2
    exit 73
  fi
  rm -f "$DEV_OWNER_FILE"
fi

printf '%s %s\n' "$$" "$PROJECT_DIR" > "$DEV_OWNER_FILE"
dev_pid=""

cleanup_dev_stack() {
  trap - EXIT INT TERM HUP
  if [ -n "$dev_pid" ] && kill -0 -- "-$dev_pid" 2>/dev/null; then
    kill -TERM -- "-$dev_pid" 2>/dev/null || true
    for _ in $(seq 1 30); do
      kill -0 -- "-$dev_pid" 2>/dev/null || break
      sleep 0.1
    done
    if kill -0 -- "-$dev_pid" 2>/dev/null; then
      kill -KILL -- "-$dev_pid" 2>/dev/null || true
    fi
  fi
  rm -f "$DEV_OWNER_FILE"
}

trap cleanup_dev_stack EXIT
trap 'exit 130' INT
trap 'exit 143' TERM HUP

setsid "$TAURI_BIN" "$@" &
dev_pid="$!"
set +e
wait "$dev_pid"
dev_status="$?"
set -e
exit "$dev_status"
