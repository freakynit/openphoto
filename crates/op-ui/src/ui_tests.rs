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
        // Real frame times, so a double click fits in egui's 0.3 s window
        .with_step_dt(1.0 / 60.0)
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

pub fn double_click(harness: &mut Harness<'_, OpenPhotoApp>, pos: Pos2) {
    harness.hover_at(pos);
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.event(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
            harness.step();
        }
    }
    harness.run_steps(3);
}

pub fn drag(harness: &mut Harness<'_, OpenPhotoApp>, from: Pos2, to: Pos2, modifiers: Modifiers) {
    harness.hover_at(from);
    harness.event_modifiers(
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers,
        },
        modifiers,
    );
    harness.step();
    for i in 1..=4 {
        let p = from + (to - from) * (i as f32 / 4.0);
        harness.event_modifiers(egui::Event::PointerMoved(p), modifiers);
        harness.step();
    }
    harness.event_modifiers(
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers,
        },
        modifiers,
    );
    harness.run_steps(3);
}

fn active<'a>(h: &'a Harness<'_, OpenPhotoApp>) -> &'a crate::state::DocState {
    let app = &h.state().state;
    &app.docs[&app.active_doc.unwrap()]
}

/// Screen position (egui points) of a document pixel in the active document.
fn doc_point(h: &Harness<'_, OpenPhotoApp>, x: f32, y: f32) -> Pos2 {
    let state = active(h);
    let ppp = 2.0 * UI_SCALE;
    crate::document_view::origin(state, ppp) + egui::vec2(x, y) * state.view.zoom / ppp
}

#[test]
fn marquee_selects_and_click_deselects() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    assert_eq!(h.state().state.tool, op_tools::Tool::RectangularMarquee);

    let (a, b) = (doc_point(&h, 100.0, 120.0), doc_point(&h, 300.0, 220.0));
    drag(&mut h, a, b, Modifiers::NONE);
    let sel = active(&h).doc.selection().expect("selection made");
    let (x0, y0, x1, y1) = sel.bounds().unwrap();
    assert!(
        (x0 as i32 - 100).abs() <= 1 && (y0 as i32 - 120).abs() <= 1,
        "{x0},{y0}"
    );
    assert!(
        (x1 as i32 - 300).abs() <= 1 && (y1 as i32 - 220).abs() <= 1,
        "{x1},{y1}"
    );
    let states = active(&h).history.states();
    assert_eq!(states.last().unwrap().name, "Rectangular Marquee");

    // Shift-drag adds to the selection
    let (c, d) = (doc_point(&h, 400.0, 400.0), doc_point(&h, 500.0, 500.0));
    drag(&mut h, c, d, Modifiers::SHIFT);
    let (_, _, x1, y1) = active(&h).doc.selection().unwrap().bounds().unwrap();
    assert!(x1 >= 499 && y1 >= 499);

    // A click deselects
    let p = doc_point(&h, 50.0, 50.0);
    click(&mut h, p);
    assert!(active(&h).doc.selection().is_none());
    assert_eq!(active(&h).history.states().last().unwrap().name, "Deselect");

    // Cmd+Shift+D reselects, Cmd+A selects all, Cmd+D deselects
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::D);
    h.run_steps(2);
    assert!(active(&h).doc.selection().is_some());
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.run_steps(2);
    assert_eq!(
        active(&h).doc.selection().unwrap().bounds(),
        Some((0, 0, 734, 811))
    );
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    assert!(active(&h).doc.selection().is_none());
}

#[test]
fn shift_m_cycles_marquee_tools() {
    let mut h = harness(Vec::new());
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::M);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::EllipticalMarquee);
    h.key_press(egui::Key::V);
    h.run_steps(2);
    h.key_press(egui::Key::M);
    h.run_steps(2);
    // The slot remembers the elliptical marquee
    assert_eq!(h.state().state.tool, op_tools::Tool::EllipticalMarquee);
}

