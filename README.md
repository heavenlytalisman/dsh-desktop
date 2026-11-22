# DSH Desktop

<p align="center">
  <img src="src-tauri/icons/icon.png" width="96" alt="DSH Desktop" />
</p>

<p align="center">
  <strong>Linux desktop shell for <a href="https://github.com/deepseek-ai/deepseek-harness">DeepSeek Harness</a></strong><br />
  Built with Tauri 2 & Rust · Embeds the real <code>dsh web</code> UI
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: MIT" src="https://img.shields.io/badge/License-MIT-171513.svg" /></a>
  <img alt="Platform" src="https://img.shields.io/badge/Platform-Linux-171513.svg" />
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2.x-171513.svg" />
</p>

---

## Overview

DSH Desktop is a lightweight native shell that hosts the DeepSeek Harness web UI in a desktop window. It does not reimplement or modify `dsh` — the upstream web UI runs as-is, sharing sessions, models, settings, and plugins with the CLI.

## What DSH Desktop adds

DeepSeek Harness provides the Agent runtime, Web UI, and basic model-provider integration. DSH Desktop builds on these foundations with:

- **Managed Harness lifecycle**: starts `dsh web` on launch, stops it on quit, and cleans up on exit
- **Native window management**: system title bar, resizable window, remembers size and position across sessions
- **System tray integration**: show, hide, and quit from the tray icon; clicking the tray toggles window visibility
- **Single instance**: clicking the app icon focuses the existing window instead of launching a duplicate
- **Close to tray**: closing the window hides it to the tray rather than terminating; quit explicitly from the tray menu
- **Environment detection**: locates `dsh` and `node` even when launched from app drawer or systemd (minimal PATH)
- **Wayland support**: works on KDE Wayland with proper window sizing and rendering (WebKitGTK DMA-BUF fixes)
- **Error recovery**: shows an error window if `dsh` fails to start instead of failing silently

## About

DSH Desktop is an independent community project — not affiliated with or endorsed by DeepSeek. It wraps the official DeepSeek Harness web UI in a native Linux window, adding system tray, window management, and desktop integration. All agent, model, tool, and session logic is provided by `dsh` itself.

## Install

### One-line installer

```bash
curl -fsSL https://raw.githubusercontent.com/heavenlytalisman/dsh-desktop-linux/main/install.sh | bash
```

Downloads the latest release to `~/.local/bin`, writes a desktop entry so DSH Desktop appears in your app menu and can be pinned to the taskbar.

### Manual install

| Distro | Command |
|---|---|
| Fedora / RHEL | `sudo dnf install ./DSH-Desktop-*.rpm` |
| Debian / Ubuntu | `sudo apt install ./DSH-Desktop-*.deb` |
| Other (AppImage) | `chmod +x DSH\ Desktop-*.AppImage && ./DSH\ Desktop-*.AppImage` |

### Build from source

```bash
# Dependencies (Fedora)
sudo dnf install -y webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel patchelf

# Build
pnpm install
pnpm tauri build
```

Bundles are written to `src-tauri/target/release/bundle/` (`AppImage`, `deb`, `rpm`).

## Requirements

- `dsh` installed (`npm install -g @deepseek-ai/dsh`)
- Node.js 22+ (via fnm, nvm, or system)

## How it works

1. On startup, finds `dsh` and `node` on `PATH` (or common locations like fnm/nvm)
2. Spawns `dsh web --no-open --port 0`
3. Reads the authenticated URL from stdout (`dsh web: http://127.0.0.1:<port>/?token=…`)
4. Loads it in the main webview
5. **Close** → hides to tray | **Quit from tray** → stops dsh

## Data & security

- Sessions and configuration stay in `~/.dsh` — the same directory used by the CLI
- No data is sent to external services by the shell itself; network requests are made by `dsh` according to your model provider configuration
- The shell does not modify, intercept, or proxy `dsh` traffic

## Platform support

| Platform | Status |
|---|---|
| Linux (x86_64) | Supported (AppImage, deb, rpm) |
| Linux (aarch64) | Not tested |
| macOS / Windows | Not supported (use official clients) |

## Development

```bash
pnpm install
pnpm tauri dev
```

## License

MIT — DeepSeek Harness and its web UI are the property of their respective owners.