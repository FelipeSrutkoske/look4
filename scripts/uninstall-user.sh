#!/usr/bin/env bash
set -euo pipefail
DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
BASE="/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/look4/"
SCHEMA="org.gnome.settings-daemon.plugins.media-keys"
CURRENT="$(gsettings get "$SCHEMA" custom-keybindings)"
UPDATED="$(sed "s|'$BASE'||g; s|, ,|,|g; s|\[, |[|g; s|, \]|]|g" <<<"$CURRENT")"
gsettings set "$SCHEMA" custom-keybindings "$UPDATED"
rm -f "$HOME/.local/bin/look4" "$DATA_HOME/applications/br.com.look4.LinkChecker.desktop" "$DATA_HOME/autostart/br.com.look4.LinkChecker.desktop"
echo "Look4 removido. A chave permanece no chaveiro do sistema."

