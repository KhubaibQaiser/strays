//! The global shortcut that hides or shows every pet (spec §6.3, Settings → Interaction; off by default).
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Builder, GlobalShortcutExt, Shortcut, ShortcutState};

use crate::Shared;
use crate::sync::Cmd;

pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    Builder::new()
        .with_handler(|h, _, event| {
            if event.state == ShortcutState::Pressed {
                toggle(h);
            }
        })
        .build()
}

/// "Hide all pets", flipped: the same switch as the menu's.
fn toggle(h: &AppHandle) {
    let Some(shared) = h.try_state::<Shared>() else { return };
    let mut app = shared.app.lock().unwrap();
    app.flags.hide_all = !app.flags.hide_all;
    drop(app);
    let _ = shared.tx.send(Cmd::Changed);
}

/// Registers `keys` (or nothing, when None) in place of whatever was registered before.
pub fn apply(h: &AppHandle, keys: Option<&str>) -> Result<(), String> {
    let gs = h.global_shortcut();
    gs.unregister_all().map_err(|e| format!("Couldn't clear the old shortcut: {e}"))?;
    let Some(keys) = keys else { return Ok(()) };
    let shortcut: Shortcut = keys.parse().map_err(|e| format!("{keys} isn't a shortcut Strays can use ({e})"))?;
    // taken by another app, or reserved by macOS: say so rather than silently doing nothing
    gs.register(shortcut).map_err(|e| format!("Couldn't use {keys} as the shortcut, maybe another app has it ({e})"))
}
