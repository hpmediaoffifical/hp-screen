//! Global hotkeys: start/stop, pause, screenshot. They work while another app has focus.
use crate::capture::{self, Target};
use crate::commands::AppState;
use crate::{events, screenshot};
use hp_core::settings::Settings;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HotkeyAction {
    Toggle,
    Pause,
    /// Capture an area and draw notes on it.
    Screenshot,
    /// Instant screenshot of the selected source.
    Quickshot,
}

/// Outcome of registering one hotkey, shown next to its field in Settings.
#[derive(Clone, Debug, Serialize)]
pub struct HotkeyStatus {
    pub action: HotkeyAction,
    pub hotkey: String,
    pub ok: bool,
    pub message: Option<String>,
}

/// Last registration results, kept for the `hotkey_status` command.
#[derive(Default)]
pub struct HotkeyRegistry(pub Mutex<Vec<HotkeyStatus>>);

/// Drops all hotkeys and registers the ones in `settings`. A hotkey another app already holds
/// is reported, not fatal.
pub fn register_hotkeys(app: &AppHandle, settings: &Settings) -> Vec<HotkeyStatus> {
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let results: Vec<HotkeyStatus> = [
        (HotkeyAction::Toggle, &settings.hotkey_toggle),
        (HotkeyAction::Pause, &settings.hotkey_pause),
        (HotkeyAction::Screenshot, &settings.hotkey_screenshot),
        (HotkeyAction::Quickshot, &settings.hotkey_quickshot),
    ]
    .into_iter()
    .map(|(action, hotkey)| {
        let registered = shortcuts.on_shortcut(plugin_format(hotkey).as_str(), move |app, _shortcut, event| {
            #[cfg(debug_assertions)]
            eprintln!("[hp] hotkey event {action:?} {:?}", event.state);
            if event.state == ShortcutState::Pressed {
                run_action(app, action);
            }
        });
        HotkeyStatus {
            action,
            hotkey: hotkey.clone(),
            ok: registered.is_ok(),
            message: registered.err().map(|_| format!("Không đăng ký được {hotkey}: có thể ứng dụng khác đang dùng tổ hợp này.")),
        }
    })
    .collect();
    #[cfg(debug_assertions)]
    eprintln!("[hp] register_hotkeys on {:?}: {:?}", std::thread::current().name(), results.iter().map(|r| (r.hotkey.as_str(), r.ok)).collect::<Vec<_>>());
    if let Some(registry) = app.try_state::<HotkeyRegistry>() {
        *registry.0.lock().unwrap_or_else(|e| e.into_inner()) = results.clone();
    }
    results
}

/// The primary monitor: what a hotkey records when nothing was chosen in the UI yet.
pub fn default_target() -> Target {
    let index = capture::list_monitors().into_iter().find(|m| m.primary).map(|m| m.index).unwrap_or(0);
    Target::Monitor { index }
}

pub fn run_action(app: &AppHandle, action: HotkeyAction) {
    #[cfg(debug_assertions)]
    eprintln!("[hp] run_action {action:?} state {:?}", app.try_state::<AppState>().map(|s| s.recorder.status().state));
    let Some(state) = app.try_state::<AppState>() else { return };
    let settings = state.settings();
    let result = match action {
        HotkeyAction::Toggle => state.recorder.toggle(default_target(), settings),
        // Pressing pause when nothing is recording is not an error worth showing.
        HotkeyAction::Pause => state.recorder.toggle_pause().or(Ok(())),
        HotkeyAction::Screenshot => {
            let app = app.clone();
            // Freezing the screen takes a moment; keep the hotkey thread free.
            std::thread::spawn(move || {
                if let Err(message) = crate::snip::start(&app) {
                    events::rec_error(&app, &message);
                }
            });
            Ok(())
        }
        HotkeyAction::Quickshot => {
            let target = state.recorder.last_target().unwrap_or_else(default_target);
            let app = app.clone();
            // Capturing takes a moment; keep the hotkey thread free.
            std::thread::spawn(move || match screenshot::take_screenshot(&app, &settings, &target) {
                Ok(_) => events::library_changed(&app),
                Err(message) => events::rec_error(&app, &message),
            });
            Ok(())
        }
    };
    if let Err(message) = result {
        events::rec_error(app, &message);
    }
}

/// The hotkey text in the form the shortcut plugin parses: it calls the Windows key "Super".
fn plugin_format(hotkey: &str) -> String {
    hotkey.replace("Win+", "Super+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn every_hotkey_the_settings_accept_is_understood_by_the_plugin() {
        for hotkey in ["Ctrl+Alt+R", "Win+Shift+5", "Ctrl+Win+F9", "F9", "Alt+Z", "Ctrl+Alt+Shift+Win+0", "Ctrl+F24"] {
            let canonical = hp_core::hotkey::format_hotkey(&hp_core::hotkey::parse_hotkey(hotkey).unwrap());
            let parsed: Result<Shortcut, _> = plugin_format(&canonical).parse();
            assert!(parsed.is_ok(), "{canonical} is rejected by the plugin: {:?}", parsed.err());
        }
    }
}
