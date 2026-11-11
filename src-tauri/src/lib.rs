mod audio;
mod capture;
mod clock;
mod commands;
mod encoder;
mod events;
mod hotkeys;
mod input_hook;
mod paths;
mod pipeline;
mod recorder;
mod screenshot;
mod shell;

use commands::AppState;
use hotkeys::HotkeyRegistry;
use recorder::Recorder;
use std::sync::Mutex;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let handle = app.handle();
            let settings = paths::read_settings(handle);
            app.manage(HotkeyRegistry::default());
            app.manage(AppState { settings: Mutex::new(settings.clone()), recorder: Recorder::new(handle.clone()) });
            shell::setup(handle)?;
            let _registered = hotkeys::register_hotkeys(handle, &settings);
            #[cfg(debug_assertions)]
            eprintln!("[hp] hotkeys: {_registered:?}");
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
            commands::hotkey_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running HP Screen");
}
