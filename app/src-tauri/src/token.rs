//! The GitHub token lives in the Keychain (spec §6.1) and never reaches the webview.
//! Debug builds fall back to `~/.config/strays/dev-token` so the agent can test without the setup UI.

const SERVICE: &str = "com.khubaibqaiser.strays.github";
// ponytail: one token for the personal account; per-owner tokens (§6.1) aren't built yet.
const ACCOUNT: &str = "personal";

/// macOS's "no such Keychain item" status: no token yet, which isn't an error.
#[cfg(target_os = "macos")]
const NOT_FOUND: i32 = -25300;

/// `Ok(None)` when no token is stored; `Err` when the Keychain couldn't be read (locked, access denied, …).
pub fn load() -> Result<Option<String>, String> {
    #[cfg(debug_assertions)]
    if let Ok(t) = std::env::var("STRAYS_TOKEN") {
        return Ok(Some(t)); // test runs against the fake server
    }
    #[cfg(target_os = "macos")]
    match security_framework::passwords::get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => return String::from_utf8(bytes).map(|t| Some(t.trim().to_string())).map_err(|_| "The stored GitHub token isn't text; paste it again".to_string()),
        Err(e) if e.code() == NOT_FOUND => {}
        Err(e) => return Err(format!("Couldn't read the GitHub token from the Keychain: {e}")),
    }
    #[cfg(debug_assertions)]
    if let Some(home) = std::env::var_os("HOME")
        && let Ok(t) = std::fs::read_to_string(std::path::PathBuf::from(home).join(".config/strays/dev-token"))
    {
        return Ok(Some(t.trim().to_string()));
    }
    Ok(None)
}

pub fn save(token: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return security_framework::passwords::set_generic_password(SERVICE, ACCOUNT, token.as_bytes()).map_err(|e| e.to_string());
    #[cfg(not(target_os = "macos"))]
    Err("no secure store on this platform yet".into())
}

/// Removes the token; already gone counts as done.
pub fn delete() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    match security_framework::passwords::delete_generic_password(SERVICE, ACCOUNT) {
        Err(e) if e.code() != NOT_FOUND => return Err(format!("Couldn't delete the GitHub token from the Keychain: {e}")),
        _ => {}
    }
    Ok(())
}
