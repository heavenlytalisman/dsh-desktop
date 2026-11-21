mod chrome;
mod dsh;
mod tray;

use std::sync::Mutex;
use tauri::{Manager, RunEvent};

pub struct DshChild(pub Mutex<Option<std::process::Child>>);

impl Default for DshChild {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(DshChild::default())
        .setup(|app| {
            let handle = app.handle().clone();
            chrome::setup_main_window(&handle)?;
            tray::create(&handle)?;
            std::thread::spawn(move || match dsh::spawn() {
                Ok((child, url)) => {
                    if let Some(state) = handle.try_state::<DshChild>() {
                        *state.0.lock().expect("dsh child lock poisoned") = Some(child);
                    }
                    if let Err(error) = chrome::attach_content(&handle, &url) {
                        chrome::show_error(&handle, &error);
                    }
                }
                Err(error) => chrome::show_error(&handle, &error.to_string()),
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<DshChild>() {
                    if let Some(mut child) = state.0.lock().expect("dsh child lock poisoned").take() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
            }
        });
}
