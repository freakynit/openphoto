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

/// A click with modifier keys held (Cmd-click, Shift-click).
pub fn click_with(harness: &mut Harness<'_, OpenPhotoApp>, pos: Pos2, modifiers: Modifiers) {
    harness.event(egui::Event::ModifiersChanged(modifiers));
    click(harness, pos);
    harness.event(egui::Event::ModifiersChanged(Modifiers::NONE));
    harness.step();
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
fn move_auto_select_and_transform_controls() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A red square on Layer 1, then an empty Layer 2 on top (active)
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.run_steps(2);
    let square = active(&h).doc.layers[1].id;

    h.key_press(egui::Key::V);
    h.run_steps(2);
    // Auto-Select's checkbox in the options bar
    click(&mut h, at_pt(115.0, 45.0));
    assert!(h.state().state.move_options.auto_select);
    let (a, b) = (doc_point(&h, 120.0, 120.0), doc_point(&h, 220.0, 170.0));
    drag(&mut h, a, b, Modifiers::NONE);
    assert_eq!(active(&h).doc.active_layer, Some(square));
    assert_eq!(layer_pixel(&h, 1, 220, 170), [255, 0, 0, 255]);

    // Show Transform Controls: the square (now 200..240 × 150..190) gets a
    // box, and dragging its corner starts Free Transform
    click(&mut h, at_pt(264.0, 45.0));
    assert!(h.state().state.move_options.show_transform_controls);
    h.run_steps(2);
    let (a, b) = (doc_point(&h, 240.0, 190.0), doc_point(&h, 260.0, 210.0));
    drag(&mut h, a, b, Modifiers::NONE);
    assert!(active(&h).free_transform.is_some());
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
    click(&mut h, at_pt(13.0, 720.0));
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
    let image = active(h).doc.layers[layer].image().unwrap();
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
    // Rows (top to bottom): "Layer 1 copy", "Layer 1", "Background", 42.5
    // pt apart (a 33.5 pt thumbnail + 8 pt + a 1 pt line) starting at 632 pt.
    // Drag the top row below "Layer 1".
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
    click(&mut h, at_pt(1312.5, 737.75));
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

#[test]
fn modify_selection_and_grow() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    assert!(!Command::ModifyExpand.enabled(&h.state().state));
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    run_command(&mut h, Command::ModifyExpand);
    h.event(egui::Event::Text("5".into()));
    h.run_steps(2);
    shot(&mut h, "expand_selection");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(
        active(&h).doc.selection().unwrap().bounds(),
        Some((95, 95, 145, 145))
    );
    assert_eq!(last_history(&h), "Expand");

    // Shift+F6 opens Feather
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::F6);
    h.run_steps(2);
    assert!(h.state().state.modify_dialog.is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);

    // The whole document is one color: Similar selects all of it
    run_command(&mut h, Command::Similar);
    assert_eq!(
        active(&h).doc.selection().unwrap().bounds(),
        Some((0, 0, 734, 811))
    );
    assert_eq!(last_history(&h), "Similar");
}

fn alt_click(h: &mut Harness<'_, OpenPhotoApp>, pos: Pos2) {
    h.hover_at(pos);
    for pressed in [true, false] {
        h.event_modifiers(
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::ALT,
            },
            Modifiers::ALT,
        );
        h.step();
    }
    h.run_steps(2);
}

#[test]
fn retouching_tools() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A red square at 100..140 to clone, then fill the rest gray
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);

    // Dodge (O) lightens the dark gray background
    h.key_press(egui::Key::O);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::Dodge);
    let p = doc_point(&h, 300.0, 300.0);
    click(&mut h, p);
    assert_eq!(last_history(&h), "Dodge Tool");
    assert!(composite_pixel(&mut h, 300, 300)[0] > 0x14);

    // Clone Stamp (S) without a source point refuses, like Photoshop
    h.key_press(egui::Key::S);
    h.run_steps(2);
    let target = doc_point(&h, 400.0, 400.0);
    click(&mut h, target);
    assert!(
        h.state()
            .state
            .alert
            .as_deref()
            .unwrap()
            .contains("area to clone has not been defined")
    );
    h.state_mut().state.alert = None;
    // Alt-click the square's center, then paint at (400, 400)
    let source = doc_point(&h, 120.0, 120.0);
    alt_click(&mut h, source);
    assert!(active(&h).clone_source.is_some());
    click(&mut h, target);
    assert_eq!(last_history(&h), "Clone Stamp");
    // A soft brush: nearly full strength at its center
    let cloned = composite_pixel(&mut h, 400, 400);
    assert!(cloned[0] >= 250 && cloned[1] <= 5, "{cloned:?}");

    // History Brush (Y) paints the document back as it was opened
    h.key_press(egui::Key::Y);
    h.run_steps(2);
    click(&mut h, target);
    assert_eq!(last_history(&h), "History Brush");
    let restored = composite_pixel(&mut h, 400, 400);
    assert!(restored[0] <= 0x18 && restored[1] == 0x14, "{restored:?}");
}

