//! Small AppKit calls that winit and muda don't offer (macOS only).

use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};

/// Hides the application, like the standard "Hide" menu item (which
/// OpenPhoto replaces to give it Photoshop's Ctrl+Cmd+H).
pub fn hide_app() {
    let Some(class) = AnyClass::get(c"NSApplication") else {
        return;
    };
    // SAFETY: called on the main thread from the event loop
    unsafe {
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if !app.is_null() {
            let _: () = msg_send![app, hide: std::ptr::null::<AnyObject>()];
        }
    }
}

/// Whether the event being handled is a key press: a menu item chosen
/// through its key equivalent rather than with the mouse.
pub fn handling_key_press() -> bool {
    /// `NSEventTypeKeyDown`
    const KEY_DOWN: usize = 10;
    let Some(class) = AnyClass::get(c"NSApplication") else {
        return false;
    };
    // SAFETY: menu actions run on the main thread
    unsafe {
        let app: *mut AnyObject = msg_send![class, sharedApplication];
        if app.is_null() {
            return false;
        }
        let event: *mut AnyObject = msg_send![app, currentEvent];
        if event.is_null() {
            return false;
        }
        let kind: usize = msg_send![event, type];
        kind == KEY_DOWN
    }
}
