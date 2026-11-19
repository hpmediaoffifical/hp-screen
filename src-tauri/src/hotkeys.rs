//! Global hotkeys: start/stop, pause, screenshot. They work while another app has focus.
use crate::capture::{self, Target};
use crate::commands::AppState;
use crate::{events, screenshot};
use hp_core::settings::Settings;
use hp_core::state::RecState;
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
    /// Draw on screen while recording.
    Draw,
}

/// Outcome of registering one hotkey, shown next to its field in Settings.
#[derive(Clone, Debug, Serialize)]
pub struct HotkeyStatus {
    pub action: HotkeyAction,
    pub hotkey: String,
    pub ok: bool,
    pub message: Option<String>,
    /// A free combination to switch to when another app holds this one.
    pub suggestion: Option<String>,
}

/// Last registration results, kept for the `hotkey_status` command.
#[derive(Default)]
pub struct HotkeyRegistry(pub Mutex<Vec<HotkeyStatus>>);

/// Drops all hotkeys and registers the ones in `settings`. A hotkey another app already holds
/// is reported, not fatal.
pub fn register_hotkeys(app: &AppHandle, settings: &Settings) -> Vec<HotkeyStatus> {
    let _ = app.global_shortcut().unregister_all();
    let results: Vec<HotkeyStatus> = [
        (HotkeyAction::Toggle, &settings.hotkey_toggle),
        (HotkeyAction::Pause, &settings.hotkey_pause),
        (HotkeyAction::Screenshot, &settings.hotkey_screenshot),
        (HotkeyAction::Quickshot, &settings.hotkey_quickshot),
        (HotkeyAction::Draw, &settings.hotkey_draw),
    ]
    .into_iter()
    .map(|(action, hotkey)| register_one(app, action, hotkey, settings))
    .collect();
    #[cfg(debug_assertions)]
    eprintln!("[hp] register_hotkeys on {:?}: {:?}", std::thread::current().name(), results.iter().map(|r| (r.hotkey.as_str(), r.ok)).collect::<Vec<_>>());
    if let Some(registry) = app.try_state::<HotkeyRegistry>() {
        *registry.0.lock().unwrap_or_else(|e| e.into_inner()) = results.clone();
    }
    let _ = tauri::Emitter::emit(app, "hotkeys-changed", &results);
    results
}

/// Registers one hotkey. When another app already holds it, the status says so and offers a
/// free combination to switch to.
fn register_one(app: &AppHandle, action: HotkeyAction, hotkey: &str, settings: &Settings) -> HotkeyStatus {
    let registered = app.global_shortcut().on_shortcut(plugin_format(hotkey).as_str(), move |app, _shortcut, event| {
        #[cfg(debug_assertions)]
        eprintln!("[hp] hotkey event {action:?} {:?}", event.state);
        if event.state == ShortcutState::Pressed {
            run_action(app, action);
        }
    });
    if registered.is_ok() {
        return HotkeyStatus { action, hotkey: hotkey.to_string(), ok: true, message: None, suggestion: None };
    }
    let taken = [&settings.hotkey_toggle, &settings.hotkey_pause, &settings.hotkey_screenshot, &settings.hotkey_quickshot]
        .map(|k| k.to_string());
    let suggestion = hp_core::hotkey::alternatives(default_hotkey(action), &taken).into_iter().find(|k| is_free(k));
    let known = match hotkey {
        "Ctrl+Alt+S" | "Ctrl+Alt+A" => " (Zalo dùng tổ hợp này để chụp màn hình)",
        _ => "",
    };
    HotkeyStatus {
        action,
        hotkey: hotkey.to_string(),
        ok: false,
        message: Some(format!("{hotkey} đang bị một ứng dụng khác giữ{known}, nên HP Screen không nhận được phím này.")),
        suggestion,
    }
}

fn default_hotkey(action: HotkeyAction) -> &'static str {
    use hp_core::settings::*;
    match action {
        HotkeyAction::Toggle => DEFAULT_HOTKEY_TOGGLE,
        HotkeyAction::Pause => DEFAULT_HOTKEY_PAUSE,
        HotkeyAction::Screenshot => DEFAULT_HOTKEY_SCREENSHOT,
        HotkeyAction::Quickshot => DEFAULT_HOTKEY_QUICKSHOT,
        HotkeyAction::Draw => DEFAULT_HOTKEY_DRAW,
    }
}

