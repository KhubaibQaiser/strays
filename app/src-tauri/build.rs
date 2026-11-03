// Every command needs a permission (capabilities/<window>.json), so each window can call only what it uses.
const COMMANDS: &[&str] = &[
    "adopt",
    "close_window",
    "delete_everything",
    "dismiss_issues",
    "export_data",
    "netlog",
    "open_data_folder",
    "open_github",
    "open_link",
    "open_window",
    "overlay_ready",
    "pet_action",
    "pet_click",
    "pet_log",
    "pick_folder",
    "reveal",
    "reveal_log",
    "save_settings",
    "set_flag",
    "set_goal",
    "set_login_item",
    "set_pet",
    "sync_now",
    "test_sound",
    "token_check",
    "token_disconnect",
    "token_save",
    "ui_state",
];

fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(COMMANDS))).expect("tauri-build");
}
