#!/usr/bin/env bash
# Installs TuxLR for the current user and the GoXLR udev rule system-wide.
set -euo pipefail
cd "$(dirname "$0")"

if [[ ! -x target/release/tuxlr ]]; then
    echo "Building TuxLR (release)…"
    cargo build --release
fi

BIN="${HOME}/.local/bin"
APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor"

install -Dm755 target/release/tuxlr "$BIN/tuxlr"
install -Dm644 assets/tuxlr.png "$ICONS/256x256/apps/tuxlr.png"
install -Dm644 assets/tuxlr.svg "$ICONS/scalable/apps/tuxlr.svg"
sed "s|^Exec=tuxlr|Exec=$BIN/tuxlr|" packaging/tuxlr.desktop > /tmp/tuxlr.desktop.$$
install -Dm644 /tmp/tuxlr.desktop.$$ "$APPS/tuxlr.desktop"
rm -f /tmp/tuxlr.desktop.$$
command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" || true

echo "Installed TuxLR to $BIN/tuxlr"

if [[ ! -f /etc/udev/rules.d/50-goxlr.rules ]]; then
    echo
    echo "Installing the GoXLR udev rule (needs sudo)…"
    sudo install -m 644 50-goxlr.rules /etc/udev/rules.d/50-goxlr.rules
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    echo "Done. Unplug and re-plug your GoXLR."
fi
