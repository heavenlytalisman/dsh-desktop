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

    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
