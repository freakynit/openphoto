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

fn run_command(h: &mut Harness<'_, OpenPhotoApp>, command: crate::commands::Command) {
    let ctx = h.ctx.clone();
    crate::commands::run(command, &ctx, &mut h.state_mut().state);
    h.run_steps(3);
}

#[test]
fn rotate_crop_and_trim() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    run_command(&mut h, Command::Rotate90Clockwise);
    assert_eq!((active(&h).doc.width, active(&h).doc.height), (811, 734));
    assert_eq!(last_history(&h), "Rotate Canvas");

    // Crop is only available with a selection
    assert!(!Command::Crop.enabled(&h.state().state));
    select_rect(&mut h, 10.0, 20.0, 110.0, 70.0);
    run_command(&mut h, Command::Crop);
    assert_eq!((active(&h).doc.width, active(&h).doc.height), (100, 50));
    assert_eq!(last_history(&h), "Crop");

    // A red square on white: Trim based on the top-left color keeps it
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.state_mut().state.foreground = Color::from_rgba8([255, 255, 255, 255]);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 30.0, 10.0, 40.0, 30.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    run_command(&mut h, Command::Trim);
    assert!(h.state().state.trim_dialog.is_some());
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().state.trim_dialog.is_none());
    assert_eq!((active(&h).doc.width, active(&h).doc.height), (10, 20));
    assert_eq!(last_history(&h), "Trim");
}

#[test]
#[ignore]
fn screenshot_trim_dialog() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    run_command(&mut h, crate::commands::Command::Trim);
    shot(&mut h, "trim_dialog");
}

#[test]
fn adjustments_invert_and_threshold_preview() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // The reference document is #141414 gray
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::I);
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 5, 5), [0xeb, 0xeb, 0xeb, 255]);
    assert_eq!(last_history(&h), "Invert");

    // Threshold previews while open; Cancel puts the pixels back
    run_command(&mut h, Command::Threshold);
    assert!(h.state().state.adjust_dialog.is_some());
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 5, 5), [255, 255, 255, 255]);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().state.adjust_dialog.is_none());
    assert_eq!(composite_pixel(&mut h, 5, 5), [0xeb, 0xeb, 0xeb, 255]);
    assert_eq!(last_history(&h), "Invert");

    // Posterize to 2 levels with Enter
    run_command(&mut h, Command::Posterize);
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 5, 5), [255, 255, 255, 255]);
    assert_eq!(last_history(&h), "Posterize");
}

#[test]
#[ignore]
fn screenshot_threshold_dialog() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    run_command(&mut h, crate::commands::Command::Threshold);
    shot(&mut h, "threshold_dialog");
}

#[test]
fn levels_dialog_sets_the_black_point() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::L);
    h.run_steps(3);
    assert!(h.state().state.adjust_dialog.is_some());
    // The input black field has focus with its text selected
    h.event(egui::Event::Text("20".into()));
    h.run_steps(3);
    assert_eq!(composite_pixel(&mut h, 5, 5), [0, 0, 0, 255]);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().state.adjust_dialog.is_none());
    assert_eq!(composite_pixel(&mut h, 5, 5), [0, 0, 0, 255]);
    assert_eq!(last_history(&h), "Levels");
}

#[test]
#[ignore]
fn screenshot_adjustment_dialogs() {
    use crate::commands::Command;
    for (command, name) in [
        (Command::Levels, "levels_dialog"),
        (Command::HueSaturation, "hue_saturation_dialog"),
        (Command::Exposure, "exposure_dialog"),
    ] {
        let mut h = harness(Vec::new());
        reference_document(&mut h);
        run_command(&mut h, command);
        shot(&mut h, name);
    }
}