#[test]
#[ignore]
fn screenshot_painting() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([0x14, 0xa5, 0xdc, 255]);
    h.key_press(egui::Key::B);
    for _ in 0..6 {
        h.key_press(egui::Key::CloseBracket);
    }
    h.run_steps(2);
    let (a, b) = (doc_point(&h, 120.0, 200.0), doc_point(&h, 600.0, 260.0));
    drag(&mut h, a, b, Modifiers::NONE);
    // A softer, half-opacity stroke crossing it
    h.key_press(egui::Key::Num5);
    let (c, d) = (doc_point(&h, 300.0, 100.0), doc_point(&h, 360.0, 600.0));
    drag(&mut h, c, d, Modifiers::NONE);
    h.key_press(egui::Key::E);
    let (e, f) = (doc_point(&h, 100.0, 500.0), doc_point(&h, 650.0, 520.0));
    drag(&mut h, e, f, Modifiers::NONE);
    h.key_press(egui::Key::B);
    h.hover_at(doc_point(&h, 450.0, 420.0));
    h.run_steps(3);
    shot(&mut h, "painting");
}

#[test]
#[ignore]
fn screenshot_fill_dialog() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::F5);
    h.run_steps(4);
    shot(&mut h, "fill_dialog");
}

#[test]
#[ignore]
fn screenshot_selection() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    let (a, b) = (doc_point(&h, 100.0, 120.0), doc_point(&h, 400.0, 420.0));
    drag(&mut h, a, b, Modifiers::NONE);
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::M);
    h.run_steps(2);
    let (c, d) = (doc_point(&h, 300.0, 300.0), doc_point(&h, 650.0, 700.0));
    drag(&mut h, c, d, Modifiers::SHIFT);
    shot(&mut h, "selection");
}

fn composite_pixel(h: &mut Harness<'_, OpenPhotoApp>, x: u32, y: u32) -> [u8; 4] {
    let app = &mut h.state_mut().state;
    let id = app.active_doc.unwrap();
    let state = app.docs.get_mut(&id).unwrap();
    state.sample(x, y).unwrap().to_rgba8()
}

#[test]
fn brush_paints_with_the_foreground_color() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    h.key_press(egui::Key::B);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::Brush);

    // ] grows the 30 px brush by Photoshop's step (5 px below 50)
    h.key_press(egui::Key::CloseBracket);
    h.run_steps(2);
    assert_eq!(h.state().state.brush.size, 35.0);
    // 5 sets 50% opacity, 0 back to 100%
    h.key_press(egui::Key::Num5);
    h.key_press(egui::Key::Num0);
    h.run_steps(2);
    assert_eq!(h.state().state.brush.opacity, 1.0);

    let (a, b) = (doc_point(&h, 100.0, 300.0), doc_point(&h, 500.0, 300.0));
    drag(&mut h, a, b, Modifiers::NONE);
    let px = composite_pixel(&mut h, 300, 300);
    assert!(px[0] > 240 && px[1] < 20 && px[2] < 20, "{px:?}");
    // Untouched pixels keep the document color
    assert_eq!(composite_pixel(&mut h, 300, 600), [0x14, 0x14, 0x14, 255]);
    assert_eq!(
        active(&h).history.states().last().unwrap().name,
        "Brush Tool"
    );
}

#[test]
fn painting_a_hidden_layer_shows_photoshops_alert() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    {
        let app = &mut h.state_mut().state;
        let id = app.active_doc.unwrap();
        let doc = &mut app.docs.get_mut(&id).unwrap().doc;
        doc.layers[0].visible = false;
    }
    h.key_press(egui::Key::B);
    h.run_steps(2);
    let p = doc_point(&h, 300.0, 300.0);
    click(&mut h, p);
    assert_eq!(
        h.state().state.alert.as_deref(),
        Some("Could not use the brush tool because the target layer is hidden.")
    );
}

