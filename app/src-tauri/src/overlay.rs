//! The pet overlay (spec §7.1, §7.2): one transparent window on the main display, pets on its edges,
//! clicks passing through unless ⌥ is held over a pet.
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde::Serialize;
use strays_core::pet::Species;
use strays_core::settings::{Liveliness, Modifier};
use strays_core::uuid::Uuid;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::platform;
use crate::state::PetView;

const LABEL: &str = "overlay";

/// Sends an event to the overlay page only: frames, hovers and pet moments mean nothing to other windows.
pub fn send<R: tauri::Runtime, S: Serialize + Clone>(h: &impl Emitter<R>, event: &str, payload: S) {
    if let Err(e) = h.emit_to(LABEL, event, payload) {
        crate::log::warn(&format!("could not send {event} to the overlay: {e}"));
    }
}
/// The cat art fills about 65% of its square canvas (art/cat/scene.rml), so a canvas this much
/// bigger than the pet height shows a pet of that height.
const ART_FILL: f64 = 0.65;
const MARGIN: f64 = 24.0;
const SLOW: Duration = Duration::from_millis(250);
const FAST: Duration = Duration::from_millis(33);
const NEAR: f64 = 150.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    fn right(&self) -> f64 {
        self.x + self.w
    }
    fn bottom(&self) -> f64 {
        self.y + self.h
    }
    fn near(&self, (px, py): (f64, f64)) -> bool {
        px > self.x - NEAR && px < self.right() + NEAR && py > self.y - NEAR && py < self.bottom() + NEAR
    }
    /// The pet's body inside its canvas, not the transparent margin around it.
    fn hit(&self, (px, py): (f64, f64)) -> bool {
        let (cx, cy) = (self.x + self.w * 0.5, self.y + self.h * 0.62);
        let (dx, dy) = ((px - cx) / (self.w * 0.24), (py - cy) / (self.h * 0.34));
        dx * dx + dy * dy <= 1.0
    }
}

/// The display: full frame and the part free of menu bar and Dock, in global top-left points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Screen {
    pub frame: Rect,
    pub work: Rect,
}

#[derive(Clone, Debug, Serialize)]
pub struct Placed {
    #[serde(flatten)]
    pub view: PetView,
    /// Window-local canvas position and side.
    pub x: f64,
    pub y: f64,
    pub s: f64,
}

/// Places pets on the screen edges (§7.1): ground species along the bottom from the right, keeping
/// out of the middle 60% of the frontmost window when it reaches the bottom; the parrot perches top
/// right. Pets that don't fit are left off. All coordinates are global; results are window-local.
pub fn place(views: &[PetView], screen: &Screen, front: Option<Rect>, pet_height: f64) -> Vec<Placed> {
    let s = pet_height / ART_FILL;
    let w = screen.work;
    let ground_y = w.bottom() - s;
    let avoid = front
        .filter(|f| f.bottom() > ground_y && f.y < w.bottom())
        .map(|f| (f.x + f.w * 0.2, f.x + f.w * 0.8));
    let local = |x: f64, y: f64| (x - screen.frame.x, y - screen.frame.y);
    let mut out = Vec::new();
    let mut x = w.right() - MARGIN - s;
    let mut perched = false;
    for v in views {
        if v.species == Species::Parrot && !perched {
            perched = true;
            let (lx, ly) = local(w.right() - MARGIN - s, w.y);
            out.push(Placed { view: v.clone(), x: lx, y: ly, s });
            continue;
        }
        if let Some((a, b)) = avoid
            && x + s * 0.8 > a
            && x + s * 0.2 < b
        {
            x = a - s * 0.8;
        }
        if x < w.x {
            continue;
        }
        let (lx, ly) = local(x, ground_y);
        out.push(Placed { view: v.clone(), x: lx, y: ly, s });
        x -= s * 0.7;
    }
    out
}

