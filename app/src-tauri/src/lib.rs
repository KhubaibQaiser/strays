//! Strays app shell: the core's state, a background sync loop, the menu-bar menu and the pet overlay.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

mod audio;
mod commands;
mod log;
mod menu;
mod overlay;
mod report;
mod state;
mod sync;
mod token;
mod windows;

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use strays_core::chrono::Utc;
use strays_core::uuid::Uuid;
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::overlay::Overlay;
use crate::state::App;
use crate::sync::Cmd;

struct Shared {
    app: Arc<Mutex<App>>,
    tx: Sender<Cmd>,
    overlay: Overlay,
    audio: audio::Player,
}

const TRAY: &str = "strays";

#[derive(Clone, Serialize)]
struct Cue {
    id: Uuid,
    cue: state::Cue,
}

#[derive(Clone, Serialize)]
struct Feedback {
    id: Uuid,
    ok: bool,
    text: String,
}

/// A click on a pet with the chosen modifier held: with ⇧ added it quick-syncs (§7.3), otherwise its
/// info card opens beside it (§8.3). When the modifier itself includes ⇧, every click opens the card.
#[tauri::command]
fn pet_click(id: Uuid, shift: bool, shared: State<'_, Shared>, h: AppHandle) {
    #[cfg(debug_assertions)]
    println!("CLICK pet={id} shift={shift}");
    let mut app = shared.app.lock().unwrap();
    if quick_sync(app.settings.modifier, shift) {
        drop(app);
        let _ = shared.tx.send(Cmd::SyncNow(Some(id)));
        return;
    }
    if audio::allowed(&app.settings, None, app.flags.presentation, shared.overlay.is_full_screen(), state::local_minute()) {
        shared.audio.play(audio::Sound::Tick, app.settings.volume);
    }
    match windows::open_card(&h, &id.to_string(), shared.overlay.pet_rect(id)) {
        Ok(()) => app.resolve("window"),
        Err(e) => {
            app.warn("window", format!("Couldn't open the pet's card: {e}"));
            drop(app);
            let _ = shared.tx.send(Cmd::Changed);
        }
    }
}

fn quick_sync(modifier: strays_core::settings::Modifier, shift: bool) -> bool {
    use strays_core::settings::Modifier;
    shift && !matches!(modifier, Modifier::Shift | Modifier::OptionShift)
}

#[cfg(test)]
#[test]
fn shift_quick_syncs_only_when_it_isnt_part_of_the_modifier() {
    use strays_core::settings::Modifier;
    assert!(quick_sync(Modifier::Option, true));
    assert!(quick_sync(Modifier::Control, true));
    assert!(!quick_sync(Modifier::Option, false));
    assert!(!quick_sync(Modifier::Shift, true), "⇧-click is the plain click here");
    assert!(!quick_sync(Modifier::OptionShift, true));
}

/// The webview asks for the current frame once it has loaded.
#[tauri::command]
fn overlay_ready(shared: State<'_, Shared>) {
    #[cfg(debug_assertions)]
    println!("OVERLAY READY");
    shared.overlay.resend();
}

/// Debug builds only: lets the webview report to stdout for the OS test harness.
#[tauri::command]
fn pet_log(msg: String) {
    #[cfg(debug_assertions)]
    println!("{msg}");
    #[cfg(not(debug_assertions))]
    let _ = msg;
}

fn data_dir(h: &AppHandle) -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("STRAYS_DATA_DIR") {
        return dir.into();
    }
    h.path().app_data_dir().expect("app data dir")
}