fn select_rect(h: &mut Harness<'_, OpenPhotoApp>, x0: f32, y0: f32, x1: f32, y1: f32) {
    let app = &mut h.state_mut().state;
    let id = app.active_doc.unwrap();
    let doc = &mut app.docs.get_mut(&id).unwrap().doc;
    let (w, ht) = (doc.width, doc.height);
    doc.set_selection(Some(op_core::Selection::rect(
        w,
        ht,
        op_core::selection::Rect::new(x0, y0, x1, y1),
    )));
}

#[test]
fn fill_and_clear_keys() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    h.state_mut().state.background = Color::from_rgba8([0, 0, 255, 255]);
    select_rect(&mut h, 0.0, 0.0, 100.0, 100.0);

    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 50, 50), [255, 0, 0, 255]);
    assert_eq!(composite_pixel(&mut h, 150, 150), [0x14, 0x14, 0x14, 255]);
    assert_eq!(active(&h).history.states().last().unwrap().name, "Fill");

    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 50, 50), [0, 0, 255, 255]);

    // Delete on a regular layer clears the selection to transparency
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(active(&h).history.states().last().unwrap().name, "Clear");
    assert_eq!(composite_pixel(&mut h, 50, 50), [0, 0, 255, 255]);

    // Without a selection, Delete removes the layer
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    let layers = active(&h).doc.layers.len();
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(active(&h).doc.layers.len(), layers - 1);
}

#[test]
fn fill_dialog_applies_on_enter() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([0, 255, 0, 255]);
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::F5);
    h.run_steps(3);
    assert!(h.state().state.fill_dialog.is_some());
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().state.fill_dialog.is_none());
    assert_eq!(composite_pixel(&mut h, 10, 10), [0, 255, 0, 255]);
}

#[test]
fn paint_bucket_fills_contiguous_area() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([255, 255, 0, 255]);
    h.state_mut().state.select_tool(op_tools::Tool::PaintBucket);
    let p = doc_point(&h, 200.0, 200.0);
    click(&mut h, p);
    assert_eq!(composite_pixel(&mut h, 700, 800), [255, 255, 0, 255]);
    assert_eq!(
        active(&h).history.states().last().unwrap().name,
        "Paint Bucket"
    );
}

#[test]
fn move_tool_drags_and_nudges() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A red square on a new layer
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);

    h.key_press(egui::Key::V);
    h.run_steps(2);
    let (a, b) = (doc_point(&h, 120.0, 120.0), doc_point(&h, 220.0, 170.0));
    drag(&mut h, a, b, Modifiers::NONE);
    assert_eq!(composite_pixel(&mut h, 220, 170), [255, 0, 0, 255]);
    assert_eq!(composite_pixel(&mut h, 110, 110), [0x14, 0x14, 0x14, 255]);
    assert_eq!(active(&h).history.states().last().unwrap().name, "Move");

    // The square now spans x 200..240; nudge it right by 1 and then 10
    h.key_press(egui::Key::ArrowRight);
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 250, 170), [255, 0, 0, 255]);
    assert_eq!(composite_pixel(&mut h, 205, 170), [0x14, 0x14, 0x14, 255]);
    assert_eq!(active(&h).history.states().last().unwrap().name, "Nudge");
}

#[test]
fn moving_the_background_without_selection_is_refused() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press(egui::Key::V);
    h.run_steps(2);
    let (a, b) = (doc_point(&h, 120.0, 120.0), doc_point(&h, 220.0, 170.0));
    drag(&mut h, a, b, Modifiers::NONE);
    assert_eq!(
        h.state().state.alert.as_deref(),
        Some("Could not use the move tool because the layer is locked.")
    );
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

fn layer_pixel(h: &Harness<'_, OpenPhotoApp>, layer: usize, x: u32, y: u32) -> [u8; 4] {
    let op_core::LayerKind::Raster(image) = &active(h).doc.layers[layer].kind;
    image.pixel(x, y)
}