#[test]
fn layer_masks() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A red layer over the dark background
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    // The background can't take a mask
    assert!(Command::MaskRevealAll.enabled(&h.state().state));

    // Hide Selection: the selected square shows the background through
    select_rect(&mut h, 100.0, 100.0, 140.0, 140.0);
    run_command(&mut h, Command::MaskHideSelection);
    assert_eq!(last_history(&h), "Add Layer Mask");
    assert!(active(&h).doc.editing_mask());
    assert_eq!(composite_pixel(&mut h, 120, 120), [0x14, 0x14, 0x14, 255]);
    assert_eq!(composite_pixel(&mut h, 300, 300), [255, 0, 0, 255]);
    shot(&mut h, "layer_mask");

    // Painting black on the mask hides more; the layer's pixels stay red
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::D);
    select_rect(&mut h, 200.0, 200.0, 240.0, 240.0);
    h.state_mut().state.foreground = Color::from_rgba8([0, 0, 0, 255]);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(composite_pixel(&mut h, 220, 220), [0x14, 0x14, 0x14, 255]);
    assert_eq!(layer_pixel(&h, 1, 220, 220), [255, 0, 0, 255]);

    // Disabled, the mask shows everything; Apply bakes it into the layer
    run_command(&mut h, Command::MaskToggle);
    assert_eq!(last_history(&h), "Disable Layer Mask");
    assert_eq!(composite_pixel(&mut h, 120, 120), [255, 0, 0, 255]);
    run_command(&mut h, Command::MaskToggle);
    run_command(&mut h, Command::MaskApply);
    assert!(active(&h).doc.layers[1].mask.is_none());
    assert_eq!(layer_pixel(&h, 1, 120, 120)[3], 0);
}

#[test]
fn quick_mask_mode() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    select_rect(&mut h, 100.0, 100.0, 400.0, 400.0);
    h.key_press(egui::Key::Q);
    h.run_steps(2);
    assert!(active(&h).doc.quick_mask.is_some());
    assert_eq!(last_history(&h), "Quick Mask");
    assert!(crate::doc_tabs::title(active(&h)).contains("(Quick Mask/8"));
    // Outside the old selection is tinted red
    let tinted = active_canvas_pixel(&mut h, 50, 50);
    assert!(tinted[0] > tinted[1] + 50, "{tinted:?}");
    // Paint black inside it with the brush
    h.state_mut().state.foreground = Color::from_rgba8([0, 0, 0, 255]);
    h.key_press(egui::Key::B);
    h.run_steps(2);
    let p = doc_point(&h, 250.0, 250.0);
    click(&mut h, p);
    shot(&mut h, "quick_mask");
    h.key_press(egui::Key::Q);
    h.run_steps(2);
    let s = active(&h).doc.selection().unwrap();
    // The soft brush leaves its center almost fully masked
    assert!(s.get(250, 250) < 10);
    assert_eq!(s.get(150, 150), 255);
    assert_eq!(s.get(50, 50), 0);
    assert!(active(&h).doc.quick_mask.is_none());
}

/// A pixel of the canvas as displayed (with the Quick Mask tint).
fn active_canvas_pixel(h: &mut Harness<'_, OpenPhotoApp>, x: u32, y: u32) -> [u8; 4] {
    let app = &mut h.state_mut().state;
    let id = app.active_doc.unwrap();
    let img = app.docs.get_mut(&id).unwrap().canvas_image();
    let i = ((y * img.width + x) * 4) as usize;
    img.pixels[i..i + 4].try_into().unwrap()
}

#[test]
fn shape_tools_make_layers() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([0, 128, 255, 255]);
    h.key_press(egui::Key::U);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::Rectangle);
    let (a, b) = (doc_point(&h, 100.0, 100.0), doc_point(&h, 300.0, 200.0));
    drag(&mut h, a, b, Modifiers::NONE);
    assert_eq!(layer_names(&h), ["Background", "Rectangle 1"]);
    assert_eq!(last_history(&h), "Rectangle Tool");
    assert_eq!(composite_pixel(&mut h, 200, 150), [0, 128, 255, 255]);
    assert_eq!(composite_pixel(&mut h, 350, 150), [0x14, 0x14, 0x14, 255]);

    // Shift+U: the Ellipse; Alt draws it around the press point
    h.key_press_modifiers(Modifiers::SHIFT, egui::Key::U);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::Ellipse);
    let (c, d) = (doc_point(&h, 500.0, 500.0), doc_point(&h, 560.0, 560.0));
    drag(&mut h, c, d, Modifiers::ALT);
    assert_eq!(layer_names(&h), ["Background", "Rectangle 1", "Ellipse 1"]);
    // The ellipse spans about 440..560 around (500, 500)
    assert_eq!(composite_pixel(&mut h, 450, 500), [0, 128, 255, 255]);
    assert_eq!(composite_pixel(&mut h, 445, 445), [0x14, 0x14, 0x14, 255]);
    shot(&mut h, "shapes");
}

