// Gate 0 overlay prototype: transparent always-on-top pet windows that pass clicks through
// unless the modifier (⌥) is held while the cursor is over a pet.
//
// STRAYS_MODE=A (default): one small window per pet.
// STRAYS_MODE=B: one window covering the main screen, pets drawn inside it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, PhysicalPosition, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

const PET_COUNT: usize = 3;
const PET_SIZE: f64 = 160.0; // logical points; the cat art sits in the lower part
const SLOW_POLL: Duration = Duration::from_millis(250);
const FAST_POLL: Duration = Duration::from_millis(33);
const NEAR_DISTANCE: f64 = 150.0;

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    // Hit area: the pet's body inside its square, not the transparent margin around it.
    fn hit(&self, px: f64, py: f64) -> bool {
        let (cx, cy) = (self.x + self.w * 0.5, self.y + self.h * 0.6);
        let (rx, ry) = (self.w * 0.38, self.h * 0.38);
        let (dx, dy) = ((px - cx) / rx, (py - cy) / ry);
        dx * dx + dy * dy <= 1.0
    }
    fn near(&self, px: f64, py: f64) -> bool {
        px > self.x - NEAR_DISTANCE
            && px < self.x + self.w + NEAR_DISTANCE
            && py > self.y - NEAR_DISTANCE
            && py < self.y + self.h + NEAR_DISTANCE
    }
}

#[cfg(target_os = "macos")]
fn modifier_held() -> bool {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags};
    // Reading the current modifier state needs no Accessibility or Input Monitoring permission.
    let flags = NSEvent::modifierFlags_class();
    flags.contains(NSEventModifierFlags::Option)
}

/// Cursor in logical points, top-left origin. AppKit reports bottom-left, so flip by the primary screen height.
#[cfg(target_os = "macos")]
fn cursor(primary_height: f64) -> Option<(f64, f64)> {
    let p = objc2_app_kit::NSEvent::mouseLocation();
    Some((p.x, primary_height - p.y))
}

#[cfg(not(target_os = "macos"))]
fn modifier_held() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
fn cursor(_primary_height: f64) -> Option<(f64, f64)> {
    None // ponytail: Windows poller comes with the Windows port
}

// Prototype knobs for the Gate 0 measurements: STRAYS_FPS, STRAYS_PAUSED=1, STRAYS_BURST=<seconds>.
fn query() -> String {
    let var = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.into());
    format!("fps={}&paused={}&burst={}", var("STRAYS_FPS", "30"), var("STRAYS_PAUSED", "0"), var("STRAYS_BURST", ""))
}

fn base_window(app: &AppHandle, label: &str, url: &str) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .focused(false)
        .visible_on_all_workspaces(true)
        .accept_first_mouse(true)
        .build()
}

/// Pet rects in logical screen points (top-left origin), sitting on the bottom of the work area.
fn pet_rects(app: &AppHandle) -> Vec<Rect> {
    let monitor = app.primary_monitor().ok().flatten().expect("no primary monitor");
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (ax, ay) = (area.position.x as f64 / scale, area.position.y as f64 / scale);
    let (aw, ah) = (area.size.width as f64 / scale, area.size.height as f64 / scale);
    (0..PET_COUNT)
        .map(|i| Rect {
            x: ax + aw - (PET_SIZE + 40.0) * (i as f64 + 1.0),
            y: ay + ah - PET_SIZE,
            w: PET_SIZE,
            h: PET_SIZE,
        })
        .collect()
}

fn setup_mode_a(app: &AppHandle, rects: &[Rect]) -> tauri::Result<Vec<WebviewWindow>> {
    rects
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let w = base_window(app, &format!("pet{i}"), &format!("index.html?mode=A&pet={i}&{}", query()))?;
            w.set_size(LogicalSize::new(r.w, r.h))?;
            w.set_position(LogicalPosition::new(r.x, r.y))?;
            w.set_ignore_cursor_events(true)?;
            Ok(w)
        })
        .collect()
}

fn setup_mode_b(app: &AppHandle, rects: &[Rect]) -> tauri::Result<Vec<WebviewWindow>> {
    let monitor = app.primary_monitor()?.expect("no primary monitor");
    let scale = monitor.scale_factor();
    let size = monitor.size();
    let pos = monitor.position();
    let pets: Vec<String> = rects
        .iter()
        .map(|r| format!("{},{},{},{}", r.x - pos.x as f64 / scale, r.y - pos.y as f64 / scale, r.w, r.h))
        .collect();
    let w = base_window(app, "overlay", &format!("index.html?mode=B&{}&pets={}", query(), pets.join(";")))?;
    w.set_size(LogicalSize::new(size.width as f64 / scale, size.height as f64 / scale))?;
    w.set_position(PhysicalPosition::new(pos.x, pos.y))?;
    w.set_ignore_cursor_events(true)?;
    Ok(vec![w])
}

/// Poll cursor + modifier; make a window catch clicks only while ⌥ is held over a pet.
fn start_poller(app: AppHandle, windows: Vec<WebviewWindow>, rects: Vec<Rect>, mode_b: bool) {
    thread::spawn(move || {
        let interactive: Vec<Arc<AtomicBool>> =
            windows.iter().map(|_| Arc::new(AtomicBool::new(false))).collect();
        let mut hovered: Option<usize> = None;
        // Read once: querying Tauri's monitor/cursor APIs every tick cost ~3% CPU in Gate 0.
        let primary_height = app
            .primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.size().height as f64 / m.scale_factor())
            .unwrap_or(0.0);
        loop {
            let Some((px, py)) = cursor(primary_height) else {
                thread::sleep(SLOW_POLL);
                continue;
            };
            let held = modifier_held();
            let over = rects.iter().position(|r| r.hit(px, py));
            let target = if held { over } else { None };

            if target != hovered {
                if let Some(i) = hovered {
                    let _ = app.emit("pet-hover", (i, false));
                }
                if let Some(i) = target {
                    let _ = app.emit("pet-hover", (i, true));
                }
                println!("HOVER {:?}", target);
                hovered = target;
            }

            for (wi, w) in windows.iter().enumerate() {
                let want = if mode_b { target.is_some() } else { target == Some(wi) };
                if interactive[wi].swap(want, Ordering::Relaxed) != want {
                    let _ = w.set_ignore_cursor_events(!want);
                }
            }

            let near = rects.iter().any(|r| r.near(px, py));
            thread::sleep(if near { FAST_POLL } else { SLOW_POLL });
        }
    });
}

#[tauri::command]
fn pet_clicked(pet: usize) {
    println!("CLICK pet={pet}");
}

// Prototype-only: lets the webview report to stdout so tests can read it.
#[tauri::command]
fn pet_log(msg: String) {
    println!("{msg}");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![pet_clicked, pet_log])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let rects = pet_rects(&handle);
            let mode_b = std::env::var("STRAYS_MODE").as_deref() == Ok("B");
            let windows = if mode_b {
                setup_mode_b(&handle, &rects)?
            } else {
                setup_mode_a(&handle, &rects)?
            };
            let rect_list: Vec<String> =
                rects.iter().map(|r| format!("{},{},{},{}", r.x, r.y, r.w, r.h)).collect();
            println!("READY mode={} rects={}", if mode_b { "B" } else { "A" }, rect_list.join(";"));
            start_poller(handle, windows, rects, mode_b);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
