#!/usr/bin/env bash
# Register the built desktop app in your applications menu (Linux).
# Run ./build-native.sh first, then this.
set -euo pipefail
cd "$(dirname "$0")"

BIN="$PWD/dist/bl2-editor"
if [ ! -x "$BIN" ]; then
  echo "Build the app first:  ./build-native.sh"
  exit 1
fi

APPS="$HOME/.local/share/applications"
mkdir -p "$APPS"
cat > "$APPS/bl2-save-editor.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=BL2 Save Editor
Comment=Edit Borderlands 2 character saves and account profile.bin
Exec=$BIN
Terminal=false
Categories=Game;Utility;
EOF
update-desktop-database "$APPS" 2>/dev/null || true

echo "Installed: $APPS/bl2-save-editor.desktop"
echo "'BL2 Save Editor' should now appear in your applications menu."
echo "(Run ./install-desktop.sh again after rebuilding if you move the folder.)"