#[test]
fn type_tool_sets_text() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::WHITE;
    h.state_mut().state.type_options.size_pt = 48.0;
    h.key_press(egui::Key::T);
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::HorizontalType);
    let p = doc_point(&h, 100.0, 200.0);
    click(&mut h, p);
    assert!(active(&h).text_edit.is_some());
    // Single-key tool shortcuts are off while typing: "M" is text
    h.event(egui::Event::Text("Hello".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.event(egui::Event::Text("World".into()));
    h.run_steps(2);
    assert_eq!(h.state().state.tool, op_tools::Tool::HorizontalType);
    // Previewed live as a layer
    assert_eq!(layer_names(&h), ["Background", "Hello"]);
    shot(&mut h, "type_tool");
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Enter);
    h.run_steps(2);
    assert!(active(&h).text_edit.is_none());
    assert_eq!(last_history(&h), "Type Tool");
    // White pixels where the "H" stands, above the baseline at y = 200
    let lit = (100..130).any(|x| composite_pixel(&mut h, x, 180)[0] == 255);
    assert!(lit);
    assert_eq!(composite_pixel(&mut h, 100, 230)[0], 0x14);

    // Escape throws the text away
    let q = doc_point(&h, 100.0, 500.0);
    click(&mut h, q);
    h.event(egui::Event::Text("Gone".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Hello"]);
}

#[test]
fn new_document_dialog() {
    let mut h = harness(Vec::new());
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::N);
    h.run_steps(3);
    assert!(h.state().state.new_document_dialog.is_some());
    shot(&mut h, "new_document");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().state.new_document_dialog.is_none());
    let d = &active(&h).doc;
    assert_eq!(
        (d.title.as_str(), d.width, d.height),
        ("Untitled-2", 1920, 1080)
    );
    assert!(d.layers[0].is_background);

    // A transparent background is a regular "Layer 1"
    let app = &mut h.state_mut().state;
    crate::actions::create_document(
        app,
        "Clear".into(),
        (300, 200),
        150.0,
        crate::dialogs::NewContents::Transparent,
    );
    h.run_steps(2);
    let d = &active(&h).doc;
    assert_eq!(d.layers[0].name, "Layer 1");
    assert!(!d.layers[0].is_background);
    assert_eq!(d.resolution, 150.0);
    assert_eq!(composite_pixel(&mut h, 10, 10)[3], 0);
}

#[test]
fn more_adjustments() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::from_rgba8([255, 0, 0, 255]);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    // Black & White (Alt+Shift+Cmd+B) with the default preset: red is 40%
    h.key_press_modifiers(
        Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT,
        egui::Key::B,
    );
    h.run_steps(3);
    assert!(h.state().state.adjust_dialog.is_some());
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(composite_pixel(&mut h, 5, 5), [102, 102, 102, 255]);
    assert_eq!(last_history(&h), "Black & White");

    // Gradient Map from the foreground (black) to the background (white)
    h.state_mut().state.foreground = Color::from_rgba8([0, 0, 0, 255]);
    run_command(&mut h, Command::GradientMap);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let px = composite_pixel(&mut h, 5, 5);
    assert!(px[0] == px[1] && px[0] > 90 && px[0] < 115, "{px:?}");

    run_command(&mut h, Command::PhotoFilter);
    shot(&mut h, "photo_filter");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
}

#[test]
fn curves_dialog_adds_points() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // A mid gray to bend
    h.state_mut().state.foreground = Color::from_rgba8([128, 128, 128, 255]);
    h.key_press_modifiers(Modifiers::ALT, egui::Key::Backspace);
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::M);
    h.run_steps(3);
    assert!(h.state().state.adjust_dialog.is_some());
    // The 420 × 380 dialog is centered in the 1350 × 800 window; its graph
    // (240 square) starts 20 right and 88 down of the dialog's corner.
    // Click the curve point (128, 192).
    let (gx, gy) = (675.0 - 210.0 + 20.0, 400.0 - 190.0 + 88.0);
    let p = at_pt(
        gx + 128.0 / 255.0 * 240.0,
        gy + 240.0 - 192.0 / 255.0 * 240.0,
    );
    click(&mut h, p);
    shot(&mut h, "curves");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(last_history(&h), "Curves");
    let v = composite_pixel(&mut h, 5, 5)[0];
    assert!((185..=198).contains(&v), "{v}");
}

