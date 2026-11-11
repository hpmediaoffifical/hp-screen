mod audio;
mod capture;
mod clock;
mod commands;
mod encoder;
mod events;
mod paths;
mod pipeline;
mod recorder;
mod screenshot;

use commands::AppState;
use recorder::Recorder;
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
            app.manage(AppState { settings: Mutex::new(settings), recorder: Recorder::new(app.handle().clone()) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_monitors,
            commands::list_windows,
            commands::take_screenshot,
            commands::rec_start,
            commands::rec_stop,
            commands::rec_toggle_pause,
            commands::rec_cancel,
            commands::rec_status,
            commands::audio_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running HP Screen");
}