fn covers(a: Rect, b: Rect) -> bool {
    a.x <= b.x + 1.0 && a.y <= b.y + 1.0 && a.right() >= b.right() - 1.0 && a.bottom() >= b.bottom() - 1.0
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Frame {
    pub pets: Vec<Placed>,
    pub liveliness: Liveliness,
    pub reduce_motion: bool,
    /// a full-screen app is in front (§7.1): the page keeps its pets but doesn't show or draw them
    pub hidden: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Hover {
    pub id: Option<Uuid>,
    pub held: bool,
}

struct Inner {
    window: WebviewWindow,
    screen: Screen,
    front: Option<Rect>,
    views: Vec<PetView>,
    liveliness: Liveliness,
    reduce_motion: bool,
    pet_height: f64,
    modifier: Modifier,
    hide_full: bool,
    placed: Vec<Placed>,
    hidden: bool,
}

/// The settings the overlay follows (§7.1, §7.2, §7.4).
pub struct Config {
    pub liveliness: Liveliness,
    pub pet_height: f64,
    pub modifier: Modifier,
    pub hide_in_full_screen: bool,
}

#[derive(Clone)]
pub struct Overlay(Arc<Mutex<Inner>>);

fn main_screen(h: &AppHandle) -> Option<Screen> {
    let m = h.primary_monitor().ok()??;
    let k = m.scale_factor();
    let (p, sz, wa) = (m.position(), m.size(), m.work_area());
    Some(Screen {
        frame: Rect { x: p.x as f64 / k, y: p.y as f64 / k, w: sz.width as f64 / k, h: sz.height as f64 / k },
        work: Rect { x: wa.position.x as f64 / k, y: wa.position.y as f64 / k, w: wa.size.width as f64 / k, h: wa.size.height as f64 / k },
    })
}

fn front_rect() -> Option<Rect> {
    // debug-only: measurement runs pin placement so other apps can't move or hide the pets
    if cfg!(debug_assertions) && std::env::var("STRAYS_IGNORE_FRONT").as_deref() == Ok("1") {
        return None;
    }
    platform::frontmost_window().map(|r| Rect { x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height })
}

impl Overlay {
    /// Must run on the main thread (window creation and NSWindow setup).
    pub fn create(h: &AppHandle) -> tauri::Result<Self> {
        let screen = main_screen(h).ok_or(tauri::Error::WindowNotFound)?;
        // debug builds can load another page in the overlay, e.g. proto3d.html to measure 3D pets' CPU
        let page = std::env::var("STRAYS_OVERLAY_PAGE").ok().filter(|_| cfg!(debug_assertions)).unwrap_or_else(|| "index.html".into());
        let window = WebviewWindowBuilder::new(h, LABEL, WebviewUrl::App(page.into()))
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .resizable(false)
            .focused(false)
            .visible_on_all_workspaces(true)
            .accept_first_mouse(true)
            .build()?;
        window.set_size(LogicalSize::new(screen.frame.w, screen.frame.h))?;
        window.set_position(LogicalPosition::new(screen.frame.x, screen.frame.y))?;
        window.set_ignore_cursor_events(true)?;
        platform::configure_overlay(window.ns_window()?);
        Ok(Self(Arc::new(Mutex::new(Inner {
            window,
            screen,
            front: front_rect(),
            views: Vec::new(),
            liveliness: Liveliness::Calm,
            reduce_motion: platform::reduce_motion(),
            pet_height: 104.0,
            modifier: Modifier::Option,
            hide_full: true,
            placed: Vec::new(),
            hidden: false,
        }))))
    }

    pub fn set_pets(&self, views: Vec<PetView>, c: Config) {
        let mut o = self.0.lock().unwrap();
        o.views = views;
        o.liveliness = c.liveliness;
        o.pet_height = c.pet_height;
        o.modifier = c.modifier;
        o.hide_full = c.hide_in_full_screen;
        relayout(&mut o);
    }

    /// Whether pets are hidden because an app is full screen.
    pub fn is_full_screen(&self) -> bool {
        self.0.lock().unwrap().hidden
    }

    /// Where a pet is drawn, in global points, for placing its info card.
    pub fn pet_rect(&self, id: Uuid) -> Option<Rect> {
        let o = self.0.lock().unwrap();
        o.placed.iter().filter(|_| !o.hidden).find(|p| p.view.id == id).map(|p| Rect { x: o.screen.frame.x + p.x, y: o.screen.frame.y + p.y, w: p.s, h: p.s })
    }

    /// Re-sends the current frame, for a webview that has just loaded.
    pub fn resend(&self) {
        relayout(&mut self.0.lock().unwrap());
    }

    pub fn window(&self) -> WebviewWindow {
        self.0.lock().unwrap().window.clone()
    }

    /// Cursor and ⌥ poller (§7.2), plus a slower check of the frontmost window, display and Reduce Motion.
    pub fn start_poller(&self, h: AppHandle) {
        let me = self.clone();
        thread::spawn(move || {
            let mut hovered: Option<Uuid> = None;
            let mut held = false;
            let mut slow_ticks = 0u32;
            loop {
                let primary_h = me.0.lock().unwrap().screen.frame.h;
                let (mx, my) = platform::mouse_location();
                let p = (mx, primary_h - my);
                let modifier = me.0.lock().unwrap().modifier;
                let now_held = held_modifier(modifier);
                let (target, near) = {
                    let o = me.0.lock().unwrap();
                    let global = |pl: &Placed| Rect { x: o.screen.frame.x + pl.x, y: o.screen.frame.y + pl.y, w: pl.s, h: pl.s };
                    let target = o.placed.iter().filter(|_| !o.hidden).find(|pl| global(pl).hit(p)).map(|pl| pl.view.id).filter(|_| now_held);
                    (target, o.placed.iter().any(|pl| global(pl).near(p)))
                };
                if target != hovered || now_held != held {
                    let w = me.window();
                    if target.is_some() != hovered.is_some()
                        && let Err(e) = w.set_ignore_cursor_events(target.is_none())
                    {
                        crate::log::warn(&format!("could not switch the overlay's click-through: {e}"));
                    }
                    send(&w, "hover", Hover { id: target, held: now_held });
                    #[cfg(debug_assertions)]
                    println!("HOVER {:?} held={now_held}", target);
                    hovered = target;
                    held = now_held;
                }
                slow_ticks += 1;
                if slow_ticks >= 8 {
                    slow_ticks = 0;
                    let screen = main_screen(&h);
                    let front = front_rect();
                    let rm = platform::reduce_motion();
                    let (old, w) = {
                        let o = me.0.lock().unwrap();
                        (o.screen, o.window.clone())
                    };
                    let screen = screen.unwrap_or(old);
                    // resizing waits for the main thread, so not under the lock pet_click takes there
                    if screen != old
                        && let Err(e) = w.set_size(LogicalSize::new(screen.frame.w, screen.frame.h)).and_then(|_| w.set_position(LogicalPosition::new(screen.frame.x, screen.frame.y)))
                    {
                        crate::log::warn(&format!("could not fit the overlay to the screen: {e}"));
                    }
                    let mut o = me.0.lock().unwrap();
                    if screen != o.screen || front != o.front || rm != o.reduce_motion {
                        let (next, hidden) = layout(&o.views, &screen, front, o.hide_full, o.pet_height);
                        let moved = hidden != o.hidden || next.iter().map(|p| (p.x, p.y)).ne(o.placed.iter().map(|p| (p.x, p.y)));
                        o.screen = screen;
                        o.front = front;
                        let rm_changed = rm != o.reduce_motion;
                        o.reduce_motion = rm;
                        // ⌥ held keeps pets still so a click can't land where a pet just left (§7.2)
                        if (moved || rm_changed) && !held {
                            relayout(&mut o);
                        }
                    }
                }
                thread::sleep(if near { FAST } else { SLOW });
            }
        });
    }
}

/// Where the pets go, and whether they're hidden: a full-screen app hides them (unless "hide over
/// full-screen apps" is off) but never moves them, so they come back exactly where they were.
fn layout(views: &[PetView], screen: &Screen, front: Option<Rect>, hide_full: bool, pet_height: f64) -> (Vec<Placed>, bool) {
    let full = front.is_some_and(|f| covers(f, screen.frame));
    (place(views, screen, front.filter(|_| !full), pet_height), full && hide_full)
}

/// Whether the chosen modifier is down (⌥⇧ needs both). Extra keys are allowed: ⇧ on top of the
/// modifier is the quick-sync click (§7.3).
fn held_modifier(m: Modifier) -> bool {
    use objc2_app_kit::NSEventModifierFlags as F;
    let f = platform::modifier_flags();
    match m {
        Modifier::Option => f.contains(F::Option),
        Modifier::Control => f.contains(F::Control),
        Modifier::Shift => f.contains(F::Shift),
        Modifier::OptionShift => f.contains(F::Option) && f.contains(F::Shift),
    }
}

fn relayout(o: &mut Inner) {
    (o.placed, o.hidden) = layout(&o.views, &o.screen, o.front, o.hide_full, o.pet_height);
    #[cfg(debug_assertions)]
    for p in &o.placed {
        println!("LAYOUT {} {} {} {}", p.view.id, (o.screen.frame.x + p.x).round(), (o.screen.frame.y + p.y).round(), p.s.round());
    }
    let frame = Frame { pets: o.placed.clone(), liveliness: o.liveliness, reduce_motion: o.reduce_motion, hidden: o.hidden };
    send(&o.window, "overlay", frame);
}

#[cfg(test)]
mod tests {
    /// Overlay events go to the overlay only (`send`); a plain `.emit(` would broadcast them to every window.
    #[test]
    fn overlay_events_are_never_broadcast() {
        for (file, src) in [("overlay.rs", include_str!("overlay.rs")), ("lib.rs", include_str!("lib.rs"))] {
            for event in ["overlay", "hover", "cue", "nudge", "feedback"] {
                assert!(!src.contains(&format!(".emit(\"{event}\"")), "{file} broadcasts {event}");
            }
        }
    }

    use super::*;
    use strays_core::mood::Mood;

    fn view(species: Species) -> PetView {
        PetView { id: Uuid::new_v4(), species, colors: Default::default(), label: String::new(), mood: Mood::Content, confused: false, liveliness: None }
    }

    fn screen() -> Screen {
        // 1512×982 with a 33 pt menu bar and a 70 pt Dock
        Screen { frame: Rect { x: 0.0, y: 0.0, w: 1512.0, h: 982.0 }, work: Rect { x: 0.0, y: 33.0, w: 1512.0, h: 879.0 } }
    }

    #[test]
    fn ground_pets_line_up_from_the_right_above_the_dock() {
        let v = [view(Species::Cat), view(Species::Dog)];
        let p = place(&v, &screen(), None, 104.0);
        let s = 160.0;
        assert_eq!(p.len(), 2);
        assert_eq!((p[0].x, p[0].y, p[0].s), (1512.0 - 24.0 - s, 912.0 - s, s));
        assert_eq!(p[1].x, p[0].x - s * 0.7);
    }

    #[test]
    fn parrot_perches_top_right_under_the_menu_bar() {
        let v = [view(Species::Parrot), view(Species::Cat)];
        let p = place(&v, &screen(), None, 104.0);
        assert_eq!(p[0].y, 33.0);
        assert_eq!(p[1].y, 912.0 - 160.0);
        assert_eq!(p[1].x, p[0].x, "the cat takes the first ground spot");
    }

    #[test]
    fn keeps_out_of_the_frontmost_window_middle() {
        let v = [view(Species::Cat), view(Species::Cat)];
        let front = Rect { x: 900.0, y: 100.0, w: 600.0, h: 812.0 }; // reaches the bottom; middle = 1020..1380
        let p = place(&v, &screen(), Some(front), 104.0);
        for pl in &p {
            let (a, b) = (pl.x + pl.s * 0.2, pl.x + pl.s * 0.8);
            assert!(b <= 1020.0 || a >= 1380.0, "pet body {a}..{b} overlaps the window middle");
        }
        // a window that stops above the pets doesn't push them
        let high = Rect { x: 900.0, y: 100.0, w: 600.0, h: 300.0 };
        assert_eq!(place(&v, &screen(), Some(high), 104.0)[0].x, 1512.0 - 24.0 - 160.0);
    }

    #[test]
    fn full_screen_app_hides_pets_and_full_rows_drop_extras() {
        let v = [view(Species::Cat)];
        let (kept, hidden) = layout(&v, &screen(), Some(screen().frame), true, 104.0);
        assert!(hidden && kept.len() == 1, "hidden, not removed");
        assert_eq!(kept[0].x, place(&v, &screen(), None, 104.0)[0].x, "and not moved either");
        assert!(!layout(&v, &screen(), Some(screen().frame), false, 104.0).1, "unless that setting is off");
        let many: Vec<_> = (0..20).map(|_| view(Species::Cat)).collect();
        let p = place(&many, &screen(), None, 104.0);
        assert!(p.len() < 20 && p.iter().all(|pl| pl.x >= 0.0));
    }

    #[test]
    fn hit_area_is_the_body_not_the_margin() {
        let r = Rect { x: 0.0, y: 0.0, w: 160.0, h: 160.0 };
        assert!(r.hit((80.0, 99.0)));
        assert!(!r.hit((5.0, 5.0)));
        assert!(!r.hit((150.0, 20.0)));
    }
}
