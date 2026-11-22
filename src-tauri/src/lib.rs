mod chrome;
mod dsh;
mod tray;

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;
use tauri::{Manager, RunEvent};

pub struct DshChild(pub Mutex<Option<std::process::Child>>);

impl Default for DshChild {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

static DSH_PID: AtomicI32 = AtomicI32::new(0);

// `RunEvent::Exit` does not run on SIGTERM, so kill the spawned host here too.
extern "C" fn on_terminate(_signal: libc::c_int) {
    let pid = DSH_PID.load(Ordering::SeqCst);
    if pid > 0 {
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
    unsafe { libc::_exit(0) };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGTERM, on_terminate as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_terminate as *const () as libc::sighandler_t);
    }
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
            tray::create(&handle)?;
            match dsh::spawn() {
                Ok((child, url)) => {
                    DSH_PID.store(child.id() as i32, Ordering::SeqCst);
                    if let Some(state) = handle.try_state::<DshChild>() {
                        *state.0.lock().expect("dsh child lock poisoned") = Some(child);
                    }
                    if let Err(error) = chrome::open_main_window(&handle, &url) {
                        eprintln!("failed to open main window: {error}");
                    }
                }
                Err(error) => eprintln!("failed to spawn dsh: {error}"),
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                if let Some(state) = app.try_state::<DshChild>() {
                    if let Some(mut child) = state.0.lock().expect("dsh child lock poisoned").take()
                    {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                }
                DSH_PID.store(0, Ordering::SeqCst);
            }
        });
}
