//! Strays app shell: the core's state, a background sync loop, the menu-bar menu and the pet overlay.
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

mod commands;
mod menu;
mod overlay;
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
}

const TRAY: &str = "strays";

#[derive(Clone, Serialize)]
struct Feedback {
    id: Uuid,
    ok: bool,
    text: String,
}

/// ⌥-click on a pet: ⇧ quick-syncs it (§7.3), otherwise its info card opens beside it (§8.3).
#[tauri::command]
fn pet_click(id: Uuid, shift: bool, shared: State<'_, Shared>, h: AppHandle) {
    #[cfg(debug_assertions)]
    println!("CLICK pet={id} shift={shift}");
    if shift {
        let _ = shared.tx.send(Cmd::SyncNow(Some(id)));
        return;
    }
    if let Err(e) = windows::open_card(&h, &id.to_string(), shared.overlay.pet_rect(id)) {
        eprintln!("could not open the info card: {e}");
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
        menu::Event::NewFolder(i, adopt) => {
            let Some(path) = app.new_folders.get(i).cloned() else { return };
            if adopt {
                drop(app);
                let _ = windows::adopt(h, &format!("folder={}", windows::encode(&path)));
                return;
            }
            app.ignore_folder(&path);
            Ok(None)
        }
        menu::Event::Settings => {
            drop(app);
            let _ = windows::settings(h, "pets");
            return;
        }
        menu::Event::Setup => {
            drop(app);
            let _ = windows::setup(h);
            return;
        }
        menu::Event::AdoptWindow => {
            drop(app);
            let _ = windows::adopt(h, "");
            return;
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
            commands::ignore_folder,
            commands::save_settings,
            commands::set_flag,
            commands::sync_now,
            commands::token_check,
            commands::token_save,
            commands::token_disconnect,
            commands::pick_folder,
            commands::open_github,
            commands::reveal,
            commands::netlog,
            commands::open_link,
            commands::open_data_folder,
            commands::export_data,
            commands::delete_everything,
            commands::open_window,
            commands::close_window,
            commands::local_minute,
        ])
        .on_menu_event(|h, e| handle_menu(h, e.id().as_ref()))
        .setup(|tauri_app| {
            #[cfg(target_os = "macos")]
            tauri_app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let h = tauri_app.handle().clone();

            #[allow(unused_mut)]
            let mut app = App::load(data_dir(&h)).map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            #[cfg(debug_assertions)]
            spawn_test_pets(&mut app);
            let first_run = commands::snapshot(&app).first_run;
            let menu = menu::build(&h, &app)?;
            TrayIconBuilder::with_id(TRAY).title(menu::tray_title(&app)).tooltip("Strays").menu(&menu).show_menu_on_left_click(true).build(&h)?;

            let overlay = Overlay::create(&h)?;
            overlay.start_poller(h.clone());
            let app = Arc::new(Mutex::new(app));
            let (tx, rx) = mpsc::channel();
            h.manage(Shared { app: app.clone(), tx: tx.clone(), overlay: overlay.clone() });

            let ui = h.clone();
            std::thread::spawn(move || {
                let fs = overlay.clone();
                sync::run(app, tx, rx, move || fs.is_full_screen(), move |app, outcome, nudge| {
                    let s = &app.settings;
                    let config = overlay::Config {
                        liveliness: s.liveliness,
                        pet_height: s.pet_size.height(),
                        modifier: s.modifier,
                        hide_in_full_screen: s.hide_in_full_screen,
                    };
                    overlay.set_pets(app.views(Utc::now()), config);
                    commands::emit_changed(&ui);
                    if let Some(tray) = ui.tray_by_id(TRAY) {
                        let _ = tray.set_title(Some(menu::tray_title(app)));
                        if let Ok(m) = menu::build(&ui, app) {
                            let _ = tray.set_menu(Some(m));
                        }
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
            if first_run {
                let _ = windows::welcome(&h);
            }
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