#[test]
fn info_navigator_and_histogram_panels() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.key_press(egui::Key::F8);
    h.run_steps(2);
    assert!(h.state().state.floating.info);
    run_command(&mut h, Command::ToggleNavigator);
    run_command(&mut h, Command::ToggleHistogram);
    // Hovering the document tells the Info panel where the pointer is
    let p = doc_point(&h, 120.0, 80.0);
    h.hover_at(p);
    h.run_steps(3);
    let pointer = active(&h).pointer.unwrap();
    assert!((pointer.x - 120.0).abs() < 1.0 && (pointer.y - 80.0).abs() < 1.0);
    // Each panel in its own place
    let rect = |name: &str| {
        h.ctx
            .memory(|m| m.area_rect(egui::Id::new(("floating-panel", name))))
            .unwrap()
    };
    assert!(!rect("Info").intersects(rect("Navigator")));
    assert!(!rect("Info").intersects(rect("Histogram")));
    shot(&mut h, "floating_panels");
    // The Navigator moves the view: clicking its thumbnail's corner
    let before = active(&h).view.offset;
    let ppp = 2.0;
    crate::document_view::center_on(
        h.state_mut().state.active().unwrap(),
        egui::pos2(0.0, 0.0),
        ppp,
    );
    assert_ne!(active(&h).view.offset, before);
    h.key_press(egui::Key::F8);
    h.run_steps(2);
    assert!(!h.state().state.floating.info);
}

#[test]
#[ignore]
fn screenshot_move_tool() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::WHITE;
    h.state_mut().state.background = Color::from_rgba8([0x14, 0xa5, 0xdc, 255]);
    h.key_press(egui::Key::V);
    h.run_steps(3);
    shot(&mut h, "move_tool");
}

/// Gray levels Photoshop 2026 shows at these points (in points from the
/// window's top-left corner, sampled from a 2x capture of its default
/// workspace with the Move tool). Each one pins a measured edge: the 1 pt
/// lines of dividers and collapse bars, separators, frames and icons. A
/// layout change that moves any of them by a point fails here.
const PHOTOSHOP_PIXELS: &[(&str, f32, f32, u8)] = &[
    ("collapse bar top line", 1305.0, 62.5, 56),
    ("collapse bar", 1305.0, 68.0, 66),
    ("panel tab bar", 1305.0, 90.0, 67),
    ("tab bar line", 1305.0, 102.5, 56),
    ("color body", 1300.0, 150.0, 83),
    ("group gap dark", 1300.0, 222.5, 56),
    ("group gap light", 1300.0, 223.5, 71),
    ("group gap dark2", 1300.0, 224.5, 56),
    ("props tab line", 1300.0, 252.5, 56),
    ("layers gap light", 1300.0, 512.5, 71),
    ("layers tab bar", 1300.0, 530.0, 66),
    ("strip left dark", 985.5, 300.0, 56),
    ("strip left light", 986.5, 300.0, 71),
    ("strip left dark2", 987.5, 300.0, 56),
    ("strip right dark", 1025.5, 300.0, 56),
    ("strip right light", 1026.5, 300.0, 71),
    ("panel body left", 1028.5, 300.0, 83),
    ("toolbar divider dark", 39.5, 300.0, 56),
    ("toolbar divider light", 40.5, 300.0, 71),
    ("toolbar bar line", 20.0, 62.5, 56),
    ("toolbar collapse", 30.0, 68.0, 66),
    ("options sep1", 53.5, 45.0, 62),
    ("options sep2", 102.5, 45.0, 62),
    ("home leg", 24.0, 50.0, 221),
    ("home door", 28.0, 50.0, 83),
    ("fg swatch", 13.0, 720.0, 255),
    ("fg frame", 4.5, 720.0, 54),
    ("bg frame", 33.5, 735.0, 54),
    ("bg white", 32.5, 735.0, 255),
    ("panel fg frame light", 1036.5, 120.0, 140),
    ("sv field left", 1072.5, 150.0, 83),
    ("hue left", 1313.5, 150.0, 83),
    ("doc box border", 1038.5, 270.0, 99),
    ("doc box fill", 1040.5, 262.0, 56),
    ("W field border", 1092.5, 325.0, 102),
    ("W field fill", 1093.5, 320.0, 69),
    ("X field fill", 1172.0, 320.0, 77),
    ("strip line", 1000.0, 143.5, 56),
    ("strip grip", 997.5, 80.0, 69),
    ("menu line", 1340.0, 85.5, 168),
    ("menu gap", 1340.0, 86.5, 66),
    ("layers sep 1", 1300.0, 573.5, 62),
    ("layers sep 2", 1300.0, 601.5, 62),
    ("kind border", 1031.5, 560.0, 94),
    ("blend fill", 1100.0, 590.0, 77),
    ("opacity border", 1208.5, 590.0, 94),
    ("fill field border", 1208.5, 615.0, 94),
    ("eye column", 1031.0, 645.0, 83),
    ("eye divider", 1058.0, 645.0, 69),
    ("row selected", 1200.0, 640.0, 107),
    ("list bg", 1200.0, 700.0, 77),
    ("footer line", 1200.0, 775.5, 62),
    ("new layer outline", 1286.5, 787.5, 221),
    ("eye stroke", 1037.5, 653.0, 221),
    ("thumb frame", 1062.25, 650.0, 46),
];

