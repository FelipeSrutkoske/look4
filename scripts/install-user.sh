#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(dirname "$(realpath "$0")")"
APP="$(realpath "${1:-$SCRIPT_DIR/../target/release/look4}")"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="$HOME/.local/bin"
ICON_DIR="$DATA_HOME/icons/hicolor/scalable/apps"

mkdir -p "$BIN_DIR" "$DATA_HOME/applications" "$DATA_HOME/autostart" "$ICON_DIR"
install -m 0755 "$APP" "$BIN_DIR/look4"

# Instalar ícone customizado SVG
install -m 0644 "$SCRIPT_DIR/../data/br.com.look4.LinkChecker.svg" "$ICON_DIR/br.com.look4.LinkChecker.svg"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$DATA_HOME/icons/hicolor" 2>/dev/null || true
fi

# Instalar lançador .desktop
sed "s|^Exec=.*|Exec=$BIN_DIR/look4|" "$SCRIPT_DIR/../data/br.com.look4.LinkChecker.desktop" > "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop"
cp "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop" "$DATA_HOME/autostart/br.com.look4.LinkChecker.desktop"
chmod 0644 "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop" "$DATA_HOME/autostart/br.com.look4.LinkChecker.desktop"

# Configurar atalho global GNOME
SCHEMA="org.gnome.settings-daemon.plugins.media-keys"
BASE="/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/look4/"
CURRENT="$(gsettings get "$SCHEMA" custom-keybindings)"
if ! grep -q "custom-keybindings/look4/" <<<"$CURRENT"; then
  if [[ "$CURRENT" == "@as []" || "$CURRENT" == "[]" ]]; then UPDATED="['$BASE']"; else UPDATED="${CURRENT%]}, '$BASE']"; fi
  gsettings set "$SCHEMA" custom-keybindings "$UPDATED"
fi
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:"$BASE" name 'Verificar link com Look4'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:"$BASE" command "$BIN_DIR/look4 --scan-clipboard"
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:"$BASE" binding '<Control><Alt>v'
echo "Look4 instalado com ícone personalizado, inicialização automática e atalho Ctrl+Alt+V."