/// Debug only: `STRAYS_SPAWN=n` adds n local cats (not saved) so the overlay can be tested without GitHub.
#[cfg(debug_assertions)]
fn spawn_test_pets(app: &mut App) {
    use strays_core::pet::{Activity, ActivitySource, LocalRef, Pet, Project, Species};
    let Some(n) = std::env::var("STRAYS_SPAWN").ok().and_then(|v| v.parse::<usize>().ok()) else { return };
    let now = Utc::now();
    let looks = [["#EFE1CC", "#D9C4A6", "#FFF8EE", "#7A5C46"], ["#34343A", "#26262B", "#55555E", "#A9A9B6"], ["#E8A15C", "#CB8240", "#FBE3C6", "#7A4520"]];
    for i in 0..n {
        let local = LocalRef { bookmark_id: Uuid::new_v4(), display_path: format!("~/Code/test-{i}"), path: String::new() };
        let project = Project { github: None, local: Some(local) };
        let mut p = Pet::new(now, &format!("Test {i}"), Species::Cat, project, Activity { last: now, source: ActivitySource::LocalCommit }, app.settings.default_pace);
        for (name, hex) in ["coatBase", "coatShade", "coatHighlight", "outline"].iter().zip(looks[i % 3]) {
            p.colors.insert((*name).into(), hex.into());
        }
        p.adopt(now).expect("new pet");
        app.pets.push(p);
    }
}

/// Main thread only: rebuilds the menu-bar title and menu from the current state.
fn refresh_tray(h: &AppHandle) {
    let Some(tray) = h.tray_by_id(TRAY) else { return };
    let shared = h.state::<Shared>();
    let app = shared.app.lock().unwrap();
    // the tray is where problems are shown, so its own failures can only go to the log
    if let Err(e) = tray.set_title(Some(menu::tray_title(&app))) {
        log::warn(&format!("could not update the menu-bar title: {e}"));
    }
    match menu::build(h, &app).and_then(|m| tray.set_menu(Some(m))) {
        Ok(()) => {}
        Err(e) => log::warn(&format!("could not rebuild the menu: {e}")),
    }
}

/// Opens a window from the menu; a failure is shown in the menu, as the menu is where it was asked for.
fn open_from_menu(h: &AppHandle, opened: tauri::Result<()>) {
    let shared = h.state::<Shared>();
    let mut app = shared.app.lock().unwrap();
    match opened {
        Ok(()) => app.resolve("window"),
        Err(e) => app.warn("window", format!("Couldn't open the window: {e}")),
    }
    drop(app);
    let _ = shared.tx.send(Cmd::Changed);
}