#[test]
fn layout_matches_photoshop_2026() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.foreground = Color::WHITE;
    h.state_mut().state.background = Color::from_rgba8([0x14, 0xa5, 0xdc, 255]);
    h.key_press(egui::Key::V);
    h.run_steps(3);
    let image = h.render().expect("render frame");
    let mut wrong = Vec::new();
    for &(name, x, y, expected) in PHOTOSHOP_PIXELS {
        let [r, g, b, _] = image.get_pixel((x * 2.0) as u32, (y * 2.0) as u32).0;
        let gray = (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32).round() as i32;
        if (gray - expected as i32).abs() > 6 {
            wrong.push(format!(
                "{name} at ({x}, {y}): {gray}, Photoshop {expected}"
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "differs from Photoshop:
{}",
        wrong.join(
            "
"
        )
    );
}

#[test]
fn layers_panel_footer_and_lock_buttons() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // Footer: Create a new layer (58.25 pt from the right edge)
    click(&mut h, at_pt(1350.0 - 58.25, 787.5));
    assert_eq!(layer_names(&h), ["Background", "Layer 1"]);
    assert_eq!(last_history(&h), "New Layer");
    // Lock transparent pixels, then Lock position
    click(&mut h, at_pt(1028.0 + 43.5, 542.0 + 73.75));
    click(&mut h, at_pt(1028.0 + 88.0, 542.0 + 73.75));
    let layer = &active(&h).doc.layers[1];
    assert!(layer.lock_transparency && layer.lock_position && !layer.lock_pixels);
    assert_eq!(last_history(&h), "Lock Layer");
    // Lock all locks everything; while it's on the other buttons are inert
    click(&mut h, at_pt(1028.0 + 129.0, 542.0 + 73.75));
    click(&mut h, at_pt(1028.0 + 88.0, 542.0 + 73.75));
    let layer = &active(&h).doc.layers[1];
    assert!(layer.lock_all && layer.pixels_locked() && layer.lock_position);
    // Turning it off brings back the locks underneath
    click(&mut h, at_pt(1028.0 + 129.0, 542.0 + 73.75));
    let layer = &active(&h).doc.layers[1];
    assert!(!layer.lock_all && layer.lock_transparency && layer.lock_position);
    assert!(!layer.pixels_locked());
    // Prevent auto-nesting has its own flag
    click(&mut h, at_pt(1028.0 + 110.5, 542.0 + 73.75));
    assert!(active(&h).doc.layers[1].lock_nesting);
    // The background's locks can't be changed
    click(&mut h, at_pt(1100.0, 632.0 + 43.5 + 21.0));
    click(&mut h, at_pt(1028.0 + 43.5, 542.0 + 73.75));
    assert!(!active(&h).doc.layers[0].lock_transparency);
    // Footer: Delete layer removes Layer 1 again
    click(&mut h, at_pt(1100.0, 652.0));
    click(&mut h, at_pt(1350.0 - 30.25, 787.5));
    assert_eq!(layer_names(&h), ["Background"]);
    assert_eq!(last_history(&h), "Delete Layer");
}

#[test]
fn lock_layers_dialog() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // Only the background: nothing to lock
    assert!(!Command::LockLayers.enabled(&h.state().state));
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.run_steps(2);
    h.state_mut().state.active().unwrap().doc.layers[1].lock_position = true;
    // The menu item opens it with the layer's locks ticked
    run_command(&mut h, Command::LockLayers);
    assert!(h.state().state.lock_dialog.is_some());
    // The dialog is centered: its corner at (540.5, 297) pt
    let at = |x: f32, y: f32| at_pt(540.5 + x, 297.0 + y);
    // Tick Image and Prevent auto-nest, then OK
    click(&mut h, at(58.0, 86.0));
    click(&mut h, at(58.0, 138.0));
    click(&mut h, at(214.0, 60.0));
    assert!(h.state().state.lock_dialog.is_none());
    let layer = &active(&h).doc.layers[1];
    assert!(layer.lock_pixels && layer.lock_position && layer.lock_nesting);
    assert!(!layer.lock_transparency && !layer.lock_all);
    assert_eq!(last_history(&h), "Lock Layers");
    // All (Enter for OK)
    run_command(&mut h, Command::LockLayers);
    click(&mut h, at(58.0, 174.0));
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(active(&h).doc.layers[1].lock_all);
    // Cancel leaves the locks alone
    run_command(&mut h, Command::LockLayers);
    click(&mut h, at(58.0, 174.0));
    click(&mut h, at(214.0, 96.0));
    assert!(h.state().state.lock_dialog.is_none());
    assert!(active(&h).doc.layers[1].lock_all);
}

#[test]
fn cmd_slash_toggles_lock_all() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // On the background Photoshop answers with an alert
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Slash);
    h.run_steps(2);
    assert_eq!(
        h.state().state.alert.as_deref(),
        Some("The command \u{201c}Set\u{201d} is not currently available.")
    );
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.state().state.alert.is_none());
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.run_steps(2);
    h.state_mut().state.active().unwrap().doc.layers[1].lock_position = true;
    // Locks all without a dialog...
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Slash);
    h.run_steps(2);
    assert!(h.state().state.lock_dialog.is_none());
    assert!(active(&h).doc.layers[1].lock_all);
    assert_eq!(last_history(&h), "Lock Layer");
    // ...and unlocks everything
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Slash);
    h.run_steps(2);
    assert_eq!(active(&h).doc.layers[1].locks(), op_core::Locks::default());
    assert_eq!(last_history(&h), "Unlock Layer");
}

