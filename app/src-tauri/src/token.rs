//! The GitHub token lives in the Keychain (spec §6.1) and never reaches the webview.
//! Debug builds fall back to `~/.config/strays/dev-token` so the agent can test without the setup UI.

const SERVICE: &str = "com.khubaibqaiser.strays.github";
// ponytail: one token for the personal account; per-owner tokens (§6.1) arrive with the setup screen.
const ACCOUNT: &str = "personal";

pub fn load() -> Option<String> {
    #[cfg(debug_assertions)]
    if let Ok(t) = std::env::var("STRAYS_TOKEN") {
        return Some(t); // test runs against the fake server
    }
    #[cfg(target_os = "macos")]
    if let Ok(bytes) = security_framework::passwords::get_generic_password(SERVICE, ACCOUNT) {
        return String::from_utf8(bytes).ok().map(|t| t.trim().to_string());
    }
    #[cfg(debug_assertions)]
    {
        let path = std::path::PathBuf::from(std::env::var_os("HOME")?).join(".config/strays/dev-token");
        if let Ok(t) = std::fs::read_to_string(path) {
            return Some(t.trim().to_string());
        }
    }
    None
}

#[allow(dead_code)] // used by the setup screen (Gate 2b)
pub fn save(token: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return security_framework::passwords::set_generic_password(SERVICE, ACCOUNT, token.as_bytes()).map_err(|e| e.to_string());
    #[cfg(not(target_os = "macos"))]
    Err("no secure store on this platform yet".into())
}

#[allow(dead_code)] // used by the setup screen (Gate 2b)
pub fn delete() {
    #[cfg(target_os = "macos")]
    let _ = security_framework::passwords::delete_generic_password(SERVICE, ACCOUNT);
}
