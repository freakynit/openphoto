//! Headless UI tests: the whole app runs in egui_kittest with the wgpu
//! renderer, so interactions can be simulated and frames rendered to PNG.
//!
//! Rendering uses 2 physical pixels per point, like the Retina screen the
//! Photoshop references were taken on (so 100% zoom matches). Screenshots are
//! written to `target/ui-shots/` both at 2× and downscaled to 1 pixel per
//! Photoshop point (`*_1x.png`), which lines up with Photoshop screenshots. The tests
//! that only write screenshots are `#[ignore]`d; run them with
//! `cargo test -p op-ui ui_tests -- --ignored`.

use std::path::PathBuf;

use egui::{Modifiers, Pos2, Vec2};
use egui_kittest::Harness;
use op_core::Color;

use crate::OpenPhotoApp;
use crate::theme::{UI_SCALE, pt};

/// Photoshop's default window on the reference screen, in points.
pub const WINDOW_PT: Vec2 = Vec2::new(1350.0, 800.0);

pub fn harness(files: Vec<PathBuf>) -> Harness<'static, OpenPhotoApp> {
    let mut harness = Harness::builder()
        .with_size(WINDOW_PT / UI_SCALE)
        .with_pixels_per_point(2.0)
        .wgpu()
        .build_eframe(|cc| OpenPhotoApp::new_headless(cc, files));
    // The app sets egui's zoom factor on the first frame
    harness.run_steps(4);
    harness
}

/// Renders the current frame to `target/ui-shots/{name}.png`.
pub fn shot(harness: &mut Harness<'_, OpenPhotoApp>, name: &str) -> PathBuf {
    let image = harness.render().expect("render frame");
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-shots");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.png"));
    image.save(&path).unwrap();
    let small = image::imageops::resize(
        &image,
        image.width() / 2,
        image.height() / 2,
        image::imageops::FilterType::Triangle,
    );
    small.save(dir.join(format!("{name}_1x.png"))).unwrap();
    path
}

/// A point given in Photoshop points from the window's top-left corner.
pub fn at_pt(x: f32, y: f32) -> Pos2 {
    Pos2::new(pt(x), pt(y))
}

pub fn click(harness: &mut Harness<'_, OpenPhotoApp>, pos: Pos2) {
    harness.hover_at(pos);
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.step();
    harness.event(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(3);
}

#[test]
fn toolbar_foreground_opens_color_picker() {
    let mut h = harness(Vec::new());
    h.state_mut().state.foreground = Color::from_rgba8([0x00, 0xaf, 0xdc, 255]);
    // The foreground swatch at the bottom of the toolbar
    click(&mut h, at_pt(13.0, 690.0));
    let session = h
        .state()
        .state
        .color_picker
        .as_ref()
        .expect("picker opened");
    assert_eq!(session.target, crate::state::PickerTarget::Foreground);
    assert!(h.state().state.modal_open());

    // Escape cancels and leaves the color alone
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().state.color_picker.is_none());
    assert_eq!(
        h.state().state.foreground.to_rgba8(),
        [0x00, 0xaf, 0xdc, 255]
    );
}

#[test]
#[ignore]
fn screenshot_color_picker() {
    let mut h = harness(Vec::new());
    h.state_mut().state.foreground = Color::from_rgba8([0x00, 0xaf, 0xdc, 255]);
    h.state_mut()
        .state
        .open_color_picker(crate::state::PickerTarget::Foreground);
    h.run_steps(4);
    shot(&mut h, "color_picker");
}

/// The document open in the Photoshop reference screenshots: 734×811,
/// without an embedded profile, shown at 100%.
pub fn reference_document(h: &mut Harness<'_, OpenPhotoApp>) {
    let app = &mut h.state_mut().state;
    crate::actions::close_all(app);
    let doc = op_core::Document::new_with_background(
        "Weixin Image_20260909100239_11307_249.png",
        734,
        811,
        Color::from_rgba8([0x14, 0x14, 0x14, 255]),
    );
    app.add_document(doc, "Open");
    h.run_steps(6);
}

#[test]
#[ignore]
fn screenshot_main_window() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    shot(&mut h, "main_window");
}
