//! Strays app shell: the core's state, a background sync loop, the menu-bar menu and the pet overlay.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

mod menu;
mod overlay;
mod state;
mod sync;
mod token;

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
}

const TRAY: &str = "strays";

#[derive(Clone, Serialize)]
struct Feedback {
    id: Uuid,
    ok: bool,
    text: String,
}

/// ⌥-click on a pet: ⇧ quick-syncs it (§7.3), otherwise its menu opens at the cursor.
#[tauri::command]
fn pet_click(id: Uuid, shift: bool, shared: State<'_, Shared>, h: AppHandle) {
    #[cfg(debug_assertions)]
    println!("CLICK pet={id} shift={shift}");
    if shift {
        let _ = shared.tx.send(Cmd::SyncNow(Some(id)));
        return;
    }
    let menu = menu::pet_menu(&h, &shared.app.lock().unwrap(), id);
    if let Ok(Some(menu)) = menu {
        let w = shared.overlay.window();
        // ponytail: after Escape closes this menu, AppKit swallows the next click (a known AppKit
        // pop-up quirk in a non-active app). The info card (§8.3, Gate 2b) replaces this menu.
        let _ = w.popup_menu(&menu);
    }
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
        let local = LocalRef { bookmark_id: Uuid::new_v4(), display_path: format!("~/Code/test-{i}") };
        let project = Project { github: None, local: Some(local) };
        let mut p = Pet::new(now, &format!("Test {i}"), Species::Cat, project, Activity { last: now, source: ActivitySource::LocalCommit }, app.settings.default_pace);
        for (name, hex) in ["coatBase", "coatShade", "coatHighlight", "outline"].iter().zip(looks[i % 3]) {
            p.colors.insert((*name).into(), hex.into());
        }
        p.adopt(now).expect("new pet");
        app.pets.push(p);
    }
}

fn handle_menu(h: &AppHandle, id: &str) {
    let shared = h.state::<Shared>();
    let Some(event) = menu::parse(id) else { return };
    let now = Utc::now();
    let mut app = shared.app.lock().unwrap();
    let result = match event {
        menu::Event::Pet(pet, action) => app.act(pet, action, now).map(Some),
        menu::Event::SyncPet(pet) => {
            let _ = shared.tx.send(Cmd::SyncNow(Some(pet)));
            return;
        }
        menu::Event::Sync => {
            let _ = shared.tx.send(Cmd::SyncNow(None));
            return;
        }
        menu::Event::Repo(repo, species) => app.adopt_repo(repo, species, now).map(Some),
        menu::Event::IgnoreRepo(repo) => {
            app.ignore_new(repo);
            Ok(None)
        }
        menu::Event::Presentation => {
            app.flags.presentation = !app.flags.presentation;
            Ok(None)
        }
        menu::Event::Vacation => {
            let on = !app.flags.vacation;
            app.set_vacation(on, now);
            Ok(None)
        }
        menu::Event::HideAll => {
            app.flags.hide_all = !app.flags.hide_all;
            Ok(None)
        }
        menu::Event::Quit => {
            app.save_all();
            h.exit(0);
            return;
        }
    };
    drop(app);
    match result {
        Ok(Some(msg)) | Err(msg) => {
            #[cfg(debug_assertions)]
            println!("ACTION {msg}");
            let _ = h.emit("notice", msg);
        }
        Ok(None) => {}
    }
    let _ = shared.tx.send(Cmd::Changed);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![pet_click, overlay_ready, pet_log])
        .on_menu_event(|h, e| handle_menu(h, e.id().as_ref()))
        .setup(|tauri_app| {
            #[cfg(target_os = "macos")]
            tauri_app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let h = tauri_app.handle().clone();

            #[allow(unused_mut)]
            let mut app = App::load(data_dir(&h)).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            #[cfg(debug_assertions)]
            spawn_test_pets(&mut app);
            let menu = menu::build(&h, &app)?;
            TrayIconBuilder::with_id(TRAY).title(menu::tray_title(&app)).tooltip("Strays").menu(&menu).show_menu_on_left_click(true).build(&h)?;

            let overlay = Overlay::create(&h)?;
            overlay.start_poller(h.clone());
            let app = Arc::new(Mutex::new(app));
            let (tx, rx) = mpsc::channel();
            h.manage(Shared { app: app.clone(), tx, overlay: overlay.clone() });

            let ui = h.clone();
            std::thread::spawn(move || {
                sync::run(app, rx, move |app, outcome| {
                    overlay.set_pets(app.views(Utc::now()), app.settings.liveliness);
                    if let Some(tray) = ui.tray_by_id(TRAY) {
                        let _ = tray.set_title(Some(menu::tray_title(app)));
                        if let Ok(m) = menu::build(&ui, app) {
                            let _ = tray.set_menu(Some(m));
                        }
                    }
                    if let Some(sync::SyncOutcome { ok, for_pet: Some(id) }) = outcome {
                        let text = if ok { "✓".into() } else { format!("⚠ {}", app.sync_error.clone().unwrap_or_default()) };
                        let _ = ui.emit("feedback", Feedback { id, ok, text });
                    }
                    #[cfg(debug_assertions)]
                    println!("STATE visible={} pets={} sync={}", app.visible.len(), app.pets.len(), menu::status_line(app));
                });
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, event| {
            // a menu-bar app keeps running with no windows open
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
