#!/usr/bin/env sh
# Register Ctrl+Alt+B as "Brevity" in GNOME.
set -eu
BIN="${BIN:-$HOME/.local/bin/brevity}"
KEY="${KEY:-<Ctrl><Alt>b}"
BASE=/org/gnome/settings-daemon/plugins/media-keys
SLOT="$BASE/custom-keybindings/brevity/"

existing=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings)
case "$existing" in
  *"$SLOT"*) ;;
  "@as []") gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['$SLOT']" ;;
  *) gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "${existing%]}, '$SLOT']" ;;
esac

S="org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$SLOT"
gsettings set "$S" name 'Brevity'
gsettings set "$S" command "$BIN"
gsettings set "$S" binding "$KEY"
echo "bound $KEY -> $BIN"