#[test]
fn copy_and_paste_stack_a_new_layer() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::C);
    h.run_steps(2);
    // egui-winit delivers Cmd+V as a Paste event
    h.event(egui::Event::Paste(String::new()));
    h.run_steps(2);
    let doc = &active(&h).doc;
    assert_eq!(doc.layers.len(), 2);
    assert_eq!(doc.layers[1].name, "Layer 1");
    assert!(doc.selection().is_none());
    assert_eq!(active(&h).history.states().last().unwrap().name, "Paste");
    // Pasted in place over the original; nothing outside it
    assert_eq!(layer_pixel(&h, 1, 120, 120), [255, 0, 0, 255]);
    assert_eq!(layer_pixel(&h, 1, 150, 120), [0, 0, 0, 0]);

    // Cut on the new layer leaves a hole that shows the background
    select_rect(&mut h, 100.0, 100.0, 120.0, 140.0);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::X);
    h.run_steps(2);
    assert_eq!(layer_pixel(&h, 1, 110, 120)[3], 0);
    assert_eq!(layer_pixel(&h, 1, 130, 120), [255, 0, 0, 255]);
    assert_eq!(active(&h).history.states().last().unwrap().name, "Cut");

    // Copying only transparent pixels is refused like in Photoshop
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::C);
    h.run_steps(2);
    assert_eq!(
        h.state().state.alert.as_deref(),
        Some("Could not complete the Copy command because the selected area is empty.")
    );
}

fn layer_names(h: &Harness<'_, OpenPhotoApp>) -> Vec<String> {
    active(h)
        .doc
        .layers
        .iter()
        .map(|l| l.name.clone())
        .collect()
}

fn last_history(h: &Harness<'_, OpenPhotoApp>) -> String {
    active(h).history.states().last().unwrap().name.clone()
}

#[test]
fn layer_shortcuts_copy_arrange_and_merge() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // Cmd+J on the background without a selection: "Layer 1"
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Layer 1"]);
    assert_eq!(last_history(&h), "Layer Via Copy");
    // ...and on a regular layer: "Layer 1 copy"
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Layer 1", "Layer 1 copy"]);
    // Cmd+[ sends it backward
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::OpenBracket);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Layer 1 copy", "Layer 1"]);
    assert_eq!(last_history(&h), "Layer Order");
    // Cmd+E merges it down into the background
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::E);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Layer 1"]);
    assert_eq!(last_history(&h), "Merge Down");
    // Shift+Cmd+E merges everything visible
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::E);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background"]);
    assert_eq!(last_history(&h), "Merge Visible");
}

#[test]
fn layers_panel_drag_and_rename() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.run_steps(3);
    // Rows (top to bottom): "Layer 1 copy", "Layer 1", "Background", each
    // 40.5 pt tall starting at 634 pt. Drag the top row below "Layer 1".
    drag(
        &mut h,
        at_pt(1130.0, 654.0),
        at_pt(1130.0, 716.0),
        Modifiers::NONE,
    );
    assert_eq!(layer_names(&h), ["Background", "Layer 1 copy", "Layer 1"]);
    assert_eq!(last_history(&h), "Layer Order");
    // Nothing goes below the background
    drag(
        &mut h,
        at_pt(1130.0, 654.0),
        at_pt(1130.0, 760.0),
        Modifiers::NONE,
    );
    assert_eq!(layer_names(&h), ["Background", "Layer 1 copy", "Layer 1"]);

    // Double-click the middle row's name and type a new one
    double_click(&mut h, at_pt(1130.0, 694.0));
    assert!(active(&h).renaming.is_some());
    h.event(egui::Event::Text("Sky".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(active(&h).renaming.is_none());
    assert_eq!(layer_names(&h), ["Background", "Sky", "Layer 1"]);
    assert_eq!(last_history(&h), "Rename Layer");

    // Clicking the background's lock makes it "Layer 0"
    click(&mut h, at_pt(1329.0, 734.0));
    assert_eq!(layer_names(&h), ["Layer 0", "Sky", "Layer 1"]);
    assert!(!active(&h).doc.layers[0].is_background);
}

#[test]
#[ignore]
fn screenshot_layers() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.run_steps(3);
    shot(&mut h, "layers");
    double_click(&mut h, at_pt(1130.0, 694.0));
    shot(&mut h, "layers_rename");
}