fn handle_menu(h: &AppHandle, id: &str) {
    let shared = h.state::<Shared>();
    let Some(event) = menu::parse(id) else { return };
    let outcome = menu::apply(&mut shared.app.lock().unwrap(), event, Utc::now());
    match outcome {
        menu::Outcome::Done => {}
        menu::Outcome::Sync(pet) => drop(shared.tx.send(Cmd::SyncNow(pet))),
        menu::Outcome::Open(w) => open_from_menu(
            h,
            match w {
                menu::Window::Settings(section) => windows::settings(h, section),
                menu::Window::Setup => windows::setup(h),
                menu::Window::Adopt(query) => windows::adopt(h, &query),
            },
        ),
        menu::Outcome::Quit => return h.exit(0),
    }
    let _ = shared.tx.send(Cmd::Changed);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            pet_click,
            overlay_ready,
            pet_log,
            commands::ui_state,
            commands::pet_action,
            commands::set_goal,
            commands::set_pet,
            commands::adopt,
            commands::ignore_repo,
            commands::save_settings,
            commands::set_flag,
            commands::sync_now,
            commands::dismiss_issues,
            commands::test_sound,
            commands::set_login_item,
            commands::token_check,
            commands::token_save,
            commands::token_disconnect,
            commands::pick_folder,
            commands::open_github,
            commands::reveal,
            commands::netlog,
            commands::open_link,
            commands::open_data_folder,
            commands::reveal_log,
            commands::export_data,
            commands::delete_everything,
            commands::open_window,
            commands::close_window,
        ])
        .on_menu_event(|h, e| handle_menu(h, e.id().as_ref()))
        .setup(|tauri_app| {
            #[cfg(target_os = "macos")]
            tauri_app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let h = tauri_app.handle().clone();

            let dir = data_dir(&h);
            // the log is best-effort: without it problems still show in the menu and Settings
            if let Err(e) = log::init(&dir) {
                eprintln!("no log file: {e}");
            }
            report::install_crash_hook(dir.clone());
            #[allow(unused_mut)]
            let mut app = App::load(dir).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            #[cfg(target_os = "macos")]
            app.open_bookmarks(platform::open_bookmark);
            #[cfg(debug_assertions)]
            spawn_test_pets(&mut app);
            let first_run = commands::snapshot(&app).first_run;
            let menu = menu::build(&h, &app)?;
            // the paw is a template image (black + alpha, 44 px), rendered by art/icons/render.sh
            let paw = tauri::image::Image::new(include_bytes!("../icons/tray.rgba"), 44, 44);
            TrayIconBuilder::with_id(TRAY).icon(paw).icon_as_template(true).title(menu::tray_title(&app)).tooltip("Strays").menu(&menu).show_menu_on_left_click(true).build(&h)?;

            let overlay = Overlay::create(&h)?;
            overlay.start_poller(h.clone());
            let app = Arc::new(Mutex::new(app));
            let (tx, rx) = mpsc::channel();
            let player = audio::Player::start();
            h.manage(Shared { app: app.clone(), tx: tx.clone(), overlay: overlay.clone(), audio: player.clone() });

            let ui = h.clone();
            std::thread::spawn(move || {
                let fs = overlay.clone();
                sync::run(app, tx, rx, sync::Deps::real(move || fs.is_full_screen()), move |app, outcome, nudge, cues| {
                    // cues go first: a pet that's leaving must start its clip before the frame removes it
                    let quiet_ok = |sp| audio::allowed(&app.settings, Some(sp), app.flags.presentation, overlay.is_full_screen(), state::local_minute());
                    let species = |id| app.pets.iter().find(|p| p.id == id).map(|p| p.species);
                    for (id, cue) in cues {
                        if let Some(sp) = species(id)
                            && let Some(sound) = audio::for_cue(cue, sp).filter(|_| quiet_ok(sp)) {
                                player.play(sound, app.settings.volume);
                            }
                        let _ = ui.emit("cue", Cue { id, cue });
                    }
                    let s = &app.settings;
                    let config = overlay::Config {
                        liveliness: s.liveliness,
                        pet_height: s.pet_size.height(),
                        modifier: s.modifier,
                        hide_in_full_screen: s.hide_in_full_screen,
                    };
                    overlay.set_pets(app.views(Utc::now()), config);
                    commands::emit_changed(&ui);
                    // menu calls wait for the main thread, which may itself be waiting for the lock this thread holds
                    let main = ui.clone();
                    let _ = ui.run_on_main_thread(move || refresh_tray(&main));
                    if let Some(strays_core::nudge::Nudge::Pet { id, .. }) = &nudge
                        && let Some(sp) = species(*id).filter(|&sp| quiet_ok(sp)) {
                            player.play(audio::Sound::Sad(sp), app.settings.volume);
                        }
                    if let Some(n) = nudge {
                        let (id, text) = match n {
                            strays_core::nudge::Nudge::Pet { id, text } | strays_core::nudge::Nudge::TakingABreak { id, text } => (id, text),
                        };
                        #[cfg(debug_assertions)]
                        println!("NUDGE {id} {text}");
                        let _ = ui.emit("nudge", Feedback { id, ok: true, text });
                    }
                    if let Some(sync::SyncOutcome { ok, for_pet: Some(id) }) = outcome {
                        let text = if ok { "✓".into() } else { format!("⚠ {}", app.sync_error.clone().unwrap_or_default()) };
                        let _ = ui.emit("feedback", Feedback { id, ok, text });
                    }
                    #[cfg(debug_assertions)]
                    println!("STATE visible={} pets={} sync={}", app.visible.len(), app.pets.len(), menu::status_line(app));
                });
            });
            if first_run && let Err(e) = windows::welcome(&h) {
                h.state::<Shared>().app.lock().unwrap().warn("window", format!("Couldn't open the welcome window: {e}"));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|h, event| match event {
            // a menu-bar app keeps running with no windows open
            tauri::RunEvent::ExitRequested { api, code: None, .. } => api.prevent_exit(),
            // opening the app again (Finder, Spotlight) shows Settings, the way in when the notch hides the icon
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen { .. } => open_from_menu(h, windows::settings(h, "pets")),
            _ => {}
        });
}
