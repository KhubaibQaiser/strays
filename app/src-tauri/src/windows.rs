//! The app's standard windows (spec §8): info card, settings, GitHub setup, adoption and welcome.
//! All are served from `ui.html`, routed by the URL fragment.
use tauri::{AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::overlay::Rect;

pub const CARD: &str = "card";

/// Opens (or focuses and re-routes) a window. `route` is the fragment, e.g. `settings/github`.
pub fn open(h: &AppHandle, label: &str, route: &str, title: &str, size: (f64, f64)) -> tauri::Result<()> {
    if let Some(w) = h.get_webview_window(label) {
        if let Err(e) = w.emit_to(label, "route", route) {
            crate::log::warn(&format!("could not show {route} in the open {label} window: {e}"));
        }
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
    let (w, ht) = (320.0, 480.0);
    let url = WebviewUrl::App(format!("ui.html#card/{pet}").into());
    let win = WebviewWindowBuilder::new(h, CARD, url)
        .title("")
        .inner_size(w, ht)
        .resizable(false)
        .decorations(false)
        // see-through, so the page's rounded card is the window's shape (no square corners behind it)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .build()?;
    if let Some(r) = near {
        // the display the pet is on, which may not be the one the card opened on
        let frame = |m: &tauri::Monitor| {
            let (p, s, k) = (m.position(), m.size(), m.scale_factor());
            Rect { x: p.x as f64 / k, y: p.y as f64 / k, w: s.width as f64 / k, h: s.height as f64 / k }
        };
        let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let under = win.available_monitors()?.iter().map(frame).find(|f| cx >= f.x && cx < f.x + f.w && cy >= f.y && cy < f.y + f.h);
        let screen = under.or(win.current_monitor()?.as_ref().map(frame));
        let (x, y) = card_position(r, (w, ht), screen);
        win.set_position(LogicalPosition::new(x, y))?;
    } else {
        win.center()?;
    }
    win.set_focus()
}

/// Left of the pet with bottoms aligned, or right of it when there's no room on the left; then kept
/// inside the screen (below the menu bar), so a pet at an edge never pushes its card off it.
fn card_position(pet: Rect, (w, h): (f64, f64), screen: Option<Rect>) -> (f64, f64) {
    let s = screen.unwrap_or(Rect { x: 0.0, y: 0.0, w: f64::MAX / 4.0, h: f64::MAX / 4.0 });
    let x = if pet.x - w - 8.0 > s.x { pet.x - w - 8.0 } else { pet.x + pet.w + 8.0 };
    let y = pet.y + pet.h - h;
    let fit = |v: f64, lo: f64, hi: f64| v.min(hi).max(lo);
    (fit(x, s.x + 8.0, s.x + s.w - w - 8.0), fit(y, s.y + 30.0, s.y + s.h - h - 8.0))
}

#[cfg(test)]
#[test]
fn the_card_stays_on_screen() {
    let screen = Some(Rect { x: 0.0, y: 0.0, w: 1440.0, h: 900.0 });
    let pet = |x, y| Rect { x, y, w: 100.0, h: 100.0 };
    assert_eq!(card_position(pet(800.0, 700.0), (320.0, 430.0), screen), (472.0, 370.0), "left of the pet, bottoms aligned");
    assert_eq!(card_position(pet(100.0, 700.0), (320.0, 430.0), screen), (208.0, 370.0), "no room on the left: on the right");
    // a parrot perched top right, and a pet wider than the gap at the right edge
    assert_eq!(card_position(pet(1340.0, 40.0), (320.0, 430.0), screen), (1012.0, 30.0));
    let (x, _) = card_position(Rect { x: 200.0, y: 700.0, w: 1200.0, h: 100.0 }, (320.0, 430.0), screen);
    assert_eq!(x, 1440.0 - 320.0 - 8.0, "never past the right edge");
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
