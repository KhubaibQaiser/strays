//! The app's standard windows (spec §8): info card, settings, GitHub setup, adoption and welcome.
//! All are served from `ui.html`, routed by the URL fragment.
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::overlay::Rect;

pub const CARD: &str = "card";

/// Opens (or focuses and re-routes) a window. `route` is the fragment, e.g. `settings/github`.
pub fn open(h: &AppHandle, label: &str, route: &str, title: &str, size: (f64, f64)) -> tauri::Result<()> {
    if let Some(w) = h.get_webview_window(label) {
        let _ = w.emit_to(label, "route", route);
        w.show()?;
        w.set_focus()?;
        return Ok(());
    }
    let url = WebviewUrl::App(format!("ui.html#{route}").into());
    WebviewWindowBuilder::new(h, label, url)
        .title(title)
        .inner_size(size.0, size.1)
        .resizable(label != CARD)
        .center()
        .focused(true)
        .build()?
        .set_focus()
}

/// The info card (§8.3): a small floating panel beside the pet. The page closes it on Escape or
/// when it loses focus.
pub fn open_card(h: &AppHandle, pet: &str, near: Option<Rect>) -> tauri::Result<()> {
    if let Some(w) = h.get_webview_window(CARD) {
        w.close()?;
    }
    let (w, ht) = (320.0, 430.0);
    let url = WebviewUrl::App(format!("ui.html#card/{pet}").into());
    let win = WebviewWindowBuilder::new(h, CARD, url)
        .title("")
        .inner_size(w, ht)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .build()?;
    if let Some(r) = near {
        // left of the pet, bottoms aligned; on the right if there's no room on the left
        let x = if r.x - w - 8.0 > 0.0 { r.x - w - 8.0 } else { r.x + r.w + 8.0 };
        let y = (r.y + r.h - ht).max(30.0);
        win.set_position(LogicalPosition::new(x, y))?;
    } else {
        win.center()?;
    }
    win.set_focus()
}

pub fn settings(h: &AppHandle, section: &str) -> tauri::Result<()> {
    open(h, "settings", &format!("settings/{section}"), "Strays Settings", (780.0, 560.0))
}

pub fn setup(h: &AppHandle) -> tauri::Result<()> {
    open(h, "setup", "setup", "Connect GitHub", (520.0, 600.0))
}

/// `query` prefills the project: `repo=<id>` or `folder=<path>` (URL-encoded).
pub fn adopt(h: &AppHandle, query: &str) -> tauri::Result<()> {
    open(h, "adopt", &format!("adopt?{query}"), "Adopt a pet", (560.0, 660.0))
}

pub fn welcome(h: &AppHandle) -> tauri::Result<()> {
    open(h, "welcome", "welcome", "Welcome to Strays", (560.0, 560.0))
}

/// Percent-encodes a value for the window's URL fragment.
pub fn encode(v: &str) -> String {
    v.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}
