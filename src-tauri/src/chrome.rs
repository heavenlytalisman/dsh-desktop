use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

const STATE_FILE: &str = "window-state.json";

fn state_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|p| p.join(STATE_FILE))
}

fn load_window_state(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let path = state_path(app)?;
    let data = fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&data).ok()?;
    Some((
        v.get("x")?.as_f64()?,
        v.get("y")?.as_f64()?,
        v.get("w")?.as_f64()?,
        v.get("h")?.as_f64()?,
    ))
}

pub fn open_main_window(app: &AppHandle, url: &str) -> Result<(), String> {
    // Single instance: if a main window already exists, just focus it.
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let content_url = url
        .parse::<url::Url>()
        .map_err(|_| format!("invalid dsh URL: {url}"))?;

    // Restore saved window geometry or use defaults.
    let (x, y, w, h) = load_window_state(app).unwrap_or((0.0, 0.0, 1280.0, 860.0));
    let use_saved = x > 0.0 && y > 0.0;

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DSH Desktop")
        .inner_size(w, h)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .visible(true);

    if use_saved {
        builder = builder.position(x, y);
    } else {
        builder = builder.center();
    }

    let window = builder.build().map_err(|e| e.to_string())?;

    let window2 = window.clone();
    let app2 = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            // Save window state before hiding.
            if let (Ok(pos), Ok(size)) = (window2.outer_position(), window2.outer_size()) {
                if let Some(path) = state_path(&app2) {
                    let json = serde_json::json!({
                        "x": pos.x as f64,
                        "y": pos.y as f64,
                        "w": size.width as f64,
                        "h": size.height as f64,
                    });
                    let _ = fs::write(&path, json.to_string());
                }
            }
            api.prevent_close();
            let _ = window2.hide();
        }
    });

    // Navigate the webview to the dsh URL now that the window has a size.
    window.navigate(content_url.clone()).map_err(|e| e.to_string())?;

    // Inject CSS optimizations after the page loads.
    let css = r#"
        (function() {
            // Force prefers-color-scheme: dark for WebKitGTK
            const origMatchMedia = window.matchMedia;
            window.matchMedia = function(q) {
                if (q === '(prefers-color-scheme: dark)') {
                    return { matches: true, media: q, onchange: null, addListener: () => {}, removeListener: () => {}, addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => true };
                }
                if (q === '(prefers-color-scheme: light)') {
                    return { matches: false, media: q, onchange: null, addListener: () => {}, removeListener: () => {}, addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => true };
                }
                return origMatchMedia.call(window, q);
            };
            const style = document.createElement('style');
            style.id = 'dsh-desktop-opt';
            style.textContent = `
                html {
                    -webkit-font-smoothing: antialiased !important;
                    -moz-osx-font-smoothing: grayscale !important;
                    text-rendering: optimizeLegibility !important;
                    color-scheme: dark !important;
                }
                * {
                    -webkit-font-smoothing: antialiased;
                }
                html, body {
                    scroll-behavior: smooth !important;
                    background-color: #151517 !important;
                }
                :root {
                    --dsw-alias-bg-base: #151517 !important;
                }
            `;
            const inject = () => {
                if (document.head && !document.getElementById('dsh-desktop-opt')) {
                    document.head.appendChild(style);
                }
            };
            if (document.readyState === 'loading') {
                document.addEventListener('DOMContentLoaded', inject, { once: true });
            } else {
                inject();
            }
            const observer = new MutationObserver(() => {
                if (!document.getElementById('dsh-desktop-opt')) inject();
            });
            if (document.body) {
                observer.observe(document.body, { childList: true, subtree: true });
            }
        })();
    "#;
    let _ = window.eval(css);

    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(dead_code)]
pub fn show_error(app: &AppHandle, message: &str) {
    if let Some(webview) = app.get_webview("main") {
        let script = format!(
            "window.__DSH_SHELL__?.showError({});",
            serde_json::to_string(message).unwrap_or_else(|_| "\"dsh failed\"".into())
        );
        let _ = webview.eval(&script);
    } else {
        eprintln!("{message}");
    }
}
