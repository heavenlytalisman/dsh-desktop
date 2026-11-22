#!/usr/bin/env bash
#
# DSH Desktop installer
# Downloads the latest AppImage release from GitHub and installs it for the
# current user (no root required).
#
set -euo pipefail

REPO="heavenlytalisman/dsh-desktop"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
DESKTOP_DIR="${DESKTOP_DIR:-$HOME/.local/share/applications}"
APP_NAME="DSH Desktop"

echo "==> $APP_NAME installer"
echo "    Installs the latest release to $INSTALL_DIR (no root required)"

command -v curl >/dev/null 2>&1 || { echo "ERROR: curl is required"; exit 1; }

# Resolve the AppImage download URL from the latest GitHub release.
LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"
echo "==> Fetching latest release from $LATEST_URL"
RELEASE_JSON="$(curl -fsSL "$LATEST_URL")"

APP_IMAGE_URL="$(printf '%s' "$RELEASE_JSON" \
  | grep -o '"browser_download_url": *"[^"]*\.AppImage"' \
  | head -n1 \
  | sed 's/.*"\([^"]*\)".*/\1/')"

if [ -z "$APP_IMAGE_URL" ]; then
  echo "ERROR: no AppImage asset found in the latest release"
  exit 1
fi

echo "==> Downloading $APP_IMAGE_URL"
mkdir -p "$INSTALL_DIR" "$DESKTOP_DIR"

TMP_APP="$(mktemp)"
curl -fsSL "$APP_IMAGE_URL" -o "$TMP_APP"
chmod +x "$TMP_APP"

DEST="$INSTALL_DIR/dsh-desktop"
mv -f "$TMP_APP" "$DEST"

# Write a desktop entry so the app appears in the app menu and can be pinned
# to the taskbar.
DESKTOP_FILE="$DESKTOP_DIR/dsh.desktop"
cat > "$DESKTOP_FILE" <<DESKTOP
[Desktop Entry]
Name=$APP_NAME
GenericName=DeepSeek Harness Desktop
Comment=Linux desktop shell for DeepSeek Harness
Exec=$DEST
Icon=dsh-desktop
Terminal=false
Type=Application
Categories=Development;Office;
StartupWMClass=dsh-desktop
DESKTOP

echo
echo "==> Installed $APP_NAME to $DEST"
echo "==> Desktop entry written to $DESKTOP_FILE"
echo "==> Done."
echo "    Launch from your app menu, or run: $DEST"
