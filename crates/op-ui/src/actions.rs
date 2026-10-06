//! File operations and global shortcuts.

use std::path::PathBuf;

use egui::Key;
use op_core::{Color, Document};

use crate::state::AppState;

pub fn open_paths(app: &mut AppState, paths: Vec<PathBuf>) {
    for path in paths {
        match op_io::open(&path) {
            Ok(doc) => app.add_document(doc, "Open"),
            Err(e) => {
                log::error!("{}: {e}", path.display());
                app.alert = Some(format!("Could not open “{}”: {e}", path.display()));
            }
        }
    }
}

pub fn open_dialog(app: &mut AppState) {
    let paths = rfd::FileDialog::new()
        .add_filter("Images", op_io::OPEN_EXTENSIONS)
        .pick_files();
    if let Some(paths) = paths {
        open_paths(app, paths);
    }
}

pub fn new_document(app: &mut AppState) {
    app.untitled_counter += 1;
    let doc = Document::new_with_background(
        format!("Untitled-{}", app.untitled_counter),
        1920,
        1080,
        Color::WHITE,
    );
    app.add_document(doc, "New");
}

pub fn export_dialog(app: &mut AppState) {
    let Some(state) = app.active() else {
        return;
    };
    let stem = state
        .doc
        .title
        .rsplit_once('.')
        .map_or(state.doc.title.as_str(), |(s, _)| s)
        .to_owned();
    let path = rfd::FileDialog::new()
        .set_file_name(format!("{stem}.png"))
        .add_filter("PNG", &["png"])
        .add_filter("JPEG", &["jpg", "jpeg"])
        .save_file();
    if let Some(path) = path
        && let Err(e) = op_io::export_composite(&state.doc, &path)
    {
        app.alert = Some(format!("Could not export: {e}"));
    }
}

pub fn close_active(app: &mut AppState) {
    if let Some(id) = app.active_doc {
        app.close_document(id);
    }
}

/// File > Close All.
pub fn close_all(app: &mut AppState) {
    for id in app.doc_order.clone() {
        app.close_document(id);
    }
}

/// File > Close Others: closes every document except the active one.
pub fn close_others(app: &mut AppState) {
    let active = app.active_doc;
    for id in app.doc_order.clone() {
        if Some(id) != active {
            app.close_document(id);
        }
    }
}

/// Single-key tool and color shortcuts. Modifier shortcuts are commands; see
/// [`crate::commands`].
pub fn handle_tool_keys(ctx: &egui::Context, app: &mut AppState) {
    if app.modal_open() {
        return;
    }
    // Single-key shortcuts are ignored while a text field has focus
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let keys: Vec<(Key, bool)> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } if !modifiers.command && !modifiers.ctrl && !modifiers.alt => {
                    Some((*key, modifiers.shift))
                }
                _ => None,
            })
            .collect()
    });
    for (key, shift) in keys {
        if paint_key(app, key, shift) {
            continue;
        }
        match key {
            Key::D if !shift => crate::toolbar::reset_colors(app),
            Key::X if !shift => crate::toolbar::swap_colors(app),
            _ => {
                let name = key.name();
                if name.len() == 1 {
                    let slots = app.tool_slots.clone();
                    let key = name.chars().next().unwrap();
                    if let Some(tool) =
                        op_tools::tool_for_key(key, shift, app.tool, |slot| slots[slot])
                    {
                        app.select_tool(tool);
                    }
                }
            }
        }
    }
}

/// Painting-tool keys, as in Photoshop: `[`/`]` change the size, Shift+`[`/`]`
/// the hardness in 25% steps, number keys the opacity (1 = 10% … 0 = 100%)
/// and Shift+number the flow. Returns whether the key was used.
fn paint_key(app: &mut AppState, key: Key, shift: bool) -> bool {
    let tool = app.tool;
    let Some(opts) = app.paint_options(tool) else {
        return false;
    };
    let digit = match key {
        Key::Num0 => Some(10),
        Key::Num1 => Some(1),
        Key::Num2 => Some(2),
        Key::Num3 => Some(3),
        Key::Num4 => Some(4),
        Key::Num5 => Some(5),
        Key::Num6 => Some(6),
        Key::Num7 => Some(7),
        Key::Num8 => Some(8),
        Key::Num9 => Some(9),
        _ => None,
    };
    match (key, shift) {
        (Key::OpenBracket, false) => {
            let step = crate::state::PaintOptions::size_step(opts.size - 0.5);
            opts.size = (opts.size - step).max(1.0);
        }
        (Key::CloseBracket, false) => {
            let step = crate::state::PaintOptions::size_step(opts.size);
            opts.size = (opts.size + step).min(crate::state::PaintOptions::MAX_SIZE);
        }
        (Key::OpenBracket, true) => opts.hardness = (opts.hardness - 0.25).max(0.0),
        (Key::CloseBracket, true) => opts.hardness = (opts.hardness + 0.25).min(1.0),
        _ => match digit {
            Some(d) if shift && tool != op_tools::Tool::Pencil => opts.flow = d as f32 / 10.0,
            Some(d) => opts.opacity = d as f32 / 10.0,
            None => return false,
        },
    }
    true
}