/// Whether no app holds `hotkey` right now: it is registered for an instant and released.
fn is_free(hotkey: &str) -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey, UnregisterHotKey,
    };
    const PROBE_ID: i32 = 0x4850; // "HP"
    let Ok(h) = hp_core::hotkey::parse_hotkey(hotkey) else { return false };
    let mut mods = MOD_NOREPEAT;
    for (on, flag) in [(h.ctrl, MOD_CONTROL), (h.alt, MOD_ALT), (h.shift, MOD_SHIFT), (h.win, MOD_WIN)] {
        if on {
            mods = HOT_KEY_MODIFIERS(mods.0 | flag.0);
        }
    }
    // SAFETY: a thread hotkey (no window) registered and released at once.
    unsafe {
        let free = RegisterHotKey(None, PROBE_ID, mods, h.vk).is_ok();
        if free {
            let _ = UnregisterHotKey(None, PROBE_ID);
        }
        free
    }
}

/// Hotkeys another app held at startup are tried again now and then: once that app lets go
/// (it quit, or its user changed the key), HP Screen takes the key without a restart.
pub fn watch_taken(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(20));
            let handle = app.clone();
            // Hotkeys are registered on the main thread, like at startup.
            let _ = app.run_on_main_thread(move || retry_taken(&handle));
        }
    });
}

fn retry_taken(app: &AppHandle) {
    let Some(registry) = app.try_state::<HotkeyRegistry>() else { return };
    let Some(state) = app.try_state::<AppState>() else { return };
    let settings = state.settings();
    let mut statuses = registry.0.lock().unwrap_or_else(|e| e.into_inner());
    let mut changed = false;
    for status in statuses.iter_mut().filter(|s| !s.ok) {
        let retried = register_one(app, status.action, &status.hotkey, &settings);
        changed |= retried.ok;
        *status = retried;
    }
    if changed {
        let _ = tauri::Emitter::emit(app, "hotkeys-changed", &*statuses);
    }
}

/// The primary monitor: what a hotkey records when nothing was chosen in the UI yet.
pub fn default_target() -> Target {
    let index = capture::list_monitors().into_iter().find(|m| m.primary).map(|m| m.index).unwrap_or(0);
    Target::Monitor { index }
}

pub fn run_action(app: &AppHandle, action: HotkeyAction) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let before = state.recorder.status().state;
    let settings = state.settings();
    let result = match action {
        HotkeyAction::Toggle => {
            // A press that races the session changing state must not be lost on a technicality:
            // retry briefly, and treat "already stopping/stopped" as success.
            let mut outcome = state.recorder.toggle(default_target(), settings.clone());
            for _ in 0..2 {
                if outcome.is_ok() || matches!(state.recorder.status().state, RecState::Finalizing | RecState::Idle) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(30));
                outcome = state.recorder.toggle(default_target(), settings.clone());
            }
            outcome
        }
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
            // Read the moment at the press: capturing takes a moment of its own.
            let moment = crate::library_fs::recording_moment(&state.recorder);
            let app = app.clone();
            // Capturing takes a moment; keep the hotkey thread free.
            std::thread::spawn(move || match screenshot::take_screenshot(&app, &settings, &target) {
                Ok(path) => {
                    // Taken during a recording: the picture remembers where in the video it belongs.
                    if let Some((video, at_ms)) = moment {
                        crate::library_fs::append_mark(&video, &path, at_ms);
                    }
                    events::library_changed(&app)
                }
                Err(message) => events::rec_error(&app, &message),
            });
            Ok(())
        }
        HotkeyAction::Draw => {
            let app = app.clone();
            // Opening the drawing window must not run on the hotkey (event loop) thread.
            tauri::async_runtime::spawn(async move {
                if let Err(message) = crate::live_cmds::toggle_drawing(app.clone()).await {
                    events::rec_error(&app, &message);
                }
            });
            Ok(())
        }
    };
    // Every press is logged, whether it worked or not: the one report we could not reproduce
    // was "the stop key sometimes does nothing", and this line is how we catch it next time.
    match &result {
        Ok(()) => crate::paths::log_diag(app, &format!("hotkey {action:?} in {before:?}: ok")),
        Err(message) => crate::paths::log_diag(app, &format!("hotkey {action:?} in {before:?}: FAILED {message}")),
    }
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