#[test]
fn filters_apply_and_repeat_with_last_filter() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A white square on the dark background
    h.state_mut().state.foreground = Color::from_rgba8([255, 255, 255, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    assert!(!Command::LastFilter.enabled(&h.state().state));

    run_command(&mut h, Command::GaussianBlur);
    h.event(egui::Event::Text("4".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(last_history(&h), "Gaussian Blur");
    let once = composite_pixel(&mut h, 100, 120);
    assert!(once[0] > 0x14 && once[0] < 255, "{once:?}");

    // Ctrl+Cmd+F runs it again with the same radius
    let states = active(&h).history.states().len();
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::CTRL, egui::Key::F);
    h.run_steps(3);
    assert_eq!(active(&h).history.states().len(), states + 1);
    assert_eq!(last_history(&h), "Gaussian Blur");
    // Blurred further: the edge pixel moves on toward the background
    let twice = composite_pixel(&mut h, 100, 120);
    assert!(twice[0] < once[0], "{once:?} {twice:?}");
}

#[test]
#[ignore]
fn screenshot_filter_dialogs() {
    use crate::commands::Command;
    for (command, name) in [
        (Command::GaussianBlur, "gaussian_blur_dialog"),
        (Command::AddNoise, "add_noise_dialog"),
        (Command::Offset, "offset_dialog"),
    ] {
        let mut h = harness(Vec::new());
        reference_document(&mut h);
        run_command(&mut h, command);
        shot(&mut h, name);
    }
}

#[test]
fn saving_reverting_and_closing_with_unsaved_changes() {
    let mut h = harness(Vec::new());
    let id = h.state().state.active_doc.unwrap();
    let title = |h: &Harness<'_, OpenPhotoApp>| crate::doc_tabs::title(active(h));
    assert!(!title(&h).ends_with('*'));

    // An edit marks the document as changed
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    assert!(title(&h).ends_with(" *"), "{}", title(&h));

    // Saving as a Photoshop document takes its name and clears the mark
    let dir = std::env::temp_dir().join(format!("openphoto-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Saved.psd");
    assert!(crate::actions::save_to(
        &mut h.state_mut().state,
        id,
        path.clone(),
        false
    ));
    assert!(title(&h).starts_with("Saved.psd @"), "{}", title(&h));
    assert!(!title(&h).ends_with('*'));
    // Undoing past the save is a change; redoing back to it is not
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    assert!(active(&h).is_dirty());
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::Z);
    h.run_steps(2);
    assert!(!active(&h).is_dirty());

    // Revert (F12) goes back to the file
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Backspace);
    h.run_steps(2);
    let filled = composite_pixel(&mut h, 5, 5);
    h.key_press(egui::Key::F12);
    h.run_steps(2);
    assert_ne!(composite_pixel(&mut h, 5, 5), filled);
    assert_eq!(last_history(&h), "Revert");
    assert!(!active(&h).is_dirty());

    // Closing a changed document asks first: Escape keeps it open...
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::W);
    h.run_steps(2);
    assert_eq!(h.state().state.save_prompt, Some(id));
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(h.state().state.save_prompt, None);
    assert!(h.state().state.docs.contains_key(&id));
    // ...and Don't Save (Cmd+D) closes it
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::W);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    assert!(!h.state().state.docs.contains_key(&id));
    std::fs::remove_dir_all(dir).ok();
}

#[test]
#[ignore]
fn screenshot_save_prompt() {
    let mut h = harness(Vec::new());
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::W);
    h.run_steps(3);
    shot(&mut h, "save_prompt");
}