#[test]
#[ignore]
fn screenshot_lock_layers_dialog() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.run_steps(2);
    run_command(&mut h, crate::commands::Command::LockLayers);
    h.run_steps(3);
    shot(&mut h, "lock_layers");
}

#[test]
fn duplicate_layer_and_layer_from_background_dialogs() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    let center = |w: f32, hgt: f32| (675.0 - w / 2.0, 400.0 - hgt / 2.0);

    // Duplicate Layer... into this document under a new name
    run_command(&mut h, crate::commands::Command::DuplicateLayer);
    assert!(h.state().state.duplicate_dialog.is_some());
    h.event(egui::Event::Text("Copy A".into()));
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Copy A"]);
    assert_eq!(last_history(&h), "Duplicate Layer");

    // ...and to a new document, which opens
    let first = h.state().state.active_doc;
    run_command(&mut h, crate::commands::Command::DuplicateLayer);
    let (x0, y0) = center(447.0, 208.0);
    click(&mut h, at_pt(x0 + 200.0, y0 + 121.5));
    h.get_by_label("New").click();
    h.run_steps(2);
    click(&mut h, at_pt(x0 + 408.0, y0 + 51.0));
    h.run_steps(2);
    let app = &h.state().state;
    assert_ne!(app.active_doc, first);
    assert_eq!(app.docs.len(), 2);
    // The next "Untitled-N" (the counter keeps counting, as in Photoshop)
    assert!(active(&h).doc.title.starts_with("Untitled-"));
    assert_eq!(layer_names(&h), ["Copy A copy"]);

    // Back on the first document: Layer from Background... asks for a name
    h.state_mut().state.active_doc = first;
    h.run_steps(2);
    run_command(&mut h, crate::commands::Command::LayerFromBackground);
    assert!(
        h.state()
            .state
            .new_layer_dialog
            .as_ref()
            .is_some_and(|d| d.kind() == crate::dialogs::NewLayerKind::FromBackground)
    );
    h.event(egui::Event::Text("Base".into()));
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Base", "Copy A"]);
    assert!(!active(&h).doc.layers[0].is_background);
    assert_eq!(last_history(&h), "Layer From Background");
}

#[test]
fn double_clicking_the_background_asks_for_a_name() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    double_click(&mut h, at_pt(1130.0, 652.0));
    assert!(
        h.state()
            .state
            .new_layer_dialog
            .as_ref()
            .is_some_and(|d| d.kind() == crate::dialogs::NewLayerKind::FromBackground)
    );
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(active(&h).doc.layers[0].is_background);
}

#[test]
#[ignore]
fn screenshot_duplicate_layer_dialog() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    run_command(&mut h, crate::commands::Command::DuplicateLayer);
    shot(&mut h, "duplicate_layer");
}

#[test]
fn flatten_asks_before_discarding_hidden_layers() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    crate::panels::toggle_active_visibility(h.state_mut().state.active().unwrap());
    h.run_steps(2);
    // Cancel keeps both layers
    run_command(&mut h, crate::commands::Command::FlattenImage);
    assert!(h.state().state.flatten_prompt.is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().state.flatten_prompt.is_none());
    assert_eq!(active(&h).doc.layers.len(), 2);
    // "Don't show again", then OK: flattened, and not asked next time
    run_command(&mut h, crate::commands::Command::FlattenImage);
    let (x0, y0) = (675.0 - 130.0, 400.0 - 104.0);
    click(&mut h, at_pt(x0 + 30.0, y0 + 140.0));
    click(&mut h, at_pt(x0 + 189.0, y0 + 178.0));
    assert_eq!(layer_names(&h), ["Background"]);
    assert_eq!(last_history(&h), "Flatten Image");
    assert!(h.state().state.skip_flatten_prompt);
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    crate::panels::toggle_active_visibility(h.state_mut().state.active().unwrap());
    run_command(&mut h, crate::commands::Command::FlattenImage);
    assert!(h.state().state.flatten_prompt.is_none());
    assert_eq!(layer_names(&h), ["Background"]);
}

#[test]
fn rename_layer_and_alerts() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // The background can't be renamed this way
    assert!(!Command::RenameLayer.enabled(&h.state().state));
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.run_steps(2);
    run_command(&mut h, Command::RenameLayer);
    assert!(active(&h).renaming.is_some());
    h.event(egui::Event::Text("Ink".into()));
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(layer_names(&h), ["Background", "Ink"]);

    // An error alert goes away with Enter
    h.state_mut().state.alert = Some("Could not complete the Copy command.".into());
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.state().state.alert.is_none());
}

