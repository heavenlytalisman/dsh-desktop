# DSH Desktop

A Linux desktop shell for [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness) (`dsh`), built with Tauri 2 and Rust.

It looks like the official DeepSeek Harness desktop app: a native window (system title bar + traffic lights) hosting the real `dsh web` UI, plus a system tray. Nothing in `dsh` is modified or reimplemented — the upstream web UI is used as-is, so sessions, models, settings and plugins are shared with the CLI and `dsh web`.

## Install

The easiest way is the one-line installer, which downloads the latest AppImage release and installs it to `~/.local/bin` (no root required):

```bash
curl -fsSL https://raw.githubusercontent.com/heavenlytalisman/dsh-desktop/main/install.sh | bash
```

This also writes a desktop entry so **DSH Desktop appears in your app menu and can be pinned to the taskbar**.

Launch it from your app menu or run:

```bash
~/.local/bin/dsh-desktop
```

### Manual install

Download the latest `DSH Desktop-*.AppImage` from [GitHub Releases](https://github.com/heavenlytalisman/dsh-desktop/releases), make it executable, and run it:

```bash
chmod +x DSH\ Desktop-*.AppImage
./DSH\ Desktop-*.AppImage
```

### Build from source

Requires `dsh` installed and Tauri 2 Linux build dependencies:

```bash
sudo dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf
pnpm install
pnpm tauri build
```

Bundles are written to `src-tauri/target/release/bundle/` (`AppImage`, `deb`).


## How it works

On startup the shell spawns `dsh web --no-open --port 0` and reads the authenticated URL that `dsh web` prints (`dsh web: http://127.0.0.1:<port>/?token=…`). That URL is loaded in a child webview, which keeps `dsh`'s `SameSite=Strict` authentication cookie first-party. Closing the window hides it to the tray; quitting from the tray stops the `dsh` process the shell started.

## Scope

v1 is the shell only: window, tray, single instance. It does not bundle Node.js or `dsh`, does not attach to an already-running `dsh web`, and does not add notifications or auto-update.

## License

MIT. DeepSeek Harness and its web UI are the property of their respective owners and are redistributed under their own license.
