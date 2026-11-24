use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

const STATE_FILE: &str = "window-state.json";

const AUTH_RETRY_SCRIPT: &str = r#"
(function() {
    // dsh's auth cookie is SameSite=Strict; the token redirect initiated from
    // the shell origin is cross-site, so WebKit withholds the cookie on the
    // redirect and shows "authentication required". A same-origin reload sends
    // it. Hide the page until we know it isn't that error, so the retry is
    // invisible. Retry once per origin.
    var gate = document.createElement('style');
    gate.textContent = 'html,body{opacity:0 !important}';
    (document.head || document.documentElement).appendChild(gate);
    function reveal() {
        if (gate.parentNode) gate.parentNode.removeChild(gate);
    }
    function retry() {
        var text = (document.body && document.body.innerText) || '';
        if (text.indexOf('dsh web authentication required') !== -1) {
            if (!sessionStorage.getItem('dsh-auth-retried')) {
                sessionStorage.setItem('dsh-auth-retried', '1');
                location.reload();
                return;
            }
        }
        reveal();
    }
    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', retry, { once: true });
    } else {
        retry();
    }
})();
"#;

// Runs before every page script on each top-level navigation, so the dsh page
// sees the dark theme override without racing the async navigation.
const THEME_SCRIPT: &str = r#"
    (function() {
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
        const inject = () => {
            if (document.head && !document.getElementById('dsh-desktop-opt')) {
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
                document.head.appendChild(style);
            }
        };
        const start = () => {
            inject();
            new MutationObserver(inject).observe(document.documentElement, { childList: true, subtree: true });
        };
        if (document.readyState === 'loading') {
            document.addEventListener('DOMContentLoaded', start, { once: true });
        } else {
            start();
        }
    })();
"#;

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

fn save_window_state(app: &AppHandle, window: &WebviewWindow) {
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let Some(path) = state_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::json!({
        "x": pos.x as f64,
        "y": pos.y as f64,
        "w": size.width as f64,
        "h": size.height as f64,
    });
    let _ = fs::write(&path, json.to_string());
}

/// dsh mints one `dsh-auth-<authority hash>` cookie per server instance. The
/// webview persists cookies across launches, so these pile up (one per random
/// port) until the Cookie header exceeds the server's 16 KiB header limit and
/// every request fails with 431, leaving a blank window. Drop stale auth
/// cookies before the webview opens the store.
fn prune_auth_cookie_lines(data: &str) -> (String, bool) {
    let mut out = String::with_capacity(data.len());
    let mut changed = false;
    for line in data.lines() {
        let parsed = line.strip_prefix("#HttpOnly_").unwrap_or(line);
        let name = parsed.split('\t').nth(5).unwrap_or("");
        if name.starts_with("dsh-auth") {
            changed = true;
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, changed)
}

fn prune_stale_auth_cookies(app: &AppHandle) {
    let Some(dir) = app.path().app_local_data_dir().ok() else {
        return;
    };
    let path = dir.join("cookies");
    let Ok(data) = fs::read_to_string(&path) else {
        return;
    };
    let (pruned, changed) = prune_auth_cookie_lines(&data);
    if changed {
        let _ = fs::write(&path, pruned);
    }
}

/// Create the shell window (showing the loading page) and return it. Safe to
/// call repeatedly: an existing window is focused instead of duplicated.
pub fn create_main_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(window);
    }

    prune_stale_auth_cookies(app);

    let (x, y, w, h) = load_window_state(app).unwrap_or((0.0, 0.0, 1280.0, 860.0));
    let use_saved = x > 0.0 && y > 0.0;

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DSH Desktop")
        .inner_size(w, h)
        .min_inner_size(640.0, 480.0)
        .resizable(true)
        .visible(true)
        .initialization_script(THEME_SCRIPT)
        .initialization_script(AUTH_RETRY_SCRIPT);

    builder = if use_saved {
        builder.position(x, y)
    } else {
        builder.center()
    };

    let window = builder.build().map_err(|e| e.to_string())?;

    let window2 = window.clone();
    let app2 = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            save_window_state(&app2, &window2);
            api.prevent_close();
            let _ = window2.hide();
        }
    });

    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(window)
}

/// Point the existing shell window at the authenticated dsh URL.
pub fn navigate_to(app: &AppHandle, url: &str) -> Result<(), String> {
    let content_url = url
        .parse::<url::Url>()
        .map_err(|_| format!("invalid dsh URL: {url}"))?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is missing".to_string())?;
    window.navigate(content_url).map_err(|e| e.to_string())?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn show_error(app: &AppHandle, message: &str) {
    if let Some(webview) = app.get_webview("main") {
        // The error can arrive before index.html finishes parsing, so poll
        // until the shell helper exists instead of dropping the message.
        let message = serde_json::to_string(message).unwrap_or_else(|_| "\"dsh failed\"".into());
        let script = format!(
            r#"(function() {{
                let tries = 0;
                (function show() {{
                    if (window.__DSH_SHELL__ && window.__DSH_SHELL__.showError) {{
                        window.__DSH_SHELL__.showError({message});
                    }} else if (tries++ < 200) {{
                        setTimeout(show, 50);
                    }}
                }})();
            }})();"#
        );
        let _ = webview.eval(&script);
    } else {
        eprintln!("{message}");
    }
}

#[cfg(test)]
mod tests {
    use super::prune_auth_cookie_lines;

    #[test]
    fn drops_stale_dsh_auth_cookies_only() {
        let data = "# HTTP Cookie File\n\
            #HttpOnly_127.0.0.1\tFALSE\t/\tFALSE\t0\tdsh-auth-abc123\tv1.secret\n\
            127.0.0.1\tFALSE\t/\tFALSE\t0\tother\tkeep\n";
        let (out, changed) = prune_auth_cookie_lines(data);
        assert!(changed);
        assert!(!out.contains("dsh-auth"));
        assert!(out.contains("other\tkeep"));
        assert!(out.contains("# HTTP Cookie File"));
    }

    #[test]
    fn leaves_store_untouched_without_auth_cookies() {
        let data = "# HTTP Cookie File\n127.0.0.1\tFALSE\t/\tFALSE\t0\tother\tkeep\n";
        let (out, changed) = prune_auth_cookie_lines(data);
        assert!(!changed);
        assert_eq!(out, data);
    }
}