#[test]
#[ignore]
fn screenshot_alerts() {
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    h.state_mut().state.alert =
        Some("Could not complete the Copy command because the selected area is empty.".into());
    h.run_steps(3);
    shot(&mut h, "alert_error");
    h.state_mut().state.alert = None;
    h.state_mut().state.flatten_prompt = Some(crate::dialogs::alert::Alert::caution(
        "Discard hidden layers?",
    ));
    h.run_steps(3);
    shot(&mut h, "alert_caution");
}

#[test]
fn layers_multi_selection() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    for _ in 0..3 {
        crate::panels::new_layer(h.state_mut().state.active().unwrap());
    }
    h.run_steps(2);
    // Rows 42.5 pt apart from 632: Layer 3, Layer 2, Layer 1, Background
    let row = |k: f32| at_pt(1130.0, 652.0 + 42.5 * k);
    let ids: Vec<op_core::LayerId> = active(&h).doc.layers.iter().map(|l| l.id).collect();
    click(&mut h, row(0.0));
    click_with(&mut h, row(1.0), Modifiers::COMMAND);
    assert_eq!(active(&h).doc.selected_layers(), [ids[2], ids[3]]);
    // Cmd+E with two selected merges them (Merge Layers)
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::E);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Layer 1", "Layer 3"]);
    assert_eq!(last_history(&h), "Merge Layers");
    // Shift-click selects a range; Cmd+, hides them all
    click(&mut h, row(0.0));
    click_with(&mut h, row(1.0), Modifiers::SHIFT);
    assert_eq!(active(&h).doc.selected_layers().len(), 2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(2);
    assert!(active(&h).doc.layers[1..].iter().all(|l| !l.visible));
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::Comma);
    h.run_steps(2);
    assert!(active(&h).doc.layers.iter().all(|l| l.visible));
    // Alt+Cmd+A: every layer but the background; delete them with the footer
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::A);
    h.run_steps(2);
    assert_eq!(active(&h).doc.selected_layers().len(), 2);
    click(&mut h, at_pt(1350.0 - 30.25, 787.5));
    assert_eq!(layer_names(&h), ["Background"]);
    run_command(&mut h, Command::DeselectLayers);
    assert!(active(&h).doc.selected_layers().is_empty());
}

#[test]
fn align_buttons_line_up_selected_layers() {
    use crate::commands::Command;
    use op_core::align::Distribute;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    // Three layers with a 40 px square each, at different places
    for (x0, y0) in [(100u32, 100u32), (200, 150), (300, 300)] {
        let state = h.state_mut().state.active().unwrap();
        crate::panels::new_layer(state);
        let id = state.doc.active_layer.unwrap();
        let image = state.doc.layer_mut(id).unwrap().image_mut().unwrap();
        for y in y0..y0 + 40 {
            for x in x0..x0 + 40 {
                image.set_pixel(x, y, [255, 0, 0, 255]);
            }
        }
        state.doc.mark_dirty();
    }
    h.key_press(egui::Key::V);
    h.run_steps(2);
    // One layer: the align buttons are off
    assert!(!Command::Align(op_core::align::Align::Left).enabled(&h.state().state));
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::A);
    h.run_steps(2);
    assert!(Command::Align(op_core::align::Align::Left).enabled(&h.state().state));
    // The options bar's "Align left edges" (the first align button)
    click(&mut h, at_pt(423.0, 45.0));
    assert_eq!(last_history(&h), "Align Left Edges");
    let lefts: Vec<i64> = active(&h).doc.layers[1..]
        .iter()
        .map(|l| {
            let image = l.image().unwrap();
            image.content_bounds().unwrap().0
        })
        .collect();
    assert!(lefts.iter().all(|&x| x == lefts[0]), "{lefts:?}");
    run_command(&mut h, Command::Distribute(Distribute::VerticalCenter));
    assert_eq!(last_history(&h), "Distribute Vertical Centers");
}

#[test]
fn layer_groups_in_the_layers_panel() {
    use op_core::LayerKind;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    for _ in 0..2 {
        crate::panels::new_layer(h.state_mut().state.active().unwrap());
    }
    h.run_steps(2);
    // Cmd+G groups the selected layers
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::A);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.run_steps(2);
    assert_eq!(
        layer_names(&h),
        ["Background", "Layer 1", "Layer 2", "Group 1"]
    );
    assert_eq!(last_history(&h), "Group Layers");
    let group = active(&h).doc.layers[3].id;
    assert!(
        active(&h).doc.layers[1..3]
            .iter()
            .all(|l| l.parent == Some(group))
    );
    // Rows: Group 1 (632, 25 pt), Layer 2, Layer 1 (42.5 pt each), Background.
    // The arrow collapses the group (and isn't a history state)
    click(&mut h, at_pt(1028.0 + 38.0, 632.0 + 11.75));
    assert!(matches!(
        active(&h).doc.layers[3].kind,
        LayerKind::Group { collapsed: true }
    ));
    assert_eq!(last_history(&h), "Group Layers");
    click(&mut h, at_pt(1028.0 + 38.0, 632.0 + 11.75));
    assert!(matches!(
        active(&h).doc.layers[3].kind,
        LayerKind::Group { collapsed: false }
    ));
    // Drag Layer 1 (the third row) above the group: out of it
    drag(
        &mut h,
        at_pt(1150.0, 632.0 + 25.0 + 42.5 + 20.0),
        at_pt(1150.0, 633.0),
        Modifiers::NONE,
    );
    assert_eq!(
        layer_names(&h),
        ["Background", "Layer 2", "Group 1", "Layer 1"]
    );
    assert_eq!(active(&h).doc.layers[3].parent, None);
    assert_eq!(last_history(&h), "Layer Order");
    // Shift+Cmd+G on the group ungroups it
    click(&mut h, at_pt(1150.0, 632.0 + 42.5 + 12.0));
    assert_eq!(active(&h).doc.active_layer, Some(group));
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, egui::Key::G);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Layer 2", "Layer 1"]);
    assert_eq!(last_history(&h), "Ungroup Layers");
    // The footer's folder button makes an empty group
    click(&mut h, at_pt(1350.0 - 86.5, 787.5));
    assert!(
        active(&h)
            .doc
            .layers
            .iter()
            .any(|l| l.is_group() && l.name == "Group 1")
    );
    assert_eq!(last_history(&h), "New Group");
}

