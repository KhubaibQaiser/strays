//! The pet overlay (spec §7.1, §7.2): one transparent window per display, pets on its edges, clicks
//! passing through unless ⌥ is held over a pet. Pets start on the main display; one ⌥-dragged onto
//! another display lives there, and comes back to the main one while that display is unplugged.
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use strays_core::pet::Species;
use strays_core::settings::{Liveliness, Modifier};
use strays_core::uuid::Uuid;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, Monitor, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::platform;
use crate::state::PetView;
use strays_core::pet::Home;

const LABEL: &str = "overlay";

/// The overlay window on the `i`th display (the main display is 0).
fn label(i: usize) -> String {
    if i == 0 { LABEL.into() } else { format!("{LABEL}-{i}") }
}

fn is_overlay(label: &str) -> bool {
    label == LABEL || label.starts_with("overlay-")
}

/// Sends an event to the overlay pages only (each display's): hovers and pet moments mean nothing to
/// other windows. A page ignores pets it doesn't show.
pub fn send<R: Runtime, S: Serialize + Clone>(h: &impl Manager<R>, event: &str, payload: S) {
    for (l, w) in h.webview_windows().into_iter().filter(|(l, _)| is_overlay(l)) {
        emit(&w, &l, event, payload.clone());
    }
}

/// Sends an event to one overlay page. Its listeners are its own window's, so others don't hear it.
fn emit<R: Runtime, S: Serialize + Clone>(w: &WebviewWindow<R>, label: &str, event: &str, payload: S) {
    if let Err(e) = w.emit_to(label, event, payload) {
        crate::log::warn(&format!("could not send {event} to {label}: {e}"));
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
    fn contains(&self, (px, py): (f64, f64)) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
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

/// The screen grid pets stand on (§7.2), in window-local points. Cell (0, 0) is the bottom-right
/// ground spot and columns count leftwards, rows upwards; the grid is stretched so its outer cells
/// touch the free area's edges. A pet's canvas (`s` square) stands with its bottom centre on its cell's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Grid {
    /// canvas top-left of cell (0, 0)
    pub x0: f64,
    pub y0: f64,
    /// distance between neighbouring cells
    pub cw: f64,
    pub ch: f64,
    pub cols: u32,
    pub rows: u32,
    pub s: f64,
}

/// How far apart pets stand, as a share of their canvas: about one pet's body.
const CELL: f64 = 0.7;

impl Grid {
    fn new(screen: &Screen, s: f64) -> Self {
        let w = screen.work;
        let span = |room: f64| -> (u32, f64) {
            if room < 0.0 {
                return (0, 0.0);
            }
            let n = (room / (s * CELL)).floor() as u32 + 1;
            (n, if n > 1 { room / (n - 1) as f64 } else { 0.0 })
        };
        let (cols, cw) = span(w.w - 2.0 * MARGIN - s);
        let (rows, ch) = span(w.h - s);
        Grid { x0: w.right() - MARGIN - s - screen.frame.x, y0: w.bottom() - s - screen.frame.y, cw, ch, cols, rows, s }
    }
    /// Canvas top-left of a cell.
    fn at(&self, (c, r): (u32, u32)) -> (f64, f64) {
        (self.x0 - c as f64 * self.cw, self.y0 - r as f64 * self.ch)
    }
    /// The cell nearest a canvas top-left.
    fn cell(&self, (x, y): (f64, f64)) -> (u32, u32) {
        let pick = |d: f64, step: f64, n: u32| if step > 0.0 { (d / step).round().clamp(0.0, (n - 1) as f64) as u32 } else { 0 };
        (pick(self.x0 - x, self.cw, self.cols), pick(self.y0 - y, self.ch, self.rows))
    }
    /// The free cell nearest `to` (or None when every cell is taken).
    fn nearest_free(&self, to: (u32, u32), taken: &[(u32, u32)]) -> Option<(u32, u32)> {
        let d = |c: (u32, u32)| {
            let (dx, dy) = ((c.0 as f64 - to.0 as f64) * self.cw, (c.1 as f64 - to.1 as f64) * self.ch);
            dx * dx + dy * dy
        };
        (0..self.rows).flat_map(|r| (0..self.cols).map(move |c| (c, r))).filter(|c| !taken.contains(c)).min_by(|a, b| d(*a).total_cmp(&d(*b)))
    }
    /// A cell as a home spot: its canvas's bottom centre as a share of the free area.
    fn home(&self, cell: (u32, u32), screen: &Screen) -> Home {
        let (x, y) = self.at(cell);
        let w = screen.work;
        Home { x: (screen.frame.x + x + self.s / 2.0 - w.x) / w.w, y: (screen.frame.y + y + self.s - w.y) / w.h, display: None }
    }
    /// The cell a home spot falls in on this screen.
    fn home_cell(&self, h: &Home, screen: &Screen) -> (u32, u32) {
        let w = screen.work;
        self.cell((w.x + h.x * w.w - self.s / 2.0 - screen.frame.x, w.y + h.y * w.h - self.s - screen.frame.y))
    }
}

/// Places pets on the screen grid (§7.1, §7.2). Pets the user dragged go to their home cell (or the
/// nearest free one); the rest line up along the bottom from the right, keeping out of the middle 60%
/// of the frontmost window when it reaches the bottom, and the parrot perches top right. Pets that
/// don't fit are left off. Results are window-local, in the order of `views`.
pub fn place(views: &[PetView], screen: &Screen, front: Option<Rect>, pet_height: f64) -> Vec<Placed> {
    let g = Grid::new(screen, pet_height / ART_FILL);
    if g.cols == 0 || g.rows == 0 {
        return Vec::new();
    }
    let mut taken: Vec<(u32, u32)> = Vec::new();
    let mut cells: Vec<Option<(u32, u32)>> = vec![None; views.len()];
    for (i, v) in views.iter().enumerate() {
        if let Some(h) = &v.home
            && let Some(c) = g.nearest_free(g.home_cell(h, screen), &taken)
        {
            taken.push(c);
            cells[i] = Some(c);
        }
    }
    // the middle of a front window that reaches down to the pets, in window-local x
    let ground_top = g.y0 + screen.frame.y;
    let avoid = front
        .filter(|f| f.bottom() > ground_top && f.y < screen.work.bottom())
        .map(|f| (f.x + f.w * 0.2 - screen.frame.x, f.x + f.w * 0.8 - screen.frame.x));
    let blocked = |c: u32| avoid.is_some_and(|(a, b)| {
        let x = g.at((c, 0)).0;
        x + g.s * 0.8 > a && x + g.s * 0.2 < b
    });
    let mut perched = false;
    for (i, v) in views.iter().enumerate() {
        if cells[i].is_some() || v.home.is_some() {
            continue;
        }
        let cell = if v.species == Species::Parrot && !perched {
            perched = true;
            g.nearest_free((0, g.rows - 1), &taken)
        } else {
            (0..g.cols).map(|c| (c, 0)).find(|c| !taken.contains(c) && !blocked(c.0))
        };
        if let Some(c) = cell {
            taken.push(c);
            cells[i] = Some(c);
        }
    }
    views
        .iter()
        .zip(cells)
        .filter_map(|(v, c)| c.map(|c| {
            let (x, y) = g.at(c);
            Placed { view: v.clone(), x, y, s: g.s }
        }))
        .collect()
}

fn covers(a: Rect, b: Rect) -> bool {
    a.x <= b.x + 1.0 && a.y <= b.y + 1.0 && a.right() >= b.right() - 1.0 && a.bottom() >= b.bottom() - 1.0
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Frame {
    pub pets: Vec<Placed>,
    /// the grid a dragged pet snaps to, drawn faintly while dragging
    pub grid: Grid,
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

/// One display's window and the pets on it.
struct Display {
    name: String,
    screen: Screen,
    window: WebviewWindow,
    placed: Vec<Placed>,
    /// a full-screen app is in front on this display, or nobody can see the screens
    hidden: bool,
}

impl Display {
    /// A pet's canvas in global points.
    fn global(&self, p: &Placed) -> Rect {
        Rect { x: self.screen.frame.x + p.x, y: self.screen.frame.y + p.y, w: p.s, h: p.s }
    }
}

struct Inner {
    /// the main display first
    displays: Vec<Display>,
    front: Option<Rect>,
    views: Vec<PetView>,
    liveliness: Liveliness,
    reduce_motion: bool,
    pet_height: f64,
    modifier: Modifier,
    hide_full: bool,
    /// the display is asleep or the screen locked: nobody can see the pets
    screen_off: bool,
    /// the pet the mouse is pressed on: clicks keep coming to the overlay until the button is let go
    pressed: Option<Uuid>,
    /// windows for newly plugged-in displays are being made on the main thread
    adding: bool,
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

fn to_screen(m: &Monitor) -> Screen {
    let k = m.scale_factor();
    let (p, sz, wa) = (m.position(), m.size(), m.work_area());
    Screen {
        frame: Rect { x: p.x as f64 / k, y: p.y as f64 / k, w: sz.width as f64 / k, h: sz.height as f64 / k },
        work: Rect { x: wa.position.x as f64 / k, y: wa.position.y as f64 / k, w: wa.size.width as f64 / k, h: wa.size.height as f64 / k },
    }
}

/// Every display by name, the main one (with the menu bar) first. None if they can't be read.
// ponytail: two displays of the same model share a name, so pets homed on either go to the first
fn screens(h: &AppHandle) -> Option<Vec<(String, Screen)>> {
    let (main, all) = match (h.primary_monitor(), h.available_monitors()) {
        (Ok(Some(m)), Ok(all)) => (m, all),
        (Ok(None), _) => return None,
        (Err(e), _) | (_, Err(e)) => {
            crate::log::warn(&format!("could not read the displays: {e}"));
            return None;
        }
    };
    let name = |m: &Monitor, i: usize| m.name().cloned().unwrap_or_else(|| format!("Display {i}"));
    let first = (name(&main, 1), to_screen(&main));
    let main_frame = first.1.frame;
    let rest = all.iter().enumerate().map(|(i, m)| (name(m, i + 2), to_screen(m))).filter(|(_, s)| s.frame != main_frame);
    Some(std::iter::once(first).chain(rest).collect())
}

fn front_rect() -> Option<Rect> {
    // debug-only: measurement runs pin placement so other apps can't move or hide the pets
    if cfg!(debug_assertions) && std::env::var("STRAYS_IGNORE_FRONT").as_deref() == Ok("1") {
        return None;
    }
    platform::frontmost_window().map(|r| Rect { x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height })
}

fn fit(w: &WebviewWindow, s: &Screen) -> tauri::Result<()> {
    w.set_size(LogicalSize::new(s.frame.w, s.frame.h))?;
    w.set_position(LogicalPosition::new(s.frame.x, s.frame.y))
}

/// The overlay window for display `i`. Must run on the main thread (window creation and NSWindow setup).
fn new_window(h: &AppHandle, i: usize, screen: &Screen) -> tauri::Result<WebviewWindow> {
    // debug builds can load another page in the overlay, e.g. proto3d.html to measure 3D pets' CPU
    let page = std::env::var("STRAYS_OVERLAY_PAGE").ok().filter(|_| cfg!(debug_assertions)).unwrap_or_else(|| "index.html".into());
    let window = WebviewWindowBuilder::new(h, label(i), WebviewUrl::App(page.into()))
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
    fit(&window, screen)?;
    window.set_ignore_cursor_events(true)?;
    platform::configure_overlay(window.ns_window()?);
    Ok(window)
}

/// How long the button must read as up, with a press still open, before the poller ends the press
/// itself: longer than the page's own grace for a trackpad press that eases off mid-drag.
const LOST_LET_GO: Duration = Duration::from_millis(700);

impl Overlay {
    /// Must run on the main thread (window creation and NSWindow setup). Displays whose window can't
    /// be made are tried again by the poller.
    pub fn create(h: &AppHandle) -> tauri::Result<Self> {
        let all = screens(h).ok_or(tauri::Error::WindowNotFound)?;
        let mut displays = Vec::new();
        for (i, (name, screen)) in all.into_iter().enumerate() {
            match new_window(h, i, &screen) {
                Ok(window) => displays.push(Display { name, screen, window, placed: Vec::new(), hidden: false }),
                Err(e) if i > 0 => {
                    crate::log::warn(&format!("could not open the pets' window on {name}: {e}"));
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(Self(Arc::new(Mutex::new(Inner {
            displays,
            front: front_rect(),
            views: Vec::new(),
            liveliness: Liveliness::Calm,
            reduce_motion: platform::reduce_motion(),
            pet_height: 104.0,
            modifier: Modifier::Option,
            hide_full: true,
            screen_off: false,
            pressed: None,
            adding: false,
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

    /// Whether pets are hidden somewhere: a full-screen app is in front on a display, or nobody can
    /// see the screens. Sounds and nudges hold off then too.
    pub fn is_full_screen(&self) -> bool {
        self.0.lock().unwrap().displays.iter().any(|d| d.hidden)
    }

    /// Where a pet is drawn, in global points, for placing its info card.
    pub fn pet_rect(&self, id: Uuid) -> Option<Rect> {
        let o = self.0.lock().unwrap();
        o.displays.iter().filter(|d| !d.hidden).find_map(|d| d.placed.iter().find(|p| p.view.id == id).map(|p| d.global(p)))
    }

    fn windows(&self) -> Vec<WebviewWindow> {
        self.0.lock().unwrap().displays.iter().map(|d| d.window.clone()).collect()
    }

    /// The mouse went down on a pet (`true`) or was let go: while it's down the overlay keeps taking
    /// the mouse, wherever the cursor goes, so the pet can be dragged (onto another display too).
    pub fn press(&self, id: Uuid, down: bool) {
        self.0.lock().unwrap().pressed = down.then_some(id);
        if down {
            for w in self.windows() {
                if let Err(e) = w.set_ignore_cursor_events(false) {
                    crate::log::warn(&format!("could not keep the mouse for a drag: {e}"));
                }
            }
        }
    }

    /// Where a pet dropped with its canvas at `at`, local to window `from`, settles: the free grid
    /// cell nearest the drop on the display under the pet's body, as a home spot. None if the pet
    /// isn't shown or every cell there is taken.
    pub fn drop_spot(&self, id: Uuid, from: &str, at: (f64, f64)) -> Option<Home> {
        let o = self.0.lock().unwrap();
        let src = o.displays.iter().find(|d| d.window.label() == from)?;
        let s = src.placed.iter().find(|p| p.view.id == id)?.s;
        let (gx, gy) = (src.screen.frame.x + at.0, src.screen.frame.y + at.1);
        let frames: Vec<Rect> = o.displays.iter().map(|d| d.screen.frame).collect();
        let to = frames.iter().position(|f| f.contains((gx + s * 0.5, gy + s * 0.62))).map_or(src, |i| &o.displays[i]);
        let g = Grid::new(&to.screen, o.pet_height / ART_FILL);
        if g.cols == 0 || g.rows == 0 {
            return None;
        }
        let taken: Vec<_> = to.placed.iter().filter(|p| p.view.id != id).map(|p| g.cell((p.x, p.y))).collect();
        let local = (gx - to.screen.frame.x, gy - to.screen.frame.y);
        g.nearest_free(g.cell(local), &taken).map(|c| Home { display: Some(to.name.clone()), ..g.home(c, &to.screen) })
    }

    /// Re-sends the current frames, for a webview that has just loaded.
    pub fn resend(&self) {
        relayout(&mut self.0.lock().unwrap());
    }

    /// Fits the windows to the displays: moves and resizes the ones there are, closes the ones whose
    /// display went away and opens windows on new displays (on the main thread, then lays out again).
    fn fit_displays(&self, h: &AppHandle, now: Vec<(String, Screen)>, failed: &mut bool) {
        let (old, adding): (Vec<(WebviewWindow, Screen)>, bool) = {
            let o = self.0.lock().unwrap();
            (o.displays.iter().map(|d| (d.window.clone(), d.screen)).collect(), o.adding)
        };
        // resizing waits for the main thread, so not under the lock pet_click takes there
        let mut fitted = Vec::new();
        for (i, (w, s)) in old.iter().enumerate().take(now.len()) {
            let want = now[i].1;
            if want == *s {
                fitted.push(want);
            } else if let Err(e) = fit(w, &want) {
                // keep laying out for the old screen, and try again on the next check
                if !*failed {
                    crate::problem(h, "overlay-size", format!("Couldn't fit the pets to the screen after it changed; trying again: {e}"));
                }
                *failed = true;
                fitted.push(*s);
            } else {
                if *failed {
                    *failed = false;
                    crate::solved(h, "overlay-size");
                }
                fitted.push(want);
            }
        }
        for (w, _) in old.iter().skip(now.len()) {
            if let Err(e) = w.destroy() {
                crate::log::warn(&format!("could not close the pets' window on an unplugged display: {e}"));
            }
        }
        {
            let mut o = self.0.lock().unwrap();
            o.displays.truncate(now.len());
            for (i, d) in o.displays.iter_mut().enumerate() {
                d.screen = fitted[i];
                d.name = now[i].0.clone();
            }
        }
        if now.len() > old.len() && !adding {
            self.0.lock().unwrap().adding = true;
            let (me, h2) = (self.clone(), h.clone());
            let add = now[old.len()..].to_vec();
            let queued = h.run_on_main_thread(move || {
                for (name, screen) in add {
                    let i = me.0.lock().unwrap().displays.len();
                    match new_window(&h2, i, &screen) {
                        Ok(window) => me.0.lock().unwrap().displays.push(Display { name, screen, window, placed: Vec::new(), hidden: false }),
                        Err(e) => {
                            // the poller tries again on its next check
                            crate::log::warn(&format!("could not open the pets' window on {name}: {e}"));
                            break;
                        }
                    }
                }
                let mut o = me.0.lock().unwrap();
                o.adding = false;
                relayout(&mut o);
            });
            if let Err(e) = queued {
                crate::log::warn(&format!("could not open windows on new displays: {e}"));
                self.0.lock().unwrap().adding = false;
            }
        }
    }

    /// Cursor and ⌥ poller (§7.2), plus a slower check of the frontmost window, displays and Reduce Motion.
    pub fn start_poller(&self, h: AppHandle) {
        let me = self.clone();
        thread::spawn(move || {
            let mut hovered: Option<Uuid> = None;
            let mut held = false;
            let (mut click_failed, mut resize_failed) = (false, false);
            let mut was_pressed = false;
            let mut up_since: Option<Instant> = None;
            let mut slow_ticks = 0u32;
            loop {
                let (mx, my) = platform::mouse_location();
                let modifier = me.0.lock().unwrap().modifier;
                let now_held = held_modifier(modifier);
                let (target, near, pressed) = {
                    let mut o = me.0.lock().unwrap();
                    // AppKit's y runs up from the bottom of the main display; ours down from its top
                    let p = (mx, o.displays.first().map_or(0.0, |d| d.screen.frame.y + d.screen.frame.h) - my);
                    // a let-go the page never reported (it can't miss one, but if it did the whole
                    // screen would stop taking clicks): end the press here. The page decides first: a
                    // trackpad press that eases off for a moment reads as up and must not drop the pet.
                    up_since = (o.pressed.is_some() && !platform::mouse_down()).then(|| up_since.unwrap_or_else(Instant::now));
                    if up_since.is_some_and(|t| t.elapsed() >= LOST_LET_GO) {
                        o.pressed = None;
                        up_since = None;
                        for d in &o.displays {
                            emit(&d.window, d.window.label(), "drag-end", ());
                        }
                    }
                    let shown = o.displays.iter().filter(|d| !d.hidden);
                    let target = shown.clone().find_map(|d| d.placed.iter().find(|pl| d.global(pl).hit(p))).map(|pl| pl.view.id).filter(|_| now_held);
                    let near = shown.clone().any(|d| d.placed.iter().any(|pl| d.global(pl).near(p)));
                    (target, near, o.pressed.is_some())
                };
                if target != hovered || now_held != held || pressed != was_pressed {
                    let takes = target.is_some() || pressed;
                    if takes != (hovered.is_some() || was_pressed) {
                        // a stuck switch either blocks clicks on the desktop or makes pets unclickable
                        let r = me.windows().iter().try_for_each(|w| w.set_ignore_cursor_events(!takes));
                        match r {
                            Ok(()) if click_failed => {
                                click_failed = false;
                                crate::solved(&h, "overlay-clicks");
                            }
                            Ok(()) => {}
                            Err(e) => {
                                click_failed = true;
                                crate::problem(&h, "overlay-clicks", format!("Couldn't switch whether pets take clicks; quitting and reopening Strays should fix it: {e}"));
                            }
                        }
                    }
                    send(&h, "hover", Hover { id: target, held: now_held });
                    #[cfg(debug_assertions)]
                    println!("HOVER {:?} held={now_held}", target);
                    hovered = target;
                    held = now_held;
                    was_pressed = pressed;
                }
                slow_ticks += 1;
                if slow_ticks >= 8 {
                    slow_ticks = 0;
                    if let Some(now) = screens(&h) {
                        me.fit_displays(&h, now, &mut resize_failed);
                    }
                    let front = front_rect();
                    let rm = platform::reduce_motion();
                    let off = platform::screen_off();
                    let mut o = me.0.lock().unwrap();
                    o.front = front;
                    o.screen_off = off;
                    let rm_changed = rm != o.reduce_motion;
                    o.reduce_motion = rm;
                    let moved = plan(&o).iter().zip(&o.displays).any(|((next, hidden), d)| *hidden != d.hidden || next.iter().map(|p| (p.x, p.y)).ne(d.placed.iter().map(|p| (p.x, p.y))));
                    // ⌥ held keeps pets still so a click can't land where a pet just left (§7.2)
                    if (moved || rm_changed) && !held && o.pressed.is_none() {
                        relayout(&mut o);
                    }
                }
                thread::sleep(if near || pressed { FAST } else { SLOW });
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

/// Which display each pet goes on: the one its home names if that's plugged in, else the main one.
fn display_of(views: &[PetView], names: &[&str]) -> Vec<usize> {
    views.iter().map(|v| v.home.as_ref().and_then(|h| h.display.as_deref()).and_then(|n| names.iter().position(|d| *d == n)).unwrap_or(0)).collect()
}

/// Every display's pets and whether they're hidden, in display order.
fn plan(o: &Inner) -> Vec<(Vec<Placed>, bool)> {
    let names: Vec<&str> = o.displays.iter().map(|d| d.name.as_str()).collect();
    let on = display_of(&o.views, &names);
    o.displays
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let views: Vec<PetView> = o.views.iter().zip(&on).filter(|(_, k)| **k == i).map(|(v, _)| v.clone()).collect();
            let (placed, hidden) = layout(&views, &d.screen, o.front, o.hide_full, o.pet_height);
            (placed, hidden || o.screen_off)
        })
        .collect()
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
    let next = plan(o);
    let grid_side = o.pet_height / ART_FILL;
    for (d, (placed, hidden)) in o.displays.iter_mut().zip(next) {
        (d.placed, d.hidden) = (placed, hidden);
        #[cfg(debug_assertions)]
        for p in &d.placed {
            println!("LAYOUT {} {} {} {}", p.view.id, (d.screen.frame.x + p.x).round(), (d.screen.frame.y + p.y).round(), p.s.round());
        }
        let frame = Frame { pets: d.placed.clone(), grid: Grid::new(&d.screen, grid_side), liveliness: o.liveliness, reduce_motion: o.reduce_motion, hidden: d.hidden };
        emit(&d.window, d.window.label(), "overlay", frame);
    }
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
        PetView { id: Uuid::new_v4(), species, colors: Default::default(), label: String::new(), mood: Mood::Content, confused: false, liveliness: None, home: None }
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
        // the grid is stretched to fill the width: 12 columns of 1304/11 pt
        assert_eq!(p[1].y, p[0].y);
        assert!((p[0].x - p[1].x - 1304.0 / 11.0).abs() < 1e-9);
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
    fn grid_reaches_every_edge_of_the_free_area() {
        let g = Grid::new(&screen(), 160.0);
        assert_eq!((g.cols, g.rows), (12, 7));
        let (left, top) = g.at((g.cols - 1, g.rows - 1));
        assert!((left - 24.0).abs() < 1e-9 && (top - 33.0).abs() < 1e-9, "top-left cell at {left},{top}");
        // a point between cells picks the nearest, and points off the grid clamp to its edge
        assert_eq!(g.cell(g.at((3, 2))), (3, 2));
        assert_eq!(g.cell((-500.0, -500.0)), (11, 6));
        assert_eq!(g.cell((5000.0, 5000.0)), (0, 0));
    }

    #[test]
    fn dragged_pets_keep_their_cell_and_others_flow_round_them() {
        let g = Grid::new(&screen(), 160.0);
        let home = g.home((0, 0), &screen()); // the first ground spot
        let mut dragged = view(Species::Cat);
        dragged.home = Some(home.clone());
        let p = place(&[view(Species::Dog), dragged.clone()], &screen(), None, 104.0);
        assert_eq!((p[1].x, p[1].y), g.at((0, 0)), "the dragged cat keeps its cell");
        assert_eq!((p[0].x, p[0].y), g.at((1, 0)), "the dog takes the next free ground spot");
        // two pets with the same home: the second gets the nearest free cell
        let p = place(&[dragged.clone(), dragged], &screen(), None, 104.0);
        assert_ne!((p[0].x, p[0].y), (p[1].x, p[1].y));
    }

    #[test]
    fn a_home_survives_a_resolution_change() {
        let mid = Grid::new(&screen(), 160.0);
        let home = mid.home((5, 3), &screen());
        // the same display at a bigger resolution: the pet stays in about the same place on it
        let big = Screen { frame: Rect { x: 0.0, y: 0.0, w: 2056.0, h: 1329.0 }, work: Rect { x: 0.0, y: 33.0, w: 2056.0, h: 1226.0 } };
        let g = Grid::new(&big, 160.0);
        let (x, y) = g.at(g.home_cell(&home, &big));
        let (fx, fy) = ((x + 80.0) / 2056.0, (y + 160.0 - 33.0) / 1226.0);
        assert!((fx - home.x).abs() < 0.05 && (fy - home.y).abs() < 0.05, "moved from {home:?} to {fx},{fy}");
        // and a home is exact on the screen it was made on
        assert_eq!(mid.home_cell(&home, &screen()), (5, 3));
    }

    #[test]
    fn pets_go_to_the_display_their_home_names_or_the_main_one() {
        let home = |display: Option<&str>| {
            let mut v = view(Species::Cat);
            v.home = Some(Home { x: 0.5, y: 0.5, display: display.map(String::from) });
            v
        };
        let v = [view(Species::Dog), home(None), home(Some("LG HDR 4K")), home(Some("Studio Display"))];
        assert_eq!(display_of(&v, &["Built-in Retina Display", "LG HDR 4K"]), [0, 0, 1, 0], "an unplugged display's pets come to the main one");
        assert_eq!(display_of(&v, &["LG HDR 4K", "Built-in Retina Display"]), [0, 0, 0, 0], "the main display changed: still by name");
    }

    #[test]
    fn a_display_is_found_by_the_point_inside_it() {
        // the built-in display, and a 4K one above it and a little to the left (as macOS arranges them)
        let main = Rect { x: 0.0, y: 0.0, w: 1512.0, h: 982.0 };
        let above = Rect { x: -187.0, y: -1080.0, w: 1920.0, h: 1080.0 };
        assert!(main.contains((700.0, 500.0)) && !above.contains((700.0, 500.0)));
        assert!(above.contains((700.0, -300.0)) && !main.contains((700.0, -300.0)));
        assert!(!main.contains((1512.0, 10.0)), "the right edge belongs to the next display");
    }

    #[test]
    fn hit_area_is_the_body_not_the_margin() {
        let r = Rect { x: 0.0, y: 0.0, w: 160.0, h: 160.0 };
        assert!(r.hit((80.0, 99.0)));
        assert!(!r.hit((5.0, 5.0)));
        assert!(!r.hit((150.0, 20.0)));
    }
}
