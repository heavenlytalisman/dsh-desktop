use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl, WindowEvent,
};

const TITLEBAR_HEIGHT: f64 = 36.0;

pub fn setup_main_window(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is missing".to_string())?;
    let app_for_resize = app.clone();
    let window_for_resize = window.clone();
    window.on_window_event(move |_, event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window_for_resize.hide();
        }
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            if let Some(content) = app_for_resize.get_webview("content") {
                if let Ok((position, size)) = content_area(&window_for_resize) {
                    let _ = content.set_position(position);
                    let _ = content.set_size(size);
                }
            }
        }
        _ => {}
    });
    Ok(())
}

pub fn attach_content(app: &AppHandle, url: &str) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is missing".to_string())?;
    let content_url = url
        .parse::<url::Url>()
        .map_err(|_| format!("invalid dsh URL: {url}"))?;
    let (position, size) = content_area(&window)?;
    window
        .add_child(
            WebviewBuilder::new("content", WebviewUrl::External(content_url)),
            position,
            size,
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn show_error(app: &AppHandle, message: &str) {
    let Some(window) = app.get_webview_window("main") else {
        eprintln!("{message}");
        return;
    };
    let script = format!(
        "window.__DSH_SHELL__?.showError({});",
        serde_json::to_string(message).unwrap_or_else(|_| "\"dsh failed\"".into())
    );
    let _ = window.eval(&script);
}

fn content_area(
    window: &tauri::Window,
) -> Result<(LogicalPosition<f64>, LogicalSize<f64>), String> {
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let size = window
        .inner_size()
        .map_err(|error| error.to_string())?
        .to_logical(scale);
    let height = (size.height - TITLEBAR_HEIGHT).max(0.0);
    Ok((
        LogicalPosition::new(0.0, TITLEBAR_HEIGHT),
        LogicalSize::new(size.width, height),
    ))
}
