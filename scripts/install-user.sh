#!/usr/bin/env bash
set -euo pipefail

APP="$(realpath "${1:-./target/release/look4}")"
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR" "$DATA_HOME/applications" "$DATA_HOME/autostart"
install -m 0755 "$APP" "$BIN_DIR/look4"
sed "s|^Exec=.*|Exec=$BIN_DIR/look4|" "$(dirname "$0")/../data/br.com.look4.LinkChecker.desktop" > "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop"
cp "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop" "$DATA_HOME/autostart/br.com.look4.LinkChecker.desktop"
chmod 0644 "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop" "$DATA_HOME/autostart/br.com.look4.LinkChecker.desktop"

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
echo "Look4 instalado com inicialização automática e atalho Ctrl+Alt+V."

