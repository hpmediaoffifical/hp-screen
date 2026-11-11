mod capture;
mod commands;
mod events;
mod paths;
mod screenshot;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let settings = paths::read_settings(app.handle());
            app.manage(AppState { settings: Mutex::new(settings) });
            #[cfg(debug_assertions)]
            eprintln!("[hp] monitors: {:?}", capture::list_monitors());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_monitors,
            commands::list_windows,
            commands::take_screenshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running HP Screen");
}
