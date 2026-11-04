//! macOS calls: the overlay's, folder bookmarks and the login item. None of them needs a permission prompt.
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSWindow, NSWindowCollectionBehavior, NSWindowSharingType, NSWorkspace};
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect};
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowBounds,
    kCGWindowLayer, kCGWindowOwnerPID,
};
use objc2::runtime::Bool;
use objc2_foundation::{NSData, NSProcessInfo, NSString, NSURL, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions};
use objc2_service_management::{SMAppService, SMAppServiceStatus};

/// Pass-through overlay rules (spec §7.1): every Space, still in Mission Control, kept out of ⌘-Tab,
/// over full-screen apps, and excluded from screen capture and sharing.
pub fn configure_overlay(ns_window: *mut std::ffi::c_void) {
    // SAFETY: Tauri hands out the window's live NSWindow pointer, and this runs on the main thread.
    let window = unsafe { &*(ns_window as *const NSWindow) };
    // debug builds can opt back in so the test harness can screenshot pets
    let capturable = cfg!(debug_assertions) && std::env::var("STRAYS_CAPTURABLE").as_deref() == Ok("1");
    window.setSharingType(if capturable { NSWindowSharingType::ReadOnly } else { NSWindowSharingType::None });
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
}

pub fn modifier_flags() -> NSEventModifierFlags {
    NSEvent::modifierFlags_class()
}

/// Whether nobody can see the screen: the main display is asleep or the session is locked. Pets stop
/// drawing then (review-spec A8).
pub fn screen_off() -> bool {
    use objc2_core_foundation::CFBoolean;
    use objc2_core_graphics::{CGMainDisplayID, CGSessionCopyCurrentDictionary};
    // declared here: objc2's binding sits behind its `libc` feature
    unsafe extern "C" {
        fn CGDisplayIsAsleep(display: u32) -> u32;
    }
    // SAFETY: plain C call with a display id CoreGraphics gave us.
    if unsafe { CGDisplayIsAsleep(CGMainDisplayID()) } != 0 {
        return true;
    }
    let Some(session) = CGSessionCopyCurrentDictionary() else { return false };
    // SAFETY: the session dictionary has CFString keys (CGSession.h).
    let session: CFRetained<CFDictionary<CFString, CFType>> = unsafe { CFRetained::cast_unchecked(session) };
    session.get(&CFString::from_static_str("CGSSessionScreenIsLocked")).and_then(|v| v.downcast::<CFBoolean>().ok()).is_some_and(|b| b.as_bool())
}

/// A blocking alert, for a failure with no other way to reach the user (the app couldn't start).
pub fn fatal(title: &str, text: &str) {
    let Some(mtm) = objc2::MainThreadMarker::new() else { return };
    let alert = objc2_app_kit::NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(text));
    alert.runModal();
}

/// Cursor in AppKit global points (bottom-left origin of the primary display).
pub fn mouse_location() -> (f64, f64) {
    let p = NSEvent::mouseLocation();
    (p.x, p.y)
}

pub fn reduce_motion() -> bool {
    NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
}

pub fn low_power_mode() -> bool {
    NSProcessInfo::processInfo().isLowPowerModeEnabled()
}

/// "26.5.0", for error reports.
pub fn os_version() -> String {
    let v = NSProcessInfo::processInfo().operatingSystemVersion();
    format!("{}.{}.{}", v.majorVersion, v.minorVersion, v.patchVersion)
}

/// Bounds of the frontmost app's frontmost normal window, in global points with a top-left origin.
/// Window bounds are readable without Screen Recording (titles aren't, and we don't ask for them).
pub fn frontmost_window() -> Option<CGRect> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    // processIdentifier sits behind objc2-app-kit's `libc` feature; a direct message avoids adding it.
    // SAFETY: -[NSRunningApplication processIdentifier] returns pid_t (i32).
    let pid: i32 = unsafe { objc2::msg_send![&*app, processIdentifier] };
    let list = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    )?;
    // SAFETY: the window list is documented as an array of dictionaries with CFString keys.
    let list: CFRetained<CFArray<CFDictionary<CFString, CFType>>> = unsafe { CFRetained::cast_unchecked(list) };
    let number = |d: &CFDictionary<CFString, CFType>, key: &CFString| -> Option<i64> {
        d.get(key)?.downcast::<CFNumber>().ok()?.as_i64()
    };
    for d in list.iter() {
        // SAFETY: the keys are framework constants.
        let (pid_key, layer_key, bounds_key) = unsafe { (kCGWindowOwnerPID, kCGWindowLayer, kCGWindowBounds) };
        if number(&d, pid_key) != Some(pid.into()) || number(&d, layer_key) != Some(0) {
            continue;
        }
        let bounds = d.get(bounds_key)?.downcast::<CFDictionary>().ok()?;
        let mut rect = CGRect::default();
        // SAFETY: rect is a valid out-pointer.
        if unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &mut rect) } {
            return Some(rect);
        }
    }
    None
}

/// A read-only security-scoped bookmark for a folder the user picked (§9), so the sandboxed app can open it again after a restart.
pub fn bookmark(path: &str) -> Result<Vec<u8>, String> {
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    let options = NSURLBookmarkCreationOptions::WithSecurityScope | NSURLBookmarkCreationOptions::SecurityScopeAllowOnlyReadAccess;
    url.bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(options, None, None)
        .map(|d| d.to_vec())
        .map_err(|e| e.localizedDescription().to_string())
}

/// Resolves a bookmark and starts access for the rest of the run. Returns the folder's current path and,
/// when macOS says the bookmark is stale (the folder moved), a fresh one.
pub fn open_bookmark(data: &[u8]) -> Result<(String, Option<Vec<u8>>), String> {
    let mut stale = Bool::NO;
    // SAFETY: `stale` is a valid out-pointer for the duration of the call.
    let url = unsafe {
        NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(&NSData::with_bytes(data), NSURLBookmarkResolutionOptions::WithSecurityScope, None, &mut stale)
    }
    .map_err(|e| e.localizedDescription().to_string())?;
    // SAFETY: plain Foundation call; access is kept until the app quits, as the pets watch these folders all the time.
    unsafe { url.startAccessingSecurityScopedResource() };
    let path = url.path().ok_or("the folder has no path")?.to_string();
    // a stale bookmark still works for this run; failing to refresh it only matters after the next restart
    let fresh = if stale.as_bool() { bookmark(&path).inspect_err(|e| crate::log::warn(&format!("could not refresh the bookmark for {path}: {e}"))).ok() } else { None };
    Ok((path, fresh))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LoginItem {
    On,
    Off,
    /// Registered, but the user turned it off in System Settings → Login Items.
    NeedsApproval,
}

/// Launch at login (§9) through `SMAppService`; it shows up in System Settings → Login Items.
pub fn login_item() -> LoginItem {
    // SAFETY: plain ServiceManagement calls.
    match unsafe { SMAppService::mainAppService().status() } {
        SMAppServiceStatus::Enabled => LoginItem::On,
        SMAppServiceStatus::RequiresApproval => LoginItem::NeedsApproval,
        _ => LoginItem::Off,
    }
}

pub fn set_login_item(on: bool) -> Result<LoginItem, String> {
    // SAFETY: plain ServiceManagement calls.
    let service = unsafe { SMAppService::mainAppService() };
    let result = if on { unsafe { service.registerAndReturnError() } } else { unsafe { service.unregisterAndReturnError() } };
    result.map_err(|e| e.localizedDescription().to_string())?;
    Ok(login_item())
}
