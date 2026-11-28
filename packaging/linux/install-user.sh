#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ -x "$SCRIPT_DIR/gree-remote-app" && -f "$SCRIPT_DIR/icon.png" ]]; then
  # Running from extracted GitHub Release archive
  BIN_SRC="$SCRIPT_DIR/gree-remote-app"
  ICON_SRC="$SCRIPT_DIR/icon.png"
  DESKTOP_SRC="$SCRIPT_DIR/gree-remote-app.desktop"
else
  # Running from git repository
  ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
  BIN_SRC="$ROOT_DIR/target/release/gree-remote-app"
  ICON_SRC="$ROOT_DIR/assets/icon.png"
  DESKTOP_SRC="$SCRIPT_DIR/gree-remote-app.desktop"
  if [[ ! -x "$BIN_SRC" ]]; then
    echo "Release binary not found. Building with cargo build --release..."
    (cd "$ROOT_DIR" && cargo build --release)
  fi
fi

install -Dm755 "$BIN_SRC" "$HOME/.local/bin/gree-remote-app"
install -Dm644 "$ICON_SRC" "$HOME/.local/share/icons/hicolor/256x256/apps/gree-remote-app.png"
install -Dm644 "$DESKTOP_SRC" "$HOME/.local/share/applications/gree-remote-app.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$HOME/.local/share/applications" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" || true
fi
if command -v kbuildsycoca6 >/dev/null 2>&1; then
  kbuildsycoca6 --noincremental >/dev/null 2>&1 || true
fi

echo "Installed Gree Remote for user $USER."
echo "Launch 'Gree Remote' from your application menu (ensure ~/.local/bin is in PATH)."
