mod audio;
mod capture;
mod clock;
mod commands;
mod encoder;
mod events;
mod hotkeys;
mod input_hook;
mod library_fs;
mod paths;
mod picker;
mod pipeline;
mod recorder;
mod screenshot;
mod shell;
mod snip;
mod decoder;
mod compositor;
mod export;
mod edit_cmds;
mod live_layer;
#[cfg(test)]
mod test_media;

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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .setup(|app| {
            let handle = app.handle();
            let settings = paths::read_settings(handle);
            app.manage(HotkeyRegistry::default());
            app.manage(picker::PickerState::default());
            app.manage(snip::SnipState::default());
            app.manage(edit_cmds::ExportState::default());
            // An export cut short by quitting leaves a hidden .part file; remove it.
            if let Ok(dir) = paths::output_dir(handle, &settings) {
                edit_cmds::remove_stale_parts(&dir);
            }
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
            commands::pick_target,
            commands::picker_context,
            commands::picker_done,
            commands::settings_get,
            commands::settings_set,
            commands::library_list,
            commands::library_rename,
            commands::library_delete,
            commands::library_open,
            commands::library_reveal,
            commands::preview_name,
            commands::pick_folder,
            commands::output_folder,
            commands::set_target,
            commands::snip_start,
            commands::snip_context,
            commands::snip_ready,
            commands::snip_finish,
            commands::snip_cancel,
            commands::snip_frozen,
            commands::compact_show,
            commands::compact_hide,
            commands::open_main,
            commands::get_target,
            edit_cmds::edit_open,
            edit_cmds::edit_save,
            edit_cmds::edit_keep,
            edit_cmds::export_start,
            edit_cmds::export_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("error while running HP Screen");
}