#[test]
fn eyedropper_magic_wand_and_lassos() {
    use op_tools::Tool;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A red square at 100..140
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);

    // Eyedropper: click picks the foreground, Alt-click the background
    h.state_mut().state.foreground = Color::WHITE;
    h.state_mut().state.select_tool(Tool::Eyedropper);
    let inside = doc_point(&h, 120.0, 120.0);
    click(&mut h, inside);
    assert_eq!(h.state().state.foreground.to_rgba8(), [255, 0, 0, 255]);
    let outside = doc_point(&h, 50.0, 50.0);
    h.hover_at(outside);
    for pressed in [true, false] {
        h.event_modifiers(
            egui::Event::PointerButton {
                pos: outside,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::ALT,
            },
            Modifiers::ALT,
        );
        h.step();
    }
    h.run_steps(2);
    assert_eq!(
        h.state().state.background.to_rgba8(),
        [0x14, 0x14, 0x14, 255]
    );
    // A 5×5 average across the square's corner mixes the two colors
    h.state_mut().state.eyedropper.size = 5;
    let corner = doc_point(&h, 100.5, 100.5);
    click(&mut h, corner);
    let mixed = h.state().state.foreground.to_rgba8();
    assert!(mixed[0] > 0x14 && mixed[0] < 255, "{mixed:?}");

    // Magic Wand selects the square (anti-aliased: the ring around it is
    // partly selected)
    h.state_mut().state.select_tool(Tool::MagicWand);
    click(&mut h, inside);
    let s = active(&h).doc.selection().unwrap();
    assert_eq!((s.get(100, 100), s.get(139, 139)), (255, 255));
    assert!(s.get(99, 120) < 255 && s.get(97, 120) == 0);
    assert_eq!(last_history(&h), "Magic Wand");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);

    // Polygonal Lasso: three corners, then Enter
    h.state_mut().state.select_tool(Tool::PolygonalLasso);
    for (x, y) in [(10.0, 10.0), (60.0, 10.0), (10.0, 60.0)] {
        let p = doc_point(&h, x, y);
        click(&mut h, p);
    }
    assert_eq!(active(&h).lasso.as_ref().map(|l| l.points.len()), Some(3));
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let s = active(&h).doc.selection().unwrap();
    assert_eq!(s.get(20, 20), 255);
    assert_eq!(s.get(50, 50), 0);
    assert_eq!(last_history(&h), "Polygonal Lasso");

    // Lasso: a freehand drag (right, then down-left) replaces it
    h.state_mut().state.select_tool(Tool::Lasso);
    let a = doc_point(&h, 200.0, 200.0);
    let path = [(260.0, 200.0), (200.0, 260.0)].map(|(x, y)| doc_point(&h, x, y));
    h.hover_at(a);
    h.event(egui::Event::PointerButton {
        pos: a,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    h.step();
    for p in path {
        for k in 1..=4 {
            let from = h.ctx.input(|i| i.pointer.latest_pos()).unwrap_or(a);
            h.event(egui::Event::PointerMoved(
                from + (p - from) * (k as f32 / 4.0),
            ));
            h.step();
        }
    }
    h.event(egui::Event::PointerButton {
        pos: path[1],
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    h.run_steps(3);
    assert_eq!(last_history(&h), "Lasso");
    assert!(active(&h).lasso.is_none());
    let s = active(&h).doc.selection().unwrap();
    assert_eq!((s.get(210, 210), s.get(20, 20)), (255, 0));
}

#[test]
fn gradient_tool_paints_foreground_to_background() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([0, 0, 0, 255]);
    h.state_mut().state.background = Color::from_rgba8([255, 255, 255, 255]);
    h.key_press(egui::Key::G);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::Gradient);
    let (a, b) = (doc_point(&h, 100.0, 300.0), doc_point(&h, 600.0, 300.0));
    drag(&mut h, a, b, Modifiers::NONE);
    assert_eq!(last_history(&h), "Gradient");
    assert_eq!(composite_pixel(&mut h, 50, 10), [0, 0, 0, 255]);
    assert_eq!(composite_pixel(&mut h, 700, 700), [255, 255, 255, 255]);
    let mid = composite_pixel(&mut h, 350, 500)[0];
    assert!((118..=138).contains(&mid), "{mid}");
    shot(&mut h, "gradient");
}

#[test]
fn free_transform_moves_scales_and_cancels() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // The background can't be transformed without a selection
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::T);
    h.run_steps(2);
    assert_eq!(
        h.state().state.alert.as_deref(),
        Some("Could not complete the Free Transform command because the layer is locked.")
    );
    h.state_mut().state.alert = None;

    // A red square 100..140 on a new layer
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);

    // Cmd+T, drag inside the box by (100, 50), Enter
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::T);
    h.run_steps(2);
    assert!(active(&h).free_transform.is_some());
    let (a, b) = (doc_point(&h, 120.0, 120.0), doc_point(&h, 220.0, 170.0));
    drag(&mut h, a, b, Modifiers::NONE);
    // Previewed while still transforming
    assert_eq!(layer_pixel(&h, 1, 210, 160), [255, 0, 0, 255]);
    shot(&mut h, "free_transform");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(active(&h).free_transform.is_none());
    assert_eq!(last_history(&h), "Free Transform");
    assert_eq!(layer_pixel(&h, 1, 210, 160), [255, 0, 0, 255]);
    assert_eq!(layer_pixel(&h, 1, 110, 110)[3], 0);

    // Scale by the bottom-right handle, then Escape: nothing changes
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::T);
    h.run_steps(2);
    let (a, b) = (doc_point(&h, 240.0, 190.0), doc_point(&h, 280.0, 230.0));
    drag(&mut h, a, b, Modifiers::NONE);
    let t = active(&h).free_transform.as_ref().unwrap();
    assert!((t.scale.0 - 2.0).abs() < 0.05, "{:?}", t.scale);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(active(&h).free_transform.is_none());
    assert_eq!(layer_pixel(&h, 1, 260, 210)[3], 0);
    assert_eq!(last_history(&h), "Free Transform");

    // Shift+Cmd+T moves it by (100, 50) again
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::T);
    h.run_steps(2);
    assert_eq!(layer_pixel(&h, 1, 310, 210), [255, 0, 0, 255]);
    assert_eq!(last_history(&h), "Transform Again");
}

