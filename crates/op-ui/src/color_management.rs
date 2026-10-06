//! Display color management on macOS.
//!
//! Photoshop shows document colors converted from their sRGB values to the
//! display's profile. wgpu's Metal layer has no color space by default, so
//! macOS would show our sRGB values as raw display values (too saturated on a
//! wide-gamut screen). Tagging the layer as sRGB makes macOS do the same
//! conversion Photoshop does. Neutral grays are the same in sRGB and Display
//! P3, so the interface chrome keeps its exact values.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::msg_send;
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_quartz_core::{CALayer, CAMetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Tags every Metal layer under the window's content view as sRGB.
pub fn use_srgb(window: &impl HasWindowHandle) {
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return;
    };
    // SAFETY: the handle points to the window's live NSView, used on the main
    // thread during app creation.
    let view: &AnyObject = unsafe { appkit.ns_view.cast::<AnyObject>().as_ref() };
    let root: Option<Retained<CALayer>> = unsafe { msg_send![view, layer] };
    let (Some(root), Some(srgb)) = (root, CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))) else {
        return;
    };
    let tagged = tag(&root, &srgb);
    log::info!("tagged {tagged} Metal layer(s) as sRGB");
}

/// Returns how many Metal layers were tagged.
fn tag(layer: &CALayer, srgb: &CGColorSpace) -> usize {
    let mut count = 0;
    if let Some(metal) = layer.downcast_ref::<CAMetalLayer>() {
        metal.setColorspace(Some(srgb));
        count += 1;
    }
    // SAFETY: reading the sublayer list on the main thread
    if let Some(sublayers) = unsafe { layer.sublayers() } {
        for sublayer in sublayers.iter() {
            count += tag(&sublayer, srgb);
        }
    }
    count
}