#[test]
#[ignore]
fn screenshot_layer_groups() {
    let mut h = harness(Vec::new());
    let app = &mut h.state_mut().state;
    crate::actions::close_all(app);
    let mut doc = op_core::Document::new_with_background("groups", 200, 200, Color::WHITE);
    let ids: Vec<op_core::LayerId> = (0..5).map(|_| doc.new_layer_id()).collect();
    let (g1, l1, g2, l2, g3) = (ids[0], ids[1], ids[2], ids[3], ids[4]);
    let layer = |id, name: &str, parent| {
        let mut l = op_core::Layer::raster(id, name, op_core::TiledImage::new(200, 200));
        l.parent = parent;
        l
    };
    let a = layer(l1, "Layer 1", Some(g1));
    let b = layer(l2, "Layer 2", Some(g2));
    let mut group2 = op_core::Layer::group(g2, "Group 2");
    group2.parent = Some(g1);
    doc.layers.extend([
        a,
        b,
        group2,
        op_core::Layer::group(g1, "Group 1"),
        op_core::Layer::group(g3, "Group 3"),
    ]);
    doc.select_layer(l1);
    app.add_document(doc, "New");
    h.run_steps(6);
    shot(&mut h, "layer_groups");
}

#[test]
fn deleting_a_group_asks_what_to_delete() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    for _ in 0..2 {
        crate::panels::new_layer(h.state_mut().state.active().unwrap());
    }
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::A);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.run_steps(2);
    // Group Only (the second of the stacked answers, 34 pt below the first):
    // the layers stay
    run_command(&mut h, Command::DeleteLayer);
    let prompt = h.state().state.delete_group_prompt.clone().expect("asked");
    assert_eq!(
        prompt.message,
        "Delete the group \u{201c}Group 1\u{201d} and its contents or delete only the group?"
    );
    assert_eq!(
        prompt.choices,
        ["Group and Contents", "Group Only", "Cancel"]
    );
    // Three lines of message: buttons from 164 pt in a 276 pt alert
    let (x0, y0) = (675.0 - 130.0, 400.0 - 138.0);
    click(&mut h, at_pt(x0 + 130.0, y0 + 164.0 + 34.0 + 14.0));
    assert_eq!(layer_names(&h), ["Background", "Layer 1", "Layer 2"]);
    assert!(active(&h).doc.layers.iter().all(|l| l.parent.is_none()));
    assert_eq!(last_history(&h), "Delete Layer");
    // Group and Contents (Enter): all of it goes
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::A);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.run_steps(2);
    run_command(&mut h, Command::DeleteLayer);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background"]);
    // Duplicating a group copies its layers
    crate::panels::new_layer(h.state_mut().state.active().unwrap());
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::J);
    h.run_steps(2);
    assert_eq!(
        layer_names(&h),
        [
            "Background",
            "Layer 1",
            "Group 1",
            "Layer 1",
            "Group 1 copy"
        ]
    );
}

#[test]
fn merge_group_and_reverse() {
    use crate::commands::Command;
    let mut h = harness(Vec::new());
    reference_document(&mut h);
    for _ in 0..3 {
        crate::panels::new_layer(h.state_mut().state.active().unwrap());
    }
    // Reverse Layer 1..3
    h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, egui::Key::A);
    h.run_steps(2);
    run_command(&mut h, Command::ArrangeReverse);
    assert_eq!(
        layer_names(&h),
        ["Background", "Layer 3", "Layer 2", "Layer 1"]
    );
    assert_eq!(last_history(&h), "Reverse");
    // Group them, then Cmd+E merges the group into one layer
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::G);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::E);
    h.run_steps(2);
    assert_eq!(layer_names(&h), ["Background", "Group 1"]);
    assert!(!active(&h).doc.layers[1].is_group());
    assert_eq!(last_history(&h), "Merge Group");
}
