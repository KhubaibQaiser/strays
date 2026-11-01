//! macOS calls the overlay needs. None of them needs a permission prompt.
use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSWindow, NSWindowCollectionBehavior, NSWindowSharingType, NSWorkspace};
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect};
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowBounds,
    kCGWindowLayer, kCGWindowOwnerPID,
};
use objc2_foundation::NSProcessInfo;

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
