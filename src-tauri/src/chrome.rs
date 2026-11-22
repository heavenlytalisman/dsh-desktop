use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

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

    // System-default window decorations (native title bar). No custom
    // title bar — the dsh web page loads directly.
    // Create window with local HTML first (avoids Wayland 0x0 size bug
    // when loading an external URL directly), then navigate to dsh.
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("DSH Desktop")
        .inner_size(1280.0, 860.0)
        .center()
        .resizable(true)
        .visible(true)
        .build()
        .map_err(|e| e.to_string())?;

    let window2 = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
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