#[test]
fn crop_tool_crops_to_the_box() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press(egui::Key::C);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::Crop);
    // The box starts on the whole canvas
    let full = active(&h).crop.unwrap().rect;
    assert_eq!(full.size(), egui::vec2(734.0, 811.0));
    // Pull the bottom-right handle in by (234, 311)
    let (a, b) = (doc_point(&h, 734.0, 811.0), doc_point(&h, 500.0, 500.0));
    drag(&mut h, a, b, Modifiers::NONE);
    shot(&mut h, "crop");
    let r = active(&h).crop.unwrap().rect;
    assert!(
        (r.width() - 500.0).abs() < 2.0 && (r.height() - 500.0).abs() < 2.0,
        "{r:?}"
    );
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let d = &active(&h).doc;
    assert!((d.width as i32 - 500).abs() <= 2 && (d.height as i32 - 500).abs() <= 2);
    assert_eq!(last_history(&h), "Crop");
    // The box covers the new canvas again
    let r = active(&h).crop.unwrap().rect;
    assert_eq!(r.size(), egui::vec2(d.width as f32, d.height as f32));
    // Another tool drops the box
    h.key_press(egui::Key::M);
    h.run_steps(2);
    assert!(active(&h).crop.is_none());
}

#[test]
fn image_size_resamples_proportionally() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::I);
    h.run_steps(3);
    assert!(h.state().state.image_size_dialog.is_some());
    // Width has focus with its text selected; the chain updates Height
    h.event(egui::Event::Text("367".into()));
    h.run_steps(3);
    shot(&mut h, "image_size");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().state.image_size_dialog.is_none());
    let d = &active(&h).doc;
    assert_eq!((d.width, d.height), (367, 406));
    assert_eq!(last_history(&h), "Image Size");
    assert_eq!(composite_pixel(&mut h, 100, 100), [0x14, 0x14, 0x14, 255]);
}

#[test]
fn rulers_and_guides() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::R);
    h.run_steps(3);
    assert!(h.state().state.view.rulers);
    // The top ruler runs along the top of the document window (y ≈ 100 pt)
    drag(
        &mut h,
        at_pt(500.0, 100.0),
        at_pt(500.0, 300.0),
        Modifiers::NONE,
    );
    let guides = active(&h).doc.guides.clone();
    assert_eq!(guides.len(), 1);
    assert!(!guides[0].vertical);
    assert_eq!(last_history(&h), "New Guide");
    let y = guides[0].position;
    // Grid on, for the screenshot
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Quote);
    h.run_steps(2);
    shot(&mut h, "rulers");

    // The Move tool drags it down by 50 points
    h.key_press(egui::Key::V);
    h.run_steps(2);
    drag(
        &mut h,
        at_pt(500.0, 300.0),
        at_pt(500.0, 350.0),
        Modifiers::NONE,
    );
    assert_eq!(last_history(&h), "Move Guide");
    let moved = active(&h).doc.guides[0].position;
    assert!(moved > y + 10.0, "{y} -> {moved}");
    // Dropped on the ruler it is deleted; undo brings it back
    drag(
        &mut h,
        at_pt(500.0, 350.0),
        at_pt(500.0, 100.0),
        Modifiers::NONE,
    );
    assert!(active(&h).doc.guides.is_empty());
    assert_eq!(last_history(&h), "Delete Guide");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    assert_eq!(active(&h).doc.guides.len(), 1);
    // Locked guides stay put
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::Semicolon);
    h.run_steps(2);
    assert!(h.state().state.view.lock_guides);
}
