#!/usr/bin/env bash
#
# DSH Desktop installer
# Downloads the latest AppImage release from GitHub, falling back to a source
# build when no release is published. Installs for the current user (no root).
#
set -euo pipefail

REPO="heavenlytalisman/dsh-desktop-linux"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
DESKTOP_DIR="${DESKTOP_DIR:-$HOME/.local/share/applications}"
APP_NAME="DSH Desktop"
DEST="$INSTALL_DIR/dsh-desktop"
DESKTOP_FILE="$DESKTOP_DIR/dsh.desktop"
TMP_APP=""
SRC_DIR=""

cleanup() {
  if [ -n "$TMP_APP" ]; then rm -f "$TMP_APP"; fi
  if [ -n "$SRC_DIR" ]; then rm -rf "$SRC_DIR"; fi
}
trap cleanup EXIT

echo "==> $APP_NAME installer"
echo "    Installs to $INSTALL_DIR (no root required)"

command -v curl >/dev/null 2>&1 || { echo "ERROR: curl is required"; exit 1; }
mkdir -p "$INSTALL_DIR" "$DESKTOP_DIR"

write_desktop_entry() {
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
}

install_binary() {
  local src="$1" tmp
  mkdir -p "$INSTALL_DIR" "$DESKTOP_DIR"
  tmp="$(mktemp "$INSTALL_DIR/.dsh-desktop.XXXXXX")"
  cp "$src" "$tmp"
  chmod 755 "$tmp"
  mv -f "$tmp" "$DEST"
  write_desktop_entry
}

build_from_source() {
  echo "==> No AppImage release available; building from source"
  for cmd in git pnpm cargo; do
    command -v "$cmd" >/dev/null 2>&1 || {
      echo "ERROR: $cmd is required to build from source"
      exit 1
    }
  done
  BUILD_ROOT="${XDG_CACHE_HOME:-$HOME/.cache}"
  mkdir -p "$BUILD_ROOT"
  SRC_DIR="$(mktemp -d "$BUILD_ROOT/dsh-desktop-build.XXXXXX")"
  git clone --depth 1 "https://github.com/${REPO}.git" "$SRC_DIR"
  (cd "$SRC_DIR" && pnpm install --frozen-lockfile && pnpm exec vite build)
  (cd "$SRC_DIR/src-tauri" && cargo build --release)
  install_binary "$SRC_DIR/src-tauri/target/release/dsh-desktop"
}

# Resolve the AppImage download URL from the latest GitHub release.
LATEST_URL="https://api.github.com/repos/${REPO}/releases/latest"
echo "==> Fetching latest release from $LATEST_URL"
RELEASE_JSON=""
if RELEASE_JSON="$(curl -fsSL "$LATEST_URL" 2>/dev/null)"; then
  APP_IMAGE_URL="$(printf '%s' "$RELEASE_JSON" \
    | grep -o '"browser_download_url": *"[^"]*\.AppImage"' \
    | head -n1 \
    | sed 's/.*"\([^"]*\)".*/\1/' || true)"
else
  APP_IMAGE_URL=""
fi

if [ -n "$APP_IMAGE_URL" ]; then
  # Verify the download URL points to the expected repository.
  case "$APP_IMAGE_URL" in
    https://github.com/${REPO}/releases/download/*) ;;
    *) echo "ERROR: unexpected download URL: $APP_IMAGE_URL"; exit 1 ;;
  esac

  echo "==> Downloading $APP_IMAGE_URL"
  TMP_APP="$(mktemp "$INSTALL_DIR/.dsh-desktop.XXXXXX")"
  curl -fsSL "$APP_IMAGE_URL" -o "$TMP_APP"

  # Verify SHA256 if a checksum file is available in the release.
  SHA256_URL="${APP_IMAGE_URL}.sha256"
  if curl -fsSL "$SHA256_URL" -o /dev/null 2>/dev/null; then
    EXPECTED_SHA="$(curl -fsSL "$SHA256_URL" | awk '{print $1}')"
    ACTUAL_SHA="$(sha256sum "$TMP_APP" | awk '{print $1}')"
    if [ "$EXPECTED_SHA" != "$ACTUAL_SHA" ]; then
      echo "ERROR: checksum mismatch (expected $EXPECTED_SHA, got $ACTUAL_SHA)"
      exit 1
    fi
    echo "==> Checksum verified"
  fi

  install_binary "$TMP_APP"
else
  build_from_source
fi

echo
echo "==> Installed $APP_NAME to $DEST"
echo "==> Desktop entry written to $DESKTOP_FILE"
echo "==> Done."
echo "    Launch from your app menu, or run: $DEST"
